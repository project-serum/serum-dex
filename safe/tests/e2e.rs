extern crate rand;
extern crate serum_common;
extern crate serum_safe;
extern crate solana_sdk;
extern crate solana_transaction_status;

use rand::rngs::OsRng;
use serum_common::Cluster;
use serum_safe::client::{Client, ClientError, RequestOptions};
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signature, Signer};
use solana_transaction_status::UiTransactionEncoding;
use std::str::FromStr;

// Assumes
//
// * The payer with the client is already funded.
// * The program with the client is already deployed. x
//
#[test]
fn initialized_already() {
    let client = client();
    let admin_account_owner = Keypair::generate(&mut OsRng);

    // Create the safe account and initialize it.
    let rent_account = AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false);
    let accounts = vec![rent_account];
    let (signature, safe_account) = client
        .create_account_and_initialize(&accounts, admin_account_owner.pubkey())
        .unwrap();

    let status2 = client
        .rpc()
        .get_confirmed_transaction(&signature, UiTransactionEncoding::Json);

    println!("STATUS 2 {:?}", status2);

    /*

    // Try to initialize it a second time.
    let accounts_2 = vec![AccountMeta::new(safe_account.pubkey(), false), rent_account];
    let signature = client.initialize(&accounts_2, admin_account_owner.pubkey())?;

    println!("signature {:?}", signature);

    let status = client
        .rpc()
        .get_confirmed_transaction(&signature, UiTransactionEncoding::Binary);

    println!("STATUS {:?}", status);
         */
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
