use serum_safe::accounts::{LsrmReceipt, VestingAccount};
use serum_safe::pack::DynPack;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::signature::{Keypair, Signature, Signer};
use solana_client_gen::solana_sdk::sysvar;
use spl_token::option::COption;
use spl_token::pack::Pack;
use spl_token::state::Mint;

use rand::rngs::OsRng;

mod common;

#[test]
fn mint_lsrm() {
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
        ..
    } = common::lifecycle::deposit();

    // When.
    //
    // I mint locked srm.
    let nft_count = 2;
    let lsrm_nfts = {
        let mut mint_lsrm_accounts = vec![
            AccountMeta::new(vesting_account_beneficiary.pubkey(), true),
            AccountMeta::new(vesting_account, false),
            AccountMeta::new(safe_account, false),
            AccountMeta::new(spl_token::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ];
        let mut signers = vec![&vesting_account_beneficiary, client.payer()];
        let (_sig, lsrm_nfts) = client
            .create_nfts_and_mint_locked_srm_with_signers(nft_count, signers, mint_lsrm_accounts)
            .unwrap();
        lsrm_nfts
    };

    // Then.
    //
    // The lsrm nft mints should be initialized.
    {
        let lsrm_nft_mints = lsrm_nfts.iter().map(|lsrm| {
            let account = client
                .rpc()
                .get_account_with_commitment(&lsrm.mint.pubkey(), CommitmentConfig::recent())
                .unwrap()
                .value
                .unwrap();
            Mint::unpack_unchecked(&account.data).unwrap()
        });

        for mint in lsrm_nft_mints {
            assert!(mint.is_initialized);
            assert_eq!(mint.mint_authority, COption::Some(*client.program()));
            assert_eq!(mint.supply, 0);
            assert_eq!(mint.decimals, 0);
            assert_eq!(mint.freeze_authority, COption::None);
        }
    }
    // Then.
    //
    // The lsrm receipts should be initialized.
    {
        let lsrm_receipts = lsrm_nfts.iter().map(|lsrm| {
            let account = client
                .rpc()
                .get_account_with_commitment(&lsrm.receipt, CommitmentConfig::recent())
                .unwrap()
                .value
                .unwrap();
            LsrmReceipt::unpack(&account.data).unwrap()
        });
        for (idx, receipt) in lsrm_receipts.enumerate() {
            assert!(receipt.initialized);
            assert_eq!(receipt.mint, lsrm_nfts[idx].mint.pubkey());
            assert_eq!(receipt.vesting_account, vesting_account);
            assert_eq!(receipt.burned, false);
        }
    }
    // Then.
    //
    // The vesting accounts should be updated.
    {
        let updated_vesting_account = {
            let account = client
                .rpc()
                .get_account_with_commitment(&vesting_account, CommitmentConfig::recent())
                .unwrap()
                .value
                .unwrap();
            VestingAccount::unpack(&account.data).unwrap()
        };

        // The field we care about.
        assert_eq!(updated_vesting_account.locked_outstanding, nft_count as u64);
        // Sanity check the rest.
        assert_eq!(updated_vesting_account.safe, safe_account);
        assert_eq!(
            updated_vesting_account.beneficiary,
            vesting_account_beneficiary.pubkey()
        );
        assert_eq!(updated_vesting_account.initialized, true);
        // Check slots.
        let matching = updated_vesting_account
            .slots
            .iter()
            .zip(&vesting_account_slots)
            .filter(|&(a, b)| a == b)
            .count();
        assert_eq!(vesting_account_slots.len(), matching);
        assert_eq!(
            vesting_account_slots.len(),
            updated_vesting_account.slots.len()
        );
        // Check amounts
        let matching = updated_vesting_account
            .amounts
            .iter()
            .zip(&vesting_account_amounts)
            .filter(|&(a, b)| a == b)
            .count();
        assert_eq!(vesting_account_amounts.len(), matching);
        assert_eq!(
            vesting_account_amounts.len(),
            updated_vesting_account.amounts.len()
        );
    }
}
