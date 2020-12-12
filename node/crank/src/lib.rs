extern crate crank as serum_crank;

use anyhow::Result;
use crossbeam::sync::WaitGroup;
use serum_context::Context;
use tokio::runtime::{Builder, Runtime};

mod api;
mod dispatch;

// Re-export.
pub use api::HealthResponse;
pub use dispatch::*;
pub use serum_crank::Command;

pub struct StartRequest {
    pub rpc: Receiver,
    pub start_wg: WaitGroup,
}

pub fn start(req: StartRequest) -> Runtime {
    let runtime = Builder::new()
        .thread_name("crank")
        .threaded_scheduler()
        .enable_all()
        .build()
        .expect("Failed to start crank runtime.");
    runtime.handle().spawn(dispatch(req.rpc, req.start_wg));
    runtime
}

pub fn run_cmd(ctx: &Context, cmd: Command) -> Result<()> {
    serum_crank::start(serum_crank::Opts {
        cluster: ctx.cluster.clone(),
        command: cmd,
    })
}
