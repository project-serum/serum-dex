use anyhow::Result;
use clap::Clap;
use serum_node_context::Context;
use serum_registry::client::Client;
use solana_sdk::pubkey::Pubkey;

pub fn run_cmd(ctx: &Context, cmd: Command) -> Result<()> {
    match cmd {
        Command::InitMint => init_mint(ctx)?,
    }
    Ok(())
}

fn init_mint(ctx: &Context) -> Result<()> {
    // Doesn't matter.
    let program_id = Pubkey::new_from_array([0; 32]).to_string();
    let payer_filepath = &ctx.wallet_path.to_string();
    let cluster = ctx.cluster.to_string();
    std::env::set_var(serum_common_tests::TEST_PROGRAM_ID, program_id);
    std::env::set_var(serum_common_tests::TEST_PAYER_FILEPATH, payer_filepath);
    std::env::set_var(serum_common_tests::TEST_CLUSTER, cluster);
    let g = serum_common_tests::genesis::<Client>();

    println!("{:#?}", g);

    Ok(())
}

#[derive(Debug, Clap)]
pub enum Command {
    /// Creates 1) SRM mint, 2) MSRM mint 3) SRM funded token account, and
    /// 4) MSRM funded token account, all owned by the configured wallet.
    InitMint,
}
