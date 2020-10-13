use rand::rngs::OsRng;
use serum_common_tests::Genesis;
use serum_registry::accounts::Registrar;
use serum_registry::accounts::{Entity, Member, StakeKind};
use serum_registry::client::Client;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};
use solana_client_gen::solana_sdk::sysvar;

mod common;

// NOTE: Deterministic derived addresses are used as a UX convenience so
//       make sure tests are run against a new instance of the program.

// lifecycle tests all instructions on the program in one go.
// TODO: break this up into multiple tests.
#[test]
fn lifecycle() {
    // First test initiailze.
    let genesis = serum_common_tests::genesis::<Client>();

    let Genesis {
        client,
        srm_mint: _,
        msrm_mint: _,
        mint_authority: _,
        god: _,
        god_msrm: _,
        god_balance_before: _,
        god_msrm_balance_before: _,
        god_owner: _,
    } = genesis;

    // Initialize the registrar.
    let withdrawal_timelock = 1234;
    let registrar_authority = Keypair::generate(&mut OsRng);
    let accounts = [AccountMeta::new_readonly(sysvar::rent::ID, false)];
    let (_tx_sig, registrar) = client
        .create_account_and_initialize(&accounts, registrar_authority.pubkey(), withdrawal_timelock)
        .unwrap();

    // Verify initialization.
    {
        let registrar: Registrar =
            serum_common::client::rpc::account_unpacked(client.rpc(), &registrar.pubkey());
        assert_eq!(registrar.initialized, true);
        assert_eq!(registrar.authority, registrar_authority.pubkey());
        assert_eq!(registrar.capabilities_fees_bps, [0; 32]);
    }

    // Register capabilities.
    {
        let capability_id = 1;
        let capability_fee = 1234;
        let accounts = [
            AccountMeta::new_readonly(registrar_authority.pubkey(), true),
            AccountMeta::new(registrar.pubkey(), false),
        ];
        let signers = [&registrar_authority, client.payer()];
        client
            .register_capability_with_signers(&signers, &accounts, capability_id, capability_fee)
            .unwrap();

        let registrar: Registrar =
            serum_common::client::rpc::account_unpacked(client.rpc(), &registrar.pubkey());
        let mut expected = [0; 32];
        expected[capability_id as usize] = capability_fee;
        assert_eq!(registrar.capabilities_fees_bps, expected);
    }

    // Create entity.
    let node_leader = Keypair::generate(&mut OsRng);
    let node_leader_pubkey = node_leader.pubkey();
    let entity = {
        let capabilities = 1;
        let stake_kind = StakeKind::Delegated;

        let (_tx_sig, entity_addr) = client
            .create_entity_derived(&node_leader, capabilities, stake_kind)
            .unwrap();

        let entity: Entity =
            serum_common::client::rpc::account_unpacked(client.rpc(), &entity_addr);
        assert_eq!(entity.leader, node_leader_pubkey);
        assert_eq!(entity.initialized, true);
        assert_eq!(entity.amount, 0);
        assert_eq!(entity.mega_amount, 0);
        assert_eq!(entity.capabilities, capabilities);
        assert_eq!(entity.stake_kind, stake_kind);

        entity_addr
    };

    // Update entity.
    {
        let accounts = [
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(node_leader.pubkey(), true),
        ];

        let new_capabilities = 1 | 2;
        let new_leader = Pubkey::new_rand();

        client
            .update_entity_with_signers(
                &[&node_leader, client.payer()],
                &accounts,
                new_leader.clone(),
                new_capabilities,
            )
            .unwrap();

        let entity_account: Entity =
            serum_common::client::rpc::account_unpacked(client.rpc(), &entity);
        assert_eq!(entity_account.capabilities, new_capabilities);
        assert_eq!(entity_account.leader, new_leader);
    }

    // Join enitty.
    let beneficiary = Keypair::generate(&mut OsRng);
    {
        let delegate = Pubkey::new_from_array([0; 32]);
        let (_tx_sig, member_addr) = client
            .join_entity_derived(entity, beneficiary.pubkey(), delegate)
            .unwrap();

        let member: Member =
            serum_common::client::rpc::account_unpacked(client.rpc(), &member_addr);
        assert_eq!(member.initialized, true);
        assert_eq!(member.entity, entity);
        assert_eq!(member.beneficiary, beneficiary.pubkey());
        assert_eq!(member.delegate, Pubkey::new_from_array([0; 32]));
        assert_eq!(member.amount, 0);
        assert_eq!(member.mega_amount, 0);
    }
}
