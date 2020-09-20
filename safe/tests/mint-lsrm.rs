extern crate rand;

use serum_safe::accounts::{LsrmReceipt, SafeAccount};
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::signature::{Keypair, Signature, Signer};
use solana_client_gen::solana_sdk::sysvar;
use spl_token::pack::Pack;

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
        safe_account,
    } = common::lifecycle::deposit();

    // When.
    //
    // I mint locked srm.
    let lsrm_nft_mint_keys = {
        let mut mint_lsrm_accounts = vec![
            AccountMeta::new(vesting_account_beneficiary.pubkey(), true),
            AccountMeta::new(vesting_account, false),
            AccountMeta::new(safe_account, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ];
        let mut signers = vec![&vesting_account_beneficiary, client.payer()];
        let nft_count = 2;
        let (_sig, lsrm_nft_mint_keys) = client
            .create_nfts_and_mint_locked_srm_with_signers(nft_count, signers, mint_lsrm_accounts)
            .unwrap();
        lsrm_nft_mint_keys
    };
}
