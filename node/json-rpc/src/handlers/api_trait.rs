use crate::handlers::FutureResult;
use jsonrpc_derive::rpc;
use serum_node_crank::HealthResponse as CrankHealthResponse;

/// Api defines the JSON-RPC interface. Handlers must implement this trait.
#[rpc]
pub trait Api {
    #[rpc(name = "serum_crankHealth")]
    fn crank_health(&self) -> FutureResult<CrankHealthResponse>;
}
