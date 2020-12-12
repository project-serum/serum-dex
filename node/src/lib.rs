#![cfg_attr(feature = "strict", deny(warnings))]

use anyhow::Result;
use clap::Clap;
use crossbeam::sync::WaitGroup;
use futures::channel::mpsc;
use serum_node_logging::info;
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

            // Start JSON-RPC server.
            let _json_rpc = serum_node_json_rpc::start(serum_node_json_rpc::StartRequest {
                cfg: cfg.json_rpc,
                crank: crank_sender,
            });

            info!(logger, "Starting internal api services");

            // WaitGroup to block thread until all async services are ready.
            let start_wg = WaitGroup::new();

            // Start crank service.
            let _crank = serum_node_crank::start(serum_node_crank::StartRequest {
                rpc: crank_receiver,
                start_wg: start_wg.clone(),
            });

            start_wg.wait();

            info!(logger, "Service setup complete");

            Some(Services { _json_rpc, _crank })
        }
    };

    Ok(Handle { _services })
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
}

pub struct Handle {
    _services: Option<Services>,
}

struct Services {
    _json_rpc: serum_node_json_rpc::JsonRpc,
    _crank: Runtime,
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
