//! Misc dev utilities.

extern crate crank as serum_crank;

use anyhow::Result;
use clap::Clap;
use serum_common::client::rpc;
use serum_context::Context;
use serum_dex::instruction::NewOrderInstructionV1;
use serum_dex::matching::{OrderType, Side};
use serum_registry::client::Client;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use std::num::NonZeroU64;

mod faucet;

#[derive(Debug, Clap)]
pub enum Command {
    /// Creates 1) SRM mint, 2) MSRM mint 3) SRM funded token account, and
    /// 4) MSRM funded token account, all owned by the configured wallet.
    InitMint {
        #[clap(short, long)]
        faucet: bool,
    },
    AllocateAccount {
        #[clap(short, long)]
        program_id: Pubkey,
        #[clap(short, long)]
        size: usize,
    },
    GenerateOrders {
        #[clap(short, long)]
        coin_wallet: Pubkey,
        #[clap(short, long)]
        pc_wallet: Pubkey,
    },
}

pub fn run(ctx: Context, cmd: Command) -> Result<()> {
    match cmd {
        Command::InitMint { faucet } => init_mint(&ctx, faucet),
        Command::AllocateAccount { program_id, size } => allocate_account(&ctx, program_id, size),
        Command::GenerateOrders {
            coin_wallet,
            pc_wallet,
        } => generate_orders(&ctx, coin_wallet, pc_wallet),
    }
}

fn init_mint(ctx: &Context, faucet: bool) -> Result<()> {
    // Doesn't matter.
    let program_id = Pubkey::new_from_array([0; 32]).to_string();
    let payer_filepath = &ctx.wallet_path.to_string();
    let cluster = ctx.cluster.to_string();
    let (srm_faucet, msrm_faucet) = match faucet {
        false => (None, None),
        true => {
            let srm_faucet =
                faucet::create(ctx, g.srm_mint, 1_000_000_000_000, ctx.wallet().pubkey())?;
            let msrm_faucet =
                faucet::create(ctx, g.msrm_mint, 1_000_000_000_000, ctx.wallet().pubkey())?;
            (Some(srm_faucet), Some(msrm_faucet))
        }
    };
    println!(
        "{}",
        serde_json::json!({
            "wallet": g.wallet.to_string(),
            "srmMint": g.srm_mint.to_string(),
            "msrmMint": g.msrm_mint.to_string(),
            "god": g.god.to_string(),
            "godMsrm": g.god_msrm.to_string(),
            "godBalanceBefore": g.god_balance_before,
            "godMsrmBalanceBefore": g.god_msrm_balance_before,
            "godOwner": g.god_owner.to_string(),
            "srmFaucet": match srm_faucet {
                None => "null".to_string(),
                Some(f) => f.to_string(),
            },
            "msrmFaucet": match msrm_faucet {
                None => "null".to_string(),
                Some(f) => f.to_string(),
            }
        })
    );

    Ok(())
}

fn allocate_account(ctx: &Context, program_id: Pubkey, size: usize) -> Result<()> {
    let rpc_client = ctx.rpc_client();
    let wallet = ctx.wallet().unwrap();
    let pk = rpc::create_account_rent_exempt(&rpc_client, &wallet, size, &program_id)?.pubkey();
    println!("{}", serde_json::json!({"account": pk.to_string()}));
    Ok(())
}

fn generate_orders(ctx: &Context, coin_wallet: Pubkey, pc_wallet: Pubkey) -> Result<()> {
    let client = ctx.rpc_client();

    let market_keys = serum_crank::list_market(
        &client,
        &ctx.dex_pid,
        &ctx.wallet()?,
        &ctx.srm_mint,
        &ctx.msrm_mint,
        1_000_000,
        10_000,
    )?;

    loop {
        // Place bid.
        let mut orders = None;
        serum_crank::place_order(
            &client,
            &ctx.dex_pid,
            &ctx.wallet()?,
            &pc_wallet,
            &market_keys,
            &mut orders,
            NewOrderInstructionV1 {
                side: Side::Bid,
                limit_price: NonZeroU64::new(500).unwrap(),
                max_qty: NonZeroU64::new(1_000).unwrap(),
                order_type: OrderType::Limit,
                client_id: 019269,
            },
        )?;

        // Place offer.
        let mut orders = None;
        serum_crank::place_order(
            &client,
            &ctx.dex_pid,
            &ctx.wallet()?,
            &coin_wallet,
            &market_keys,
            &mut orders,
            NewOrderInstructionV1 {
                side: Side::Ask,
                limit_price: NonZeroU64::new(499).unwrap(),
                max_qty: NonZeroU64::new(1_000).unwrap(),
                order_type: OrderType::Limit,
                client_id: 985982,
            },
        )?;

        // Match orders.
        std::thread::sleep(std::time::Duration::new(15, 0));
        serum_crank::match_orders(
            &client,
            &ctx.dex_pid,
            &ctx.wallet()?,
            &market_keys,
            &coin_wallet,
            &pc_wallet,
        )?;
    }
}
