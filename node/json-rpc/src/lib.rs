use clap::Clap;
use jsonrpc_http_server::{Server, ServerBuilder};
use serum_node_logging::info;

mod handlers;

pub struct StartRequest {
    pub cfg: Config,
    pub crank: serum_node_crank::Sender,
}

#[derive(Debug, Clap)]
pub struct Config {
    /// HTTP port for the JSON RPC server.
    #[clap(long = "json.http.port", default_value = "8080")]
    pub http_port: u16,
    /// Number of threads for the JSON RPC server.
    #[clap(long = "json.http.threads", default_value = "3")]
    pub http_threads: u16,
}

pub fn start(req: StartRequest) -> JsonRpc {
    let url = format!("127.0.0.1:{}", req.cfg.http_port);

    let logger = serum_node_logging::get_logger("json-rpc");
    info!(logger, "Starting JSON-RPC server at {}", url);

    let handlers = handlers::build(logger, req.crank);

    let _server = ServerBuilder::new(handlers)
        .threads(req.cfg.http_threads as usize)
        .start_http(&url.parse().unwrap())
        .unwrap();

    JsonRpc { _server }
}

pub struct JsonRpc {
    _server: Server,
}
