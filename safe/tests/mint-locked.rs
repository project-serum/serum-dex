use common::assert::assert_eq_vec;
use serum_common::pack::Pack;
use serum_safe::accounts::{MintReceipt, Vesting};
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::signature::Signer;
use solana_client_gen::solana_sdk::sysvar;
use spl_token::option::COption;
use spl_token::pack::Pack as TokenUnpack;
use spl_token::state::Mint;

mod common;

#[test]
fn mint() {
    // Given.
    //
    // An initialized Serum Safe with deposit.
    let common::lifecycle::Deposited {
        client,
        vesting_acc,
        vesting_acc_beneficiary,
        vesting_acc_slots,
        vesting_acc_amounts,
        safe_acc,
        safe_srm_vault_authority,
        ..
    } = common::lifecycle::deposit();

    // When.
    //
    // I mint locked srm.
    let nft_token_acc_owner = vesting_acc_beneficiary.pubkey();
    let nft_count = 1; // todo: bump this
    let lsrm_nfts = {
        let mint_lsrm_accs = vec![
            AccountMeta::new(vesting_acc_beneficiary.pubkey(), true),
            AccountMeta::new(vesting_acc, false),
            AccountMeta::new_readonly(safe_acc, false),
            AccountMeta::new(safe_srm_vault_authority, false),
            AccountMeta::new(spl_token::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ];
        let signers = vec![&vesting_acc_beneficiary, client.payer()];
        let (_sig, lsrm_nfts) = client
            .create_nfts_and_mint_locked_with_signers(
                nft_count,
                &nft_token_acc_owner,
                signers,
                mint_lsrm_accs,
            )
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
            assert_eq!(mint.mint_authority, COption::None);
            assert_eq!(mint.supply, 1);
            assert_eq!(mint.decimals, 0);
            assert_eq!(mint.freeze_authority, COption::None);
        }
    }
    // Then.
    //
    // The lsrm nft token accounts should be initialized.
    {
        let token_accs = lsrm_nfts.iter().map(|lsrm| {
            let account = serum_common::client::rpc::account_token_unpacked::<
                spl_token::state::Account,
            >(client.rpc(), &lsrm.token_acc.pubkey());
            (lsrm, account)
        });
        for (lsrm, ta) in token_accs {
            assert_eq!(ta.state, spl_token::state::AccountState::Initialized);
            assert_eq!(ta.owner, nft_token_acc_owner);
            assert_eq!(ta.mint, lsrm.mint.pubkey());
            assert_eq!(ta.amount, 1);
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
            (MintReceipt::unpack(&account.data).unwrap(), lsrm)
        });
        for (idx, (receipt, lsrm)) in lsrm_receipts.enumerate() {
            assert!(receipt.initialized);
            assert_eq!(receipt.mint, lsrm_nfts[idx].mint.pubkey());
            assert_eq!(receipt.vesting_acc, vesting_acc);
            assert_eq!(receipt.burned, false);
            assert_eq!(receipt.token_acc, lsrm.token_acc.pubkey());
        }
    }
    // Then.
    //
    // The vesting accounts should be updated.
    {
        let updated_vesting_acc = {
            let account = client
                .rpc()
                .get_account_with_commitment(&vesting_acc, CommitmentConfig::recent())
                .unwrap()
                .value
                .unwrap();
            Vesting::unpack(&account.data).unwrap()
        };

        // The field we care about.
        assert_eq!(updated_vesting_acc.locked_outstanding, nft_count as u64);
        // Sanity check the rest.
        assert_eq!(updated_vesting_acc.safe, safe_acc);
        assert_eq!(
            updated_vesting_acc.beneficiary,
            vesting_acc_beneficiary.pubkey()
        );
        assert_eq!(updated_vesting_acc.initialized, true);
        assert_eq_vec(updated_vesting_acc.slots, vesting_acc_slots);
        assert_eq_vec(updated_vesting_acc.amounts, vesting_acc_amounts);
    }
}
