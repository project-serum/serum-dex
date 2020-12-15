#!/bin/bash
# shellcheck disable=SC2155

################################################################################
#
# Does deployment + initialization of all programs and accounts needed to run
# the staking + lockup application.
#
################################################################################

CLUSTER=l
#CLUSTER=devnet
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

CONFIG_FILE=~/.config/serum/cli/dev.yaml
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
    make -s -C lockup build
    make -s -C registry build
    make -s -C registry/meta-entity build
    make -s -C registry/rewards build
    ./do.sh build dex

    #
    # Deploy all the programs.
    #
    local pids=$(make -s -C registry deploy-all)
    local rewards_pids=$(make -s -C registry/rewards deploy-all)

    local registry_pid=$(echo $pids | jq .registryProgramId -r)
    local lockup_pid=$(echo $pids | jq .lockupProgramId -r)
    local meta_entity_pid=$(echo $pids | jq .metaEntityProgramId -r)
    local dex_pid=$(echo $rewards_pids | jq .dexProgramId -r)
    local rewards_pid=$(echo $rewards_pids | jq .rewardsProgramId -r)

    #
    # Generate genesis state.
    #
    local genesis=$($serum dev init-mint)

    local srm_mint=$(echo $genesis | jq .srmMint -r)
    local msrm_mint=$(echo $genesis | jq .msrmMint -r)
    local god=$(echo $genesis | jq .god -r)
    local god_msrm=$(echo $genesis | jq .godMsrm -r)

    #
    # Write out the CLI configuration file.
    #
    mkdir -p $(dirname $CONFIG_FILE)
    cat << EOM > $CONFIG_FILE
---
network:
  cluster: $CLUSTER

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

    local lInit=$($serum --config $CONFIG_FILE \
          lockup initialize)

    local safe=$(echo $lInit | jq .safe -r)

    #
    # Initialize a node entity. Hack until we separate joining entities
    # from creating member accounts.
    #
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
    read -r -d '' VAR << EOM
{
    srm: new PublicKey('${srm_mint}'),
    msrm: new PublicKey('${msrm_mint}'),
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
