use std::str::FromStr;

use anyhow::{anyhow, Context};
use borsh::ser::BorshSerialize;
use serum_pool_schema::PoolRequestInner::InitPool;
use serum_pool_schema::{PoolBasket, PoolRequest, PoolState, PoolTokenInfo, Retbuf};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    transaction::Transaction,
};

fn main() -> anyhow::Result<()> {
    let config = solana_cli_config::Config::load(solana_cli_config::CONFIG_FILE.as_ref().unwrap())?;
    let client = RpcClient::new("http://127.0.0.1:8899".into());
    let pool_program_id = Pubkey::from_str("99Hkf66Cq9jiDZBRbMn4CYyo5BBctg2LssWyHPvXf1tw")?;

    let gas_payer = read_keypair_file(&config.keypair_path)
        .map_err(|e| anyhow!("failed to read keypair: {}", e))?;

    let pool_key = Keypair::new();

    const POOL_LEN: u64 = 256;
    let lamports = client
        .get_minimum_balance_for_rent_exemption(POOL_LEN as usize)
        .context("failed to get rent amount")?;
    let create_account = solana_sdk::system_instruction::create_account(
        &gas_payer.pubkey(),
        &pool_key.pubkey(),
        lamports,
        POOL_LEN,
        &pool_program_id,
    );

    let pool_instruction = {
        let request = PoolRequest {
            state: pool_key.pubkey().into(),
            retbuf: Retbuf {
                retbuf_account: pool_key.pubkey().into(),    // TODO
                retbuf_program_id: pool_key.pubkey().into(), // TODO
            },
            inner: InitPool(PoolState {
                basket: PoolBasket::Simple(serum_pool_schema::SimpleBasket { components: vec![] }),
                admin_key: None,
                pool_token: PoolTokenInfo {
                    mint_address: pool_key.pubkey().into(),  // TODO,
                    vault_address: pool_key.pubkey().into(), // TODO
                    vault_signer_nonce: 0,                   // TODO
                },
            }),
        };
        let data = request.try_to_vec()?;
        Instruction {
            program_id: pool_program_id,
            accounts: vec![AccountMeta::new(pool_key.pubkey(), false)],
            data,
        }
    };

    let (hash, _) = client
        .get_recent_blockhash()
        .context("failed to get recent blockhash")?;

    let pool_txn = Transaction::new_signed_with_payer(
        &[create_account, pool_instruction],
        Some(&gas_payer.pubkey()),
        &[&gas_payer, &pool_key],
        hash,
    );

    client
        .send_and_confirm_transaction_with_spinner_and_config(
            &pool_txn,
            CommitmentConfig::single(),
            RpcSendTransactionConfig {
                preflight_commitment: Some(CommitmentConfig::single().commitment),
                ..RpcSendTransactionConfig::default()
            },
        )
        .context("failed to initialize pool")?;

    println!("pool: {}", pool_key.pubkey());

    Ok(())
}
