extern crate rand;
extern crate serum_common;
extern crate serum_safe;
extern crate solana_transaction_status;

use rand::rngs::OsRng;
use serum_safe::accounts::{SafeAccount, SrmVault, VestingAccount};
use serum_safe::client::{Client, ClientError, RequestOptions};
use serum_safe::error::{SafeError, SafeErrorCode};
use serum_safe::pack::DynPack;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signature::{Keypair, Signature, Signer};
use solana_transaction_status::UiTransactionEncoding;
use spl_token::pack::Pack;
use std::error::Error;

mod common;

#[test]
fn deposit_srm() {
    let client = common::client();

    // Setup.
    //
    // Initialize the SPL token representing SRM.
    let mint_authority = client.payer().clone();
    let coin_mint = Keypair::generate(&mut OsRng);
    let _ = serum_common::rpc::genesis(
        client.rpc(),
        client.payer(),
        &coin_mint,
        &mint_authority.pubkey(),
        3,
    )
    .unwrap();

    // Setup.
    //
    // Initialize the Safe.
    let safe_authority = Keypair::generate(&mut OsRng);
    let rent_account = AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false);
    let init_accounts = vec![rent_account];
    let (_signature, safe_account) = client
        .create_account_and_initialize(&init_accounts, coin_mint.pubkey(), safe_authority.pubkey())
        .unwrap();

    // Setup.
    //
    // Create a funded SRM SPL account representing the depositor allocating
    // vesting accounts.
    let depositor_balance_before = 1_000_000;
    let depositor = serum_common::rpc::mint_to_new_account(
        client.rpc(),
        client.payer(),
        &mint_authority,
        &coin_mint.pubkey(),
        depositor_balance_before,
    )
    .unwrap();

    // Setup.
    //
    // Create an SPL account representing the Safe program's vault.
    let safe_srm_vault = {
        let safe_srm_vault_program_derived_address =
            SrmVault::program_derived_address(client.program(), &safe_account.pubkey());
        let safe_srm_vault = serum_common::rpc::create_spl_account(
            client.rpc(),
            &coin_mint.pubkey(),
            &safe_srm_vault_program_derived_address,
            client.payer(),
        )
        .unwrap();

        // Ensure the safe_srm_vault has 0 SRM before the deposit.
        let safe_srm_vault_account = {
            let account = client
                .rpc()
                .get_account_with_commitment(&safe_srm_vault.pubkey(), CommitmentConfig::recent())
                .unwrap()
                .value
                .unwrap();
            spl_token::state::Account::unpack_from_slice(&account.data).unwrap()
        };
        assert_eq!(safe_srm_vault_account.mint, coin_mint.pubkey());
        assert_eq!(
            safe_srm_vault_account.owner,
            safe_srm_vault_program_derived_address
        );
        assert_eq!(safe_srm_vault_account.amount, 0);

        safe_srm_vault
    };

    // Finally, perform the vesting account deposit.
    let (vesting_account_kp, expected_beneficiary, expected_slots, expected_amounts) = {
        let deposit_accounts = vec![
            AccountMeta::new(depositor.pubkey(), false),
            AccountMeta::new(client.payer().pubkey(), true), // Owner of the depositor SPL account.
            AccountMeta::new(safe_srm_vault.pubkey(), false),
            AccountMeta::new(safe_account.pubkey(), false),
            AccountMeta::new(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
        ];
        let vesting_account_beneficiary = Keypair::generate(&mut OsRng);
        let vesting_slots = vec![11, 12, 13, 14, 15];
        let vesting_amounts = vec![1, 2, 3, 4, 5];
        let vesting_account_size = VestingAccount::data_size(vesting_slots.len());
        let (signature, keypair) = client
            .create_account_with_size_and_deposit_srm(
                vesting_account_size,
                deposit_accounts.as_slice(),
                vesting_account_beneficiary.pubkey(),
                vesting_slots.clone(),
                vesting_amounts.clone(),
            )
            .unwrap();
        (
            keypair,
            vesting_account_beneficiary,
            vesting_slots,
            vesting_amounts,
        )
    };

    // Read the state of the program and ensure it's correct.
    //
    // Check.
    //
    // The vesting account is setup properly.
    {
        let vesting_account = {
            let account = client
                .rpc()
                .get_account_with_commitment(
                    &vesting_account_kp.pubkey(),
                    CommitmentConfig::recent(),
                )
                .unwrap()
                .value
                .unwrap();
            VestingAccount::unpack(&account.data).unwrap()
        };
        assert_eq!(vesting_account.safe, safe_account.pubkey());
        assert_eq!(vesting_account.beneficiary, expected_beneficiary.pubkey());
        assert_eq!(vesting_account.initialized, true);
        let matching = vesting_account
            .slots
            .iter()
            .zip(&expected_slots)
            .filter(|&(a, b)| a == b)
            .count();
        assert_eq!(vesting_account.slots.len(), matching);
        assert_eq!(vesting_account.slots.len(), expected_slots.len());
        let matching = vesting_account
            .amounts
            .iter()
            .zip(&expected_amounts)
            .filter(|&(a, b)| a == b)
            .count();
        assert_eq!(vesting_account.amounts.len(), matching);
        assert_eq!(vesting_account.amounts.len(), expected_slots.len());
    }
    // Check.
    //
    // The depositor's SPL token account has funds reduced.
    {
        let depositor_spl_account = {
            let account = client
                .rpc()
                .get_account_with_commitment(&depositor.pubkey(), CommitmentConfig::recent())
                .unwrap()
                .value
                .unwrap();
            spl_token::state::Account::unpack_from_slice(&account.data).unwrap()
        };
        let expected_balance = depositor_balance_before - expected_amounts.iter().sum::<u64>();
        assert_eq!(depositor_spl_account.amount, expected_balance,);
    }
    // Check.
    //
    // The program-owned SPL token vault has funds increased.
    {
        let safe_vault_spl_account = {
            let account = client
                .rpc()
                .get_account_with_commitment(&safe_srm_vault.pubkey(), CommitmentConfig::recent())
                .unwrap()
                .value
                .unwrap();
            spl_token::state::Account::unpack_from_slice(&account.data).unwrap()
        };
        let expected_balance = expected_amounts.iter().sum::<u64>();
        assert_eq!(safe_vault_spl_account.amount, expected_balance);
    }
}
