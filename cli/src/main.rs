use anyhow::Result;
use clap::Clap;
use crank as serum_crank;
use serum_context::{ConfigPath, Context};

#[cfg(feature = "dev")]
mod dev;

#[derive(Debug, Clap)]
#[clap(name = "Serum CLI")]
pub struct Opts {
    #[clap(short, long, default_value)]
    pub config: ConfigPath,
    #[clap(subcommand)]
    pub cmd: Command,
}

#[derive(Debug, Clap)]
pub enum Command {
    /// Crank client.
    Crank(serum_crank::Command),
    /// Development utilities.
    #[cfg(feature = "dev")]
    Dev(dev::Command),
}

fn main() {
    let opts = Opts::parse();
    let ctx = Context::from_config(opts.config).unwrap_or_else(|e| {
        println!("{}", e.to_string());
        std::process::exit(1);
    });
    run(ctx, opts.cmd).unwrap_or_else(|e| {
        println!("{}", e.to_string());
        std::process::exit(1);
    });
}

pub fn run(ctx: Context, cmd: Command) -> Result<()> {
    match cmd {
        Command::Crank(cmd) => serum_crank::start(
            Some(ctx.clone()),
            serum_crank::Opts {
                cluster: ctx.cluster,
                command: cmd,
            },
        ),
        #[cfg(feature = "dev")]
        Command::Dev(dev_cmd) => dev::run(ctx, dev_cmd),
    }
}
