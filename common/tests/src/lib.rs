#![cfg_attr(feature = "strict", deny(warnings))]

use serde::Serialize;
use serum_common::client::Cluster;
use solana_client_gen::prelude::ClientGen;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_client::rpc_config::RpcSendTransactionConfig;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;

// Env variables that must be exported to use this crate.
pub static TEST_PROGRAM_ID: &str = "TEST_PROGRAM_ID";
pub static TEST_PAYER_FILEPATH: &str = "TEST_PAYER_FILEPATH";
pub static TEST_CLUSTER: &str = "TEST_CLUSTER";

// Creates
//
// * Mint authority (shared between SRM and MSRM)
// * SRM mint
// * SRM god account (funded wallet)
// * MSRM mint
// * MSRM god account (funded wallet)
// * RPC client.
//
pub fn genesis<T: ClientGen>() -> (T, Genesis) {
    let client = client::<T>();

    let spl_mint_decimals = 6;

    // Initialize the SPL token representing SRM.
    let mint_authority = Keypair::from_bytes(&Keypair::to_bytes(client.payer().clone())).unwrap();
    let srm_mint = Keypair::generate(&mut OsRng);
    let _ = serum_common::client::rpc::create_and_init_mint(
        client.rpc(),
        client.payer(),
        &srm_mint,
        &mint_authority.pubkey(),
        spl_mint_decimals,
    )
    .unwrap();

    // Initialize the SPL token representing MSRM.
    let mint_authority = Keypair::from_bytes(&Keypair::to_bytes(client.payer().clone())).unwrap();
    let msrm_mint = Keypair::generate(&mut OsRng);
    let _ = serum_common::client::rpc::create_and_init_mint(
        client.rpc(),
        client.payer(),
        &msrm_mint,
        &mint_authority.pubkey(),
        spl_mint_decimals,
    );

    // Create a funded SRM account.
    let god_balance_before = 1_000_000_000_000_000;
    let god = serum_common::client::rpc::mint_to_new_account(
        client.rpc(),
        client.payer(),
        &mint_authority,
        &srm_mint.pubkey(),
        god_balance_before,
    )
    .unwrap();
    // Create a funded MSRM account.
    let god_msrm_balance_before = 1_000_000_000_000_000;
    let god_msrm = serum_common::client::rpc::mint_to_new_account(
        client.rpc(),
        client.payer(),
        &mint_authority,
        &msrm_mint.pubkey(),
        god_balance_before,
    )
    .unwrap();

    let god_owner = client.payer().pubkey();

    let g = Genesis {
        wallet: client.payer().pubkey(),
        mint_authority: mint_authority.pubkey(),
        srm_mint: srm_mint.pubkey(),
        msrm_mint: msrm_mint.pubkey(),
        god: god.pubkey(),
        god_msrm: god_msrm.pubkey(),
        god_balance_before,
        god_msrm_balance_before,
        god_owner,
    };

    (client, g)
}

// Genesis defines the initial state of the world.
#[derive(Serialize)]
pub struct Genesis {
    pub wallet: Pubkey,
    // SRM mint authority.
    pub mint_authority: Pubkey,
    // SRM.
    pub srm_mint: Pubkey,
    // MSRM.
    pub msrm_mint: Pubkey,
    // Account funded with a ton of SRM.
    pub god: Pubkey,
    // Account funded with a ton of MSRM.
    pub god_msrm: Pubkey,
    // Balance of the god account to start.
    pub god_balance_before: u64,
    // Balance of the god_msrm account to start.
    pub god_msrm_balance_before: u64,
    // Owner of both god accounts.
    pub god_owner: Pubkey,
}

pub fn client<T: ClientGen>() -> T {
    let program_id = std::env::var(TEST_PROGRAM_ID).unwrap().parse().unwrap();
    client_at(program_id)
}

pub fn client_at<T: ClientGen>(program_id: Pubkey) -> T {
    let payer_filepath = payer_filepath();
    let cluster = cluster();

    T::from_keypair_file(program_id, &payer_filepath, cluster.url())
        .expect("invalid keypair file")
        .with_options(RequestOptions {
            commitment: CommitmentConfig::single(),
            tx: RpcSendTransactionConfig {
                skip_preflight: true,
                ..RpcSendTransactionConfig::default()
            },
        })
}

pub fn cluster() -> Cluster {
    std::env::var(TEST_CLUSTER).unwrap().parse().unwrap()
}

pub fn payer_filepath() -> String {
    std::env::var(TEST_PAYER_FILEPATH).unwrap().clone()
}
