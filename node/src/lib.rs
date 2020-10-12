#![cfg_attr(feature = "strict", deny(warnings))]

use anyhow::Result;
use clap::Clap;
use crossbeam::sync::WaitGroup;
use futures::channel::mpsc;
use serum_node_context::Context;
use serum_node_logging::{error, info, trace};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::runtime::Runtime;

/// start is the entrypoint to running a node.
pub fn start(cfg: Config) -> Result<Handle> {
    // Start logging.
    serum_node_logging::start(cfg.logging);
    let logger = serum_node_logging::get_logger("node");

    // Start the services, if needed.
    let _services = {
        if !cfg.json {
            None
        } else {
            info!(logger, "Node starting 🚀");

            // Channels to relay requests from the RPC server to internal services.
            let crank_chan_size = 4;
            let (crank_sender, crank_receiver) = mpsc::channel(crank_chan_size);
            let registry_chan_size = 1024;
            let (registry_sender, registry_receiver) = mpsc::channel(registry_chan_size);

            // Start JSON-RPC server.
            let _json_rpc = serum_node_json_rpc::start(serum_node_json_rpc::StartRequest {
                cfg: cfg.json_rpc,
                crank: crank_sender,
                registry: registry_sender,
            });

            info!(logger, "Starting internal api services");

            // WaitGroup to block thread until all async services are ready.
            let start_wg = WaitGroup::new();

            // Start crank service.
            let _crank = serum_node_crank::start(serum_node_crank::StartRequest {
                rpc: crank_receiver,
                start_wg: start_wg.clone(),
            });

            // Start registry service.
            let _registry = serum_node_registry::start(serum_node_registry::StartRequest {
                rpc: registry_receiver,
                start_wg: start_wg.clone(),
            });

            start_wg.wait();

            info!(logger, "Service setup complete");

            Some(Services {
                _json_rpc,
                _crank,
                _registry,
            })
        }
    };

    // Run the command, if given.
    if let Some(cmd) = cfg.cmd {
        trace!(logger, "Executing: {:?}", cmd);
        run_cmd(&cfg.context, cmd).map_err(|err| {
            error!(logger, "{}", err);
            serum_node_logging::stop();
            err
        })?;
    }

    Ok(Handle { _services })
}

fn run_cmd(ctx: &Context, cmd: Command) -> Result<()> {
    match cmd {
        Command::Crank(crank_cmd) => serum_node_crank::run_cmd(ctx, crank_cmd),
        Command::Registry(reg_cmd) => serum_node_registry::run_cmd(ctx, reg_cmd),
        Command::Dev(dev_cmd) => serum_node_dev::run_cmd(ctx, dev_cmd),
        Command::Lockup(l_cmd) => serum_node_lockup::run_cmd(ctx, l_cmd),
        Command::Rewards(cmd) => serum_rewards_cli::run(serum_rewards_cli::Opts {
            ctx: ctx.clone(),
            cmd,
        }),
    }
}

#[derive(Debug, Clap)]
#[clap(name = "Serum CLI")]
pub struct Config {
    #[clap(flatten)]
    pub logging: serum_node_logging::Config,

    #[clap(flatten)]
    pub json_rpc: serum_node_json_rpc::Config,

    /// Enables the JSON RPC server if set. Defaults to off.
    #[clap(long)]
    pub json: bool,

    #[clap(flatten)]
    pub context: Context,

    #[clap(subcommand)]
    pub cmd: Option<Command>,
}

#[derive(Debug, Clap)]
pub enum Command {
    /// Crank client utility.
    Crank(serum_node_crank::Command),
    /// Serum registry program.
    Registry(serum_node_registry::Command),
    /// Development utilities.
    Dev(serum_node_dev::Command),
    /// Serum lockup program.
    Lockup(serum_node_lockup::Command),
    /// Serum rewards program.
    Rewards(serum_rewards_cli::Command),
}

pub struct Handle {
    _services: Option<Services>,
}

struct Services {
    _json_rpc: serum_node_json_rpc::JsonRpc,
    _crank: Runtime,
    _registry: Runtime,
}

impl Handle {
    pub fn park(&self) {
        if self._services.is_some() {
            let term = Arc::new(AtomicBool::new(false));
            while !term.load(Ordering::Acquire) {
                std::thread::park();
            }
        }
    }
}
