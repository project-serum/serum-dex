use serum_safe::accounts::{SrmVault, VestingAccount};
use serum_safe::pack::DynPack;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::signature::Signer;
use solana_client_gen::solana_sdk::sysvar;
use spl_token::pack::Pack;

mod common;

#[test]
fn withdraw() {
    return;
    // Given.
    //
    // An initialized Serum Safe with deposit.
    let common::lifecycle::Deposited {
        client,
        vesting_account,
        vesting_account_beneficiary,
        vesting_account_slots,
        vesting_account_amounts,
        safe_account,
        safe_srm_vault,
        safe_srm_vault_authority,
        srm_mint,
    } = common::lifecycle::deposit_with_schedule(vec![1, 100_000, 200_000], vec![30, 40, 50]);
    // And.
    //
    // An empty SRM SPL token account.
    let beneficiary_srm_spl_account = serum_common_client::rpc::create_spl_account(
        client.rpc(),
        &srm_mint.pubkey(),
        &vesting_account_beneficiary.pubkey(),
        client.payer(),
    )
    .unwrap();

    // When.
    //
    // The vesting period passes.
    common::blockchain::pass_time(client.rpc(), vesting_account_slots[0]);
    // And.
    //
    // I withdraw from the vesting account *to* the empty SPL token account.
    let expected_withdraw_amount = {
        let accounts = [
            AccountMeta::new_readonly(vesting_account_beneficiary.pubkey(), true),
            AccountMeta::new(vesting_account, false),
            AccountMeta::new(beneficiary_srm_spl_account.pubkey(), false),
            AccountMeta::new(safe_srm_vault.pubkey(), false),
            AccountMeta::new_readonly(safe_srm_vault_authority, false),
            AccountMeta::new(safe_account, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(sysvar::clock::ID, false),
        ];
        let signers = [&vesting_account_beneficiary, client.payer()];
        let withdraw_amount = 20;
        client
            .withdraw_srm_with_signers(&signers, &accounts, withdraw_amount)
            .unwrap();
        withdraw_amount
    };

    // Then.
    //
    // I have a balance in that account.
    {
        let spl_account = {
            let account = client
                .rpc()
                .get_account_with_commitment(
                    &beneficiary_srm_spl_account.pubkey(),
                    CommitmentConfig::recent(),
                )
                .unwrap()
                .value
                .unwrap();
            spl_token::state::Account::unpack_from_slice(&account.data).unwrap()
        };
        assert_eq!(spl_account.amount, expected_withdraw_amount);
    }
    // Then.
    //
    // My vesting account is updated.
    {
        let vesting_account = {
            let account = client
                .rpc()
                .get_account_with_commitment(&vesting_account, CommitmentConfig::recent())
                .unwrap()
                .value
                .unwrap();
            VestingAccount::unpack(&account.data).unwrap()
        };
        assert_eq!(vesting_account.amounts[0], 10);
        assert_eq!(vesting_account.amounts[1], 40);
        assert_eq!(vesting_account.amounts[2], 50);
    }
}

// TODO
#[test]
fn withdraw_some_when_locked_srm_outstanding() {
    // Given.
    //
    // A vesting account with outstanding lSRM.
    let common::lifecycle::LsrmMinted {
        client,
        lsrm,
        vesting_account,
        vesting_account_beneficiary,
        srm_mint,
    } = common::lifecycle::mint_lsrm(2);

    // When.
    //
    // I withdraw some of the vested amount after the vesting period.

    // Then.
    //
    // I should have SRM in my account.

    // Then.
    //
    // My vesting account should be updated.
}

// TODO
#[test]
fn withdraw_all_when_locked_srm_outstanding() {
    // Given.
    //
    // A vesting account with outstanding lSRM.
    let common::lifecycle::LsrmMinted {
        client,
        lsrm,
        vesting_account,
        vesting_account_beneficiary,
        srm_mint,
    } = common::lifecycle::mint_lsrm(2);

    // When.
    //
    // I withdraw the entire vested amount after the vesting period.

    // Then.
    //
    // I should *not* have SRM in my account.

    // Then.
    //
    // My vesting account should not be updated.
}

// TODO
#[test]
fn withdraw_more_than_vested() {
    // Given.
    //
    // A vesting account.
    let common::lifecycle::LsrmMinted {
        client,
        lsrm,
        vesting_account,
        vesting_account_beneficiary,
        srm_mint,
    } = common::lifecycle::mint_lsrm(2);

    // When.
    //
    // I withdraw more than has vested.

    // Then.
    //
    // I should *not* have SRM in my account.

    // Then.
    //
    // My vesting account should not be updated.
}
