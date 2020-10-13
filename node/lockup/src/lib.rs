//! serum-node-registry defines the internal registry node service.

use anyhow::Result;
use serum_node_context::Context;

pub use serum_lockup_cli::Command;

pub fn run_cmd(ctx: &Context, cmd: Command) -> Result<()> {
    serum_lockup_cli::run(serum_lockup_cli::Opts {
        ctx: ctx.clone(),
        cmd,
    })
}
