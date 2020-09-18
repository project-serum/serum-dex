extern crate rand;
extern crate serum_common;
extern crate serum_safe;
extern crate solana_transaction_status;

use rand::rngs::OsRng;
use serum_safe::accounts::{SafeAccount, SrmVault, VestingAccount, Whitelist};
use serum_safe::client::{Client, ClientError, RequestOptions};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signature::{Keypair, Signature, Signer};
use solana_transaction_status::UiTransactionEncoding;
use spl_token::pack::Pack;
use std::error::Error;
use std::str::FromStr;

mod common;

#[test]
fn initialized() {
    // Given.
    let common::lifecycle::Genesis {
        client,
        mint_authority,
        srm_mint,
        god,
        god_balance_before,
    } = common::lifecycle::genesis();

    // When.
    //
    // I create the safe account and initialize it.
    let safe_authority = Keypair::generate(&mut OsRng);
    let rent_account = AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false);
    let accounts = vec![rent_account];
    let (_signature, safe_account) = client
        .create_account_and_initialize(&accounts, srm_mint.pubkey(), safe_authority.pubkey())
        .unwrap();

    // Then.
    let account = client
        .rpc()
        .get_account_with_commitment(&safe_account.pubkey(), CommitmentConfig::recent())
        .unwrap()
        .value
        .unwrap();
    let safe_account = SafeAccount::unpack_from_slice(&account.data).unwrap();
    assert_eq!(&account.owner, client.program());
    assert_eq!(account.data.len(), SafeAccount::SIZE);
    assert_eq!(safe_account.authority, safe_authority.pubkey());
    assert_eq!(safe_account.supply, 0);
    assert_eq!(safe_account.is_initialized, true);
    assert_eq!(safe_account.whitelist, Whitelist::zeroed());
}

#[test]
fn initialized_already() {
    let client = common::client();
    let safe_authority = Keypair::generate(&mut OsRng);

    // Create the safe account and initialize it.
    let srm_mint = Keypair::generate(&mut OsRng);
    let rent_account = AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false);
    let accounts = vec![rent_account.clone()];
    let (signature, safe_account) = client
        .create_account_and_initialize(
            &accounts.clone(),
            srm_mint.pubkey(),
            safe_authority.pubkey(),
        )
        .unwrap();

    // Now try to initialize it for second time.
    let accounts_again = vec![AccountMeta::new(safe_account.pubkey(), false), rent_account];
    let new_safe_authority = Keypair::generate(&mut OsRng);
    let signature = match client.initialize(
        &accounts_again,
        srm_mint.pubkey(),
        new_safe_authority.pubkey(),
    ) {
        Ok(_) => panic!("transaction should fail"),
        Err(e) => {
            let expected: u32 = SafeErrorCode::AlreadyInitialized.into();
            assert_eq!(e.error_code().unwrap(), expected);
        }
    };
}

#[test]
fn initialized_not_rent_exempt() {
    // TODO: nice to have.
    //
    // Test initialization with a non rent exempt safe account.
}

#[test]
fn initialized_program_not_safe_account_owner() {
    // TODO: nice to have.
    //
    // Test initialization with a safe account that's not owned
    // by the program.
}
