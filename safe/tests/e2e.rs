extern crate rand;
extern crate serum_common;
extern crate serum_safe;
extern crate solana_transaction_status;

use rand::rngs::OsRng;
use serum_common::Cluster;
use serum_safe::accounts::SafeAccount;
use serum_safe::client::{Client, ClientError, RequestOptions};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signature::{Keypair, Signature, Signer};
use solana_transaction_status::UiTransactionEncoding;
use spl_token::pack::Pack;
use std::str::FromStr;

// All tests assume:
//
// * The payer with the client is already funded.
// * The program with the client is already deployed. x
//

#[test]
fn initialized() {
    let client = client();
    let safe_authority = Keypair::generate(&mut OsRng);

    // Create the safe account and initialize it.
    let rent_account = AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false);
    let accounts = vec![rent_account];
    let (signature, safe_account) = client
        .create_account_and_initialize(&accounts, safe_authority.pubkey())
        .unwrap();

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
}

use std::error::Error;

#[test]
fn initialized_already() {
    let client = client();
    let safe_authority = Keypair::generate(&mut OsRng);

    // Create the safe account and initialize it.
    let rent_account = AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false);
    let accounts = vec![rent_account.clone()];
    let (signature, safe_account) = client
        .create_account_and_initialize(&accounts.clone(), safe_authority.pubkey())
        .unwrap();

    // Now try to initialize it for second time.
    let accounts_again = vec![AccountMeta::new(safe_account.pubkey(), false), rent_account];
    let new_safe_authority = Keypair::generate(&mut OsRng);
    let signature = match client.initialize(&accounts_again, new_safe_authority.pubkey()) {
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

fn client() -> Client {
    let program_id = std::env::var("TEST_PROGRAM_ID").unwrap().parse().unwrap();
    let payer_filepath = std::env::var("TEST_PAYER_FILEPATH").unwrap().clone();
    let cluster: Cluster = std::env::var("TEST_CLUSTER_URL").unwrap().parse().unwrap();

    Client::from_keypair_file(program_id, &payer_filepath, cluster.url())
        .expect("invalid keypair file")
        .with_options(RequestOptions {
            commitment: CommitmentConfig::single(),
            tx: RpcSendTransactionConfig {
                skip_preflight: true,
                preflight_commitment: None,
            },
        })
}
