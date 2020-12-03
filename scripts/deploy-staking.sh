#!/bin/bash

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
# 100_000_000 million SRM (6 decimals)
MAX_STAKE_PER_ENTITY=100000000000000
# 1 SRM to stake.
STAKE_RATE=1000000
# 1 MSRM to stake.
STAKE_RATE_MEGA=1
REWARD_ACTIVATION_THRESHOLD=1

main() {
    # First generate the genesis state, with the SRM/MSRM mints and
    # funded wallet (at ~/.config/solana/id.json).
    #
    # Example `genesis` var:
    #
    # Genesis {
    #     wallet: FhmUh2PEpTzUwBWPt4qgDBeqfmb2ES3T64CkT1ZiktSS,
    #     mint_authority: FhmUh2PEpTzUwBWPt4qgDBeqfmb2ES3T64CkT1ZiktSS,
    #     god_owner: FhmUh2PEpTzUwBWPt4qgDBeqfmb2ES3T64CkT1ZiktSS,
    #     srm_mint: E7ScVS17ak1ZVy9nNyGsVqZ48QdcDgdxSk1wXfD8zW3o,
    #     msrm_mint: 4ozqYu5Qjz8W9hfqXDA4XZNdEARCHuWGU4v8eSYX1XDQ,
    #     god: HWT4vz4u2KdkimMDoMS96HSeJLGTpfzScU44PKWpG7D,
    #     god_msrm: 7J2HeEnbfugJN8uiyPrczk8rWMQG6gVBF1zE1g4gdyqZ,
    #     god_balance_before: 1000000000000000,
    #     god_msrm_balance_before: 1000000000000000,
    # }
    #
    local genesis=$(cargo run -p serum-node -- -c $CLUSTER dev init-mint)
    local srm_mint=$(echo $genesis | sed 's/.*{.* srm_mint: \(.*\),.*msrm_mint.*}.*/\1/g')
    local msrm_mint=$(echo $genesis | sed 's/.*{.* msrm_mint: \(.*\),.*god:.*}.*/\1/g')
    local god=$(echo $genesis | sed 's/.*{.* god: \(.*\),.*god_msrm:.*}.*/\1/g')
    local god_msrm=$(echo $genesis | sed 's/.*{.* god_msrm: \(.*\),.*god_balance_before:.*}.*/\1/g')

    #
    # Deploy all the programs.
    #
    make -C lockup build
    make -C registry build
    make -C registry/meta-entity build
    pids=$(make -s -C registry deploy-all)
    registry_pid=$(echo $pids | jq .registryProgramId -r)
    lockup_pid=$(echo $pids | jq .lockupProgramId -r)
    meta_entity_pid=$(echo $pids | jq .metaEntityProgramId -r)

    #
    # Now intialize all the accounts.
    #
    local rInit=$(cargo run -p serum-node -- \
          -c $CLUSTER \
          --srm-mint $srm_mint \
          --msrm-mint $msrm_mint \
          registry --pid $registry_pid \
          init \
          --deactivation-timelock $DEACTIVATION_TIMELOCK \
          --reward-activation-threshold $REWARD_ACTIVATION_THRESHOLD \
          --withdrawal-timelock $WITHDRAWAL_TIMELOCK \
          --max-stake-per-entity $MAX_STAKE_PER_ENTITY \
					--stake-rate $STAKE_RATE \
					--stake-rate-mega $STAKE_RATE_MEGA)
    local registrar=$(echo $rInit | jq .registrar -r)
		local registrar_nonce=$(echo $rInit | jq .nonce -r)
		local reward_q=$(echo $rInit | jq .rewardEventQueue -r)

    local lInit=$(cargo run -p serum-node -- \
          -c $CLUSTER \
          --srm-mint $srm_mint \
          --msrm-mint $msrm_mint \
          lockup --pid $lockup_pid \
          initialize)
    local safe=$(echo $lInit | jq .safe -r)

    #
    # Initialize a node entity. Hack until we separate joining entities
    # from creating member accounts.
    #
    local createEntity=$(cargo run -p serum-node -- \
          -c $CLUSTER \
          --srm-mint $srm_mint \
          --msrm-mint $msrm_mint \
          registry --pid $registry_pid \
          create-entity \
          --leader ~/.config/solana/id.json \
          --registrar $registrar \
          --about "This the default entity all new members join." \
          --image-url " " \
          --name "Default" \
          --meta-entity-program-id $meta_entity_pid)

    local entity=$(echo $createEntity | jq .entity -r)

    #
    # Add the registry to the lockup program whitelist.
    #
    cargo run -p serum-node -- \
    -c $CLUSTER \
    --srm-mint $srm_mint \
    --msrm-mint $msrm_mint \
    lockup --pid $lockup_pid \
    gov \
    --safe $safe \
    whitelist-add \
    --instance $registrar \
    --nonce $registrar_nonce \
    --program-id $registry_pid

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
