//! serum-node-registry defines the internal registry node service.

use anyhow::Result;
use clap::Clap;
use crossbeam::sync::WaitGroup;
use serum_node_context::Context;
use solana_sdk::pubkey::Pubkey;
use tokio::runtime::{Builder, Runtime};

mod api;
mod dispatch;

pub use api::HealthResponse;
pub use dispatch::*;

#[derive(Debug, Clap)]
pub struct Command {
    /// Program id of the deployed on-chain registry
    #[clap(long = "program-id")]
    pub program_id: Option<Pubkey>,
}

pub struct StartRequest {
    pub rpc: Receiver,
    pub start_wg: WaitGroup,
}

pub fn start(req: StartRequest) -> Runtime {
    let runtime = Builder::new()
        .thread_name("registry")
        .threaded_scheduler()
        .enable_all()
        .build()
        .expect("Failed to start registry runtime.");
    runtime.handle().spawn(dispatch(req.rpc, req.start_wg));
    runtime
}

pub fn run_cmd(_ctx: &Context, _cmd: Command) -> Result<()> {
    // todo
    Ok(())
}
