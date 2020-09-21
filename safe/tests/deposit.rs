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
    // Given.
    //
    // An initialized safe.
    let common::lifecycle::Initialized {
        client,
        safe_account,
        safe_authority,
        safe_srm_vault,
        safe_srm_vault_authority,
        depositor,
        depositor_balance_before,
        srm_mint,
    } = common::lifecycle::initialize();

    // When.
    //
    // A depositor performs the vesting account deposit.
    let (vesting_account_kp, expected_beneficiary, expected_slots, expected_amounts) = {
        let deposit_accounts = [
            AccountMeta::new(depositor.pubkey(), false),
            AccountMeta::new(client.payer().pubkey(), true), // Owner of the depositor SPL account.
            AccountMeta::new(safe_srm_vault.pubkey(), false),
            AccountMeta::new(safe_account.pubkey(), false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
        ];
        let vesting_account_beneficiary = Keypair::generate(&mut OsRng);
        let vesting_slots = vec![11, 12, 13, 14, 15];
        let vesting_amounts = vec![1, 2, 3, 4, 5];
        let vesting_account_size = VestingAccount::data_size(vesting_slots.len());
        let (signature, keypair) = client
            .create_account_with_size_and_deposit_srm(
                vesting_account_size,
                &deposit_accounts,
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

    // Then.
    //
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
    // Then.
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
    // Then.
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
        // Sanity check the owner of the vault account.
        assert_eq!(safe_vault_spl_account.owner, safe_srm_vault_authority,);
    }
}
