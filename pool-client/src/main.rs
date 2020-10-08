use anyhow::{anyhow, Context};
use solana_sdk::{
    pubkey::Pubkey,
    commitment_config::CommitmentConfig,
    signature::{Keypair, Signer, read_keypair_file},
    instruction::{
        AccountMeta,
        Instruction,
    },
    transaction::Transaction,
};
use solana_client::rpc_client::RpcClient;
use std::{
    str::FromStr,
    slice,
};

use capnp::{
    message,
    serialize::write_message,
    traits::HasTypeId,
};

use arrayref::array_refs;

mod schema;
use schema::{
    cpi_capnp::{self, cpi_instr},
    pool_capnp::proxy_request
};

fn write_address(mut builder: cpi_capnp::address::Builder, address: Pubkey) {
    let bytes = address.to_bytes();
    let words = array_refs![&bytes, 8, 8, 8, 8];
    builder.set_word0(u64::from_le_bytes(*words.0));
    builder.set_word1(u64::from_le_bytes(*words.1));
    builder.set_word2(u64::from_le_bytes(*words.2));
    builder.set_word3(u64::from_le_bytes(*words.3));
}

fn write_account_info(builder: cpi_capnp::account_info::Builder, address: Pubkey) {
    write_address(builder.init_address(), address)
}

fn main() -> anyhow::Result<()> {
    let client = RpcClient::new("http://127.0.0.1:8899".into());
    let pool_program_id = Pubkey::from_str("H5VPa3FzJS32rbWcb7bPq6LbSsneEc8qoMnEVuBLYLxP")?;

    let gas_payer = read_keypair_file("/home/sebastian/.config/solana/id.json")
        .map_err(|e| anyhow!("failed to read keypair: {}", e))?;

    let pool_key = Keypair::new();

    let (hash, _) = client.get_recent_blockhash()
        .context("failed to get recent blockhash")?;

    // create the pool account
    {
        const POOL_LEN: u64 = 256;
        let lamports = client.get_minimum_balance_for_rent_exemption(POOL_LEN as usize)
            .context("failed to get rent amount")?;
        // create pool account
        let txn = solana_sdk::system_transaction::create_account(
            &gas_payer,
            &pool_key,
            hash,
            lamports,
            POOL_LEN,
            &pool_program_id,
        );
        client.send_and_confirm_transaction_with_spinner_and_commitment(
            &txn,
            CommitmentConfig::max(),
        ).context("failed to create pool account")?;

    }

    // initialize the pool
    {
        let pool_instruction = {
            let mut msg = message::Builder::new_default();
            {
                let mut cpi_instr: cpi_instr::Builder<proxy_request::Owned> = msg.init_root();
                cpi_instr.set_type_id(proxy_request::Reader::type_id());
                let mut request: proxy_request::Builder = cpi_instr.init_inner_instruction();
                write_account_info(request.reborrow().init_state_root(), pool_key.pubkey());
                write_address(request.init_instruction().init_init_proxy().init_admin_key(), gas_payer.pubkey());
            }
            let mut data = Vec::new();
            write_message(&mut data, &msg)?;
            Instruction {
                program_id: pool_program_id,
                accounts: vec![
                    AccountMeta::new(pool_key.pubkey(), false)
                ],
                data,
            }
        };

        let (hash, _) = client.get_recent_blockhash()
            .context("failed to get recent blockhash")?;

        let pool_txn = Transaction::new_signed_with_payer(
            slice::from_ref(&pool_instruction),
            Some(&gas_payer.pubkey()),
            &[&gas_payer],
            hash
        );

        client.send_and_confirm_transaction_with_spinner_and_commitment(
            &pool_txn,
            CommitmentConfig::single(),
        ).context("failed to initialize pool")?;
    }

    println!("pool: {}", pool_key.pubkey());

    Ok(())
}

