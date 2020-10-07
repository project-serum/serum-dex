//! serum-node-registry defines the internal registry node service.

use anyhow::Result;
use serum_node_context::Context;

pub use serum_safe_cli::Command;

pub fn run_cmd(ctx: &Context, cmd: Command) -> Result<()> {
    serum_safe_cli::run(serum_safe_cli::Opts {
        ctx: ctx.clone(),
        cmd,
    })
}
