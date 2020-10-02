use rand::rngs::OsRng;
use serum_common_tests::Genesis;
use serum_registry::accounts::{registry, Registry};
use serum_registry::accounts::{Entity, Stake, StakeKind};
use serum_registry::client::{Client, InitializeResponse};
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};
use solana_client_gen::solana_sdk::sysvar;
use spl_token::pack::Pack as TokenPack;
use std::str::FromStr;

mod common;

// lifecycle tests all instructions on the program in one go.
// TODO: break this up into multiple tests.
#[test]
fn lifecycle() {
    // First test initiailze.
    let Genesis {
        client,
        mint_authority,
        srm_mint,
        msrm_mint,
        god,
        god_msrm,
        god_balance_before,
        god_msrm_balance_before,
        god_owner,
    } = serum_common_tests::genesis::<Client>();

    // Initialize the registry.
    let registry_authority = Keypair::generate(&mut OsRng);
    let InitializeResponse {
        registry_acc,
        vault_acc,
        mega_vault_acc,
        vault_acc_authority,
        nonce,
        ..
    } = {
        client
            .create_all_accounts_and_initialize(
                &srm_mint.pubkey(),
                &msrm_mint.pubkey(),
                &registry_authority.pubkey(),
            )
            .unwrap()
    };

    // Verify initialization.
    {
        let registry: Registry =
            serum_common::client::rpc::account_unpacked(client.rpc(), &registry_acc.pubkey());
        assert_eq!(registry.initialized, true);
        assert_eq!(registry.mint, srm_mint.pubkey());
        assert_eq!(registry.mega_mint, msrm_mint.pubkey());
        assert_eq!(registry.nonce, nonce);
        assert_eq!(
            registry.capabilities,
            [Pubkey::new_from_array([0; 32]); registry::CAPABILITIES_LEN],
        );
        assert_eq!(registry.authority, registry_authority.pubkey());
        assert_eq!(registry.rewards, Pubkey::new_from_array([0; 32]));
        assert_eq!(
            registry.rewards_return_value,
            Pubkey::new_from_array([0; 32])
        );
    }

    // Register capabilities.
    {
        let capability_id = 1;
        let capability_program = Pubkey::new_rand();
        let accounts = [
            AccountMeta::new_readonly(registry_authority.pubkey(), true),
            AccountMeta::new(registry_acc.pubkey(), false),
        ];
        let signers = [&registry_authority, client.payer()];
        client
            .register_capability_with_signers(
                &signers,
                &accounts,
                capability_id,
                capability_program,
            )
            .unwrap();

        let registry: Registry =
            serum_common::client::rpc::account_unpacked(client.rpc(), &registry_acc.pubkey());
        let mut expected = [Pubkey::new_from_array([0u8; 32]); registry::CAPABILITIES_LEN];
        expected[capability_id as usize] = capability_program;
        assert_eq!(registry.capabilities, expected);
    }

    // Donate into the registry.
    {
        let donate_amount = 1234;
        let accounts = [
            AccountMeta::new_readonly(god_owner.pubkey(), true),
            AccountMeta::new(god.pubkey(), false),
            AccountMeta::new(vault_acc.pubkey(), false),
            AccountMeta::new_readonly(registry_acc.pubkey(), false),
            AccountMeta::new_readonly(spl_token::ID, false),
        ];
        client.donate(&accounts, 1234).unwrap();
        let vault: spl_token::state::Account =
            serum_common::client::rpc::account_token_unpacked(client.rpc(), &vault_acc.pubkey());
        assert_eq!(vault.state, spl_token::state::AccountState::Initialized);
        assert_eq!(vault.mint, srm_mint.pubkey());
        assert_eq!(vault.amount, donate_amount);
        assert_eq!(vault.owner, vault_acc_authority);
    }

    // Setup the rewards program for the registry.
    let (rewards_program_id, rewards_return_value) = {
        let rewards_program_id = std::env::var("TEST_REWARDS_PROGRAM_ID")
            .unwrap()
            .parse()
            .unwrap();
        let rewards_return_value = serum_common::client::rpc::create_account_rent_exempt(
            client.rpc(),
            client.payer(),
            serum_registry::rewards::RETURN_VALUE_SIZE,
            &rewards_program_id,
        )
        .unwrap();
        (rewards_program_id, rewards_return_value.pubkey())
    };
    {
        let accounts = [
            AccountMeta::new_readonly(registry_authority.pubkey(), true),
            AccountMeta::new(registry_acc.pubkey(), false),
        ];
        let signers = vec![&registry_authority, client.payer()];
        client
            .set_rewards_with_signers(
                &signers,
                &accounts,
                rewards_program_id,
                rewards_return_value,
            )
            .unwrap();
        let registry: Registry =
            serum_common::client::rpc::account_unpacked(client.rpc(), &registry_acc.pubkey());
        assert_eq!(registry.rewards, rewards_program_id);
        assert_eq!(registry.rewards_return_value, rewards_return_value);
    }

    // Create entity.
    let node_leader = Keypair::generate(&mut OsRng);
    let entity = {
        let entity_kp = Keypair::generate(&mut OsRng);
        let accounts = [
            AccountMeta::new(entity_kp.pubkey(), false),
            AccountMeta::new_readonly(node_leader.pubkey(), true),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ];

        let capabilities = 1;
        let stake_kind = StakeKind::Delegated;

        let (sig, entity) = client
            .create_account_and_create_entity_with_signers(
                // TODO: eliminate this clone here and in the macro.
                Keypair::from_bytes(&entity_kp.to_bytes()).unwrap(),
                &[&node_leader, &entity_kp, client.payer()],
                &accounts,
                capabilities,
                stake_kind,
            )
            .unwrap();

        let entity: Entity =
            serum_common::client::rpc::account_unpacked(client.rpc(), &entity_kp.pubkey());
        assert_eq!(entity.leader, node_leader.pubkey());
        assert_eq!(entity.initialized, true);
        assert_eq!(entity.amount, 0);
        assert_eq!(entity.mega_amount, 0);
        assert_eq!(entity.capabilities, capabilities);
        assert_eq!(entity.stake_kind, stake_kind);
        entity_kp.pubkey()
    };

    // Update entity.
    {
        let accounts = [
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(node_leader.pubkey(), true),
        ];

        let new_capabilities = 1 | 2;

        client
            .update_entity_with_signers(
                &[&node_leader, client.payer()],
                &accounts,
                new_capabilities,
            )
            .unwrap();

        let entity_account: Entity =
            serum_common::client::rpc::account_unpacked(client.rpc(), &entity);
        assert_eq!(entity_account.capabilities, new_capabilities);
    }

    // Stake SRM.
    let srm_stake_amount = 123;
    let beneficiary = Keypair::generate(&mut OsRng);
    let stake_kp = Keypair::generate(&mut OsRng);
    {
        let accounts = [
            AccountMeta::new(stake_kp.pubkey(), false),
            AccountMeta::new_readonly(god_owner.pubkey(), true),
            AccountMeta::new(god.pubkey(), false),
            AccountMeta::new_readonly(registry_acc.pubkey(), false),
            AccountMeta::new(vault_acc.pubkey(), false),
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(spl_token::ID, false),
        ];
        let signers = [&god_owner, &stake_kp, client.payer()];

        let is_mega = false;

        client
            .create_account_and_stake_with_signers(
                Keypair::from_bytes(&stake_kp.to_bytes()).unwrap(),
                &signers,
                &accounts,
                srm_stake_amount,
                beneficiary.pubkey(),
                is_mega,
            )
            .unwrap();

        let stake: Stake =
            serum_common::client::rpc::account_unpacked(client.rpc(), &stake_kp.pubkey());
        assert_eq!(stake.initialized, true);
        assert_eq!(stake.beneficiary, beneficiary.pubkey());
        assert_eq!(stake.entity_id, entity);
        assert_eq!(stake.amount, srm_stake_amount);
        assert_eq!(stake.mega_amount, 0);
    }

    // Stake MSRM.
    {
        let stake_kp = Keypair::generate(&mut OsRng);
        let accounts = [
            AccountMeta::new(stake_kp.pubkey(), false),
            AccountMeta::new_readonly(god_owner.pubkey(), true),
            AccountMeta::new(god_msrm.pubkey(), false),
            AccountMeta::new_readonly(registry_acc.pubkey(), false),
            AccountMeta::new(mega_vault_acc.pubkey(), false),
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(spl_token::ID, false),
        ];
        let signers = [&god_owner, &stake_kp, client.payer()];

        let msrm_stake_amount = 5;
        let beneficiary = Pubkey::new_rand();
        let is_mega = true;

        client
            .create_account_and_stake_with_signers(
                Keypair::from_bytes(&stake_kp.to_bytes()).unwrap(),
                &signers,
                &accounts,
                msrm_stake_amount,
                beneficiary,
                is_mega,
            )
            .unwrap();

        let stake: Stake =
            serum_common::client::rpc::account_unpacked(client.rpc(), &stake_kp.pubkey());
        assert_eq!(stake.initialized, true);
        assert_eq!(stake.beneficiary, beneficiary);
        assert_eq!(stake.entity_id, entity);
        assert_eq!(stake.amount, 0);
        assert_eq!(stake.mega_amount, msrm_stake_amount);

        let entity: Entity = serum_common::client::rpc::account_unpacked(client.rpc(), &entity);
        assert_eq!(entity.amount, srm_stake_amount);
        assert_eq!(entity.mega_amount, msrm_stake_amount);
    }

    // Collect rewards.
    {
        let beneficiary_tok_acc = serum_common::client::rpc::create_token_account(
            client.rpc(),
            &srm_mint.pubkey(),
            &beneficiary.pubkey(),
            client.payer(),
        )
        .unwrap();
        // Sanity check.
        {
            let beneficiary_tokens: spl_token::state::Account =
                serum_common::client::rpc::account_token_unpacked(
                    client.rpc(),
                    &beneficiary_tok_acc.pubkey(),
                );
            assert_eq!(beneficiary_tokens.amount, 0);
        }
        let accounts = [
            AccountMeta::new_readonly(rewards_program_id, false),
            AccountMeta::new(rewards_return_value, false),
            AccountMeta::new(vault_acc.pubkey(), false),
            AccountMeta::new_readonly(vault_acc_authority, false),
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(beneficiary_tok_acc.pubkey(), false),
            AccountMeta::new_readonly(stake_kp.pubkey(), false),
            AccountMeta::new_readonly(entity, false),
            AccountMeta::new_readonly(registry_acc.pubkey(), false),
            AccountMeta::new_readonly(spl_token::ID, false),
        ];
        let signers = [&beneficiary, client.payer()];
        client
            .collect_rewards_with_signers(&signers, &accounts)
            .unwrap();

        let beneficiary_tokens: spl_token::state::Account =
            serum_common::client::rpc::account_token_unpacked(
                client.rpc(),
                &beneficiary_tok_acc.pubkey(),
            );
        // 100 is currently hard coded into the rewards contract.
        assert_eq!(beneficiary_tokens.amount, 100);
    }
}
