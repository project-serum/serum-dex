//! serum-node-registry defines the internal registry node service.

use anyhow::Result;
use crossbeam::sync::WaitGroup;
use serum_node_context::Context;
use tokio::runtime::{Builder, Runtime};

mod api;
mod dispatch;

pub use api::HealthResponse;
pub use dispatch::*;
pub use serum_registry_cli::Command;

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

pub fn run_cmd(ctx: &Context, cmd: Command) -> Result<()> {
    serum_registry_cli::run(serum_registry_cli::Opts {
        ctx: ctx.clone(),
        cmd,
    })
}
