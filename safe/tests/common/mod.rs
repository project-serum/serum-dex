use serum_common::Cluster;
use serum_safe::client::{Client, ClientError, RequestOptions};
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use std::str::FromStr;

pub mod lifecycle;

// The test client assumes:
//
// * The payer with the client is already funded.
// * The program with the client is already deployed. x
//
pub fn client() -> Client {
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
