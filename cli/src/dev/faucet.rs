use anyhow::{anyhow, Result};
use serum_common::client::rpc;
use serum_context::Context;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use solana_sdk::sysvar;
use solana_sdk::transaction::Transaction;

const FAUCET_SIZE: usize = 77;

// `admin` must be the current mint authority.
//
// Faucet program here:
//
// https://github.com/paul-schaaf/spl-token-faucet/blob/main/src/program/src/instruction.rs.
pub fn create(ctx: &Context, mint: Pubkey, amount: u64, admin: Pubkey) -> Result<Pubkey> {
    let faucet_pid = ctx.faucet_pid.ok_or(anyhow!("faucet not provided"))?;
    let faucet = rpc::create_account_rent_exempt(
        &ctx.rpc_client(),
        &ctx.wallet()?,
        FAUCET_SIZE,
        &faucet_pid,
    )?
    .pubkey();

    let ixs = {
        let (faucet_pda, _nonce) =
            Pubkey::find_program_address(&["faucet".to_string().as_bytes()], &faucet_pid);

        let set_auth_ix = spl_token::instruction::set_authority(
            &spl_token::ID,
            &mint,
            Some(&faucet_pda),
            spl_token::instruction::AuthorityType::MintTokens,
            &admin,
            &[],
        )?;

        let create_faucet_ix = {
            let accounts = vec![
                AccountMeta::new_readonly(mint, false),
                AccountMeta::new(faucet, false),
                AccountMeta::new(sysvar::rent::ID, false),
                AccountMeta::new_readonly(admin, false),
            ];

            let mut data = vec![0];
            data.extend_from_slice(&amount.to_le_bytes());

            Instruction {
                program_id: faucet_pid,
                data,
                accounts,
            }
        };

        [set_auth_ix, create_faucet_ix]
    };

    let _tx = {
        let client = ctx.rpc_client();
        let payer = ctx.wallet()?;
        let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
        let tx =
            Transaction::new_signed_with_payer(&ixs, Some(&payer.pubkey()), &[&payer], recent_hash);
        let sig = client.send_and_confirm_transaction_with_spinner_and_config(
            &tx,
            CommitmentConfig::single(),
            RpcSendTransactionConfig {
                skip_preflight: true,
                ..RpcSendTransactionConfig::default()
            },
        )?;
        sig
    };

    Ok(faucet)
}
