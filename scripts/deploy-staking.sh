#!/bin/bash
# shellcheck disable=SC2155

################################################################################
#
# Does deployment + initialization of all programs and accounts needed to run
# the staking + lockup application.
#
# Usage:
#
# ./scripts/deploy-staking.sh <localnet | devnet | mainnet>
#
################################################################################

set -euox pipefail

CLUSTER=$1

if [ "$CLUSTER" = "devnet" ]; then
    echo "Deploying to Devnet..."
    FAUCET_FLAG="--faucet"
    CONFIG_FILE=~/.config/serum/cli/devnet.yaml
    CLUSTER_URL="https://devnet.solana.com"
elif [ "$CLUSTER" = "mainnet" ]; then
    echo "Deploying to Mainnet..."
    FAUCET_FLAG=""
    CONFIG_FILE=~/.config/serum/cli/mainnet.yaml
    CLUSTER_URL="https://api.mainnet-beta.solana.com"
elif [ "$CLUSTER" = "localnet" ]; then
    echo "Deploying to Localnet..."
    FAUCET_FLAG=""
    CONFIG_FILE=~/.config/serum/cli/localnet.yaml
    CLUSTER_URL="http://localhost:8899"
else
    echo "Invalid cluster"
    exit 1
fi

#
# Seconds.
#
DEACTIVATION_TIMELOCK=60
WITHDRAWAL_TIMELOCK=60
#
# 100_000_000 million SRM (6 decimals)
#
MAX_STAKE_PER_ENTITY=100000000000000
#
# 1 SRM (6 decimals) to stake.
#
STAKE_RATE=1000000
#
# 1 MSRM (0 decimals) to stake.
#
STAKE_RATE_MEGA=1
#
# Must be built with the `dev` feature on.
#
serum=$(pwd)/target/debug/serum

main() {
    #
    # Check the CLI is built or installed.
    #
    if ! command -v $serum &> /dev/null
    then
        echo "Serum CLI not installed"
        exit
    fi
    #
    # Build all programs.
    #
    echo "Building all programs..."
    make -s -C lockup build
    make -s -C registry build
    make -s -C registry/meta-entity build
    make -s -C registry/rewards build
    ./do.sh build dex

    #
    # Deploy all the programs.
    #
    echo "Deploying all programs..."
    local pids=$(TEST_CLUSTER="$CLUSTER" TEST_CLUSTER_URL="$CLUSTER_URL" make -s -C registry deploy-all)
    local rewards_pids=$(TEST_CLUSTER="$CLUSTER" TEST_CLUSTER_URL="$CLUSTER_URL" make -s -C registry/rewards deploy-all)

    local registry_pid=$(echo $pids | jq .registryProgramId -r)
    local lockup_pid=$(echo $pids | jq .lockupProgramId -r)
    local meta_entity_pid=$(echo $pids | jq .metaEntityProgramId -r)
    local rewards_pid=$(echo $rewards_pids | jq .rewardsProgramId -r)
    local dex_pid=$(echo $rewards_pids | jq .dexProgramId -r)

    #
    # Generate genesis state. Use dummy accounts, if needed.
    #
    local srm_mint="SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt"
    local msrm_mint="MSRMcoVyrFxnSgo5uXwone5SKcGhT1KEJMFEkMEWf9L"
    local god="FhmUh2PEpTzUwBWPt4qgDBeqfmb2ES3T64CkT1ZiktSS"       # Dummy.
    local god_msrm="FhmUh2PEpTzUwBWPt4qgDBeqfmb2ES3T64CkT1ZiktSS"  # Dummy.
    local srm_faucet="null"
    local msrm_faucet="null"
    if [ "$CLUSTER" != "mainnet" ]; then
        echo "Genesis initialization..."
        genesis=$($serum --config $CONFIG_FILE dev init-mint $FAUCET_FLAG)

        srm_mint=$(echo $genesis | jq .srmMint -r)
        msrm_mint=$(echo $genesis | jq .msrmMint -r)
        god=$(echo $genesis | jq .god -r)
        god_msrm=$(echo $genesis | jq .godMsrm -r)

        if [ "$CLUSTER" = "devnet" ]; then
            srm_faucet_key=$(echo $genesis | jq .srmFaucet -r)
            srm_faucet="new PublicKey('$srm_faucet_key')"
            msrm_faucet_key=$(echo $genesis | jq .msrmFaucet -r)
            msrm_faucet="new PublicKey('$msrm_faucet_key')"
        fi
    fi

    #
    # Write out the CLI configuration file.
    #
    echo "Writing config $CONFIG_FILE..."
    mkdir -p $(dirname $CONFIG_FILE)
    cat << EOM > $CONFIG_FILE
---
network:
  cluster: $CLUSTER

#
# SRM Faucet:  $srm_faucet
# MSRM Faucet: $msrm_faucet
#
mints:
  srm: $srm_mint
  msrm: $msrm_mint

programs:
  rewards_pid: $rewards_pid
  registry_pid: $registry_pid
  meta_entity_pid: $meta_entity_pid
  lockup_pid: $lockup_pid
  dex_pid: $dex_pid

EOM

    #
    # Now intialize all the accounts.
    #
    echo "Initializing registrar..."
    local rInit=$($serum --config $CONFIG_FILE \
          registry init \
          --deactivation-timelock $DEACTIVATION_TIMELOCK \
          --withdrawal-timelock $WITHDRAWAL_TIMELOCK \
          --max-stake-per-entity $MAX_STAKE_PER_ENTITY \
          --stake-rate $STAKE_RATE \
          --stake-rate-mega $STAKE_RATE_MEGA)

    local registrar=$(echo $rInit | jq .registrar -r)
    local registrar_nonce=$(echo $rInit | jq .nonce -r)
    local reward_q=$(echo $rInit | jq .rewardEventQueue -r)

    echo "Initializing lockup..."
    local lInit=$($serum --config $CONFIG_FILE \
          lockup initialize)

    local safe=$(echo $lInit | jq .safe -r)

    #
    # Initialize a node entity. Hack until we separate joining entities
    # from creating member accounts.
    #
    echo "Creating the default node entity..."
    local createEntity=$($serum --config $CONFIG_FILE \
          registry create-entity \
          --registrar $registrar \
          --about "This the default entity all new members join." \
          --image-url " " \
          --name "Default" )

    local entity=$(echo $createEntity | jq .entity -r)

    #
    # Add the registry to the lockup program whitelist.
    #
    echo "Adding registry to the lockup whitelist..."
    $serum --config $CONFIG_FILE \
    lockup gov \
    --safe $safe \
    whitelist-add \
    --instance $registrar \
    --nonce $registrar_nonce \
    --program-id $registry_pid

    #
    # Log the generated TypeScript.
    #
    set +e
    read -r -d '' VAR << EOM
{
    srm: new PublicKey('${srm_mint}'),
    msrm: new PublicKey('${msrm_mint}'),

    srmFaucet: $srm_faucet,
    msrmFaucet: $msrm_faucet,

    god: new PublicKey('${god}'),
    megaGod: new PublicKey('${god_msrm}'),

    registryProgramId: new PublicKey(
      '${registry_pid}',
    ),
    lockupProgramId: new PublicKey(
      '${lockup_pid}',
    ),
    metaEntityProgramId: new PublicKey(
      '${meta_entity_pid}',
    ),

    registrar: new PublicKey('${registrar}'),
    rewardEventQueue: new PublicKey('${reward_q}'),
    safe: new PublicKey('${safe}'),

    defaultEntity: new PublicKey(
      '${entity}',
    ),
}
EOM
    echo $VAR
}

main
