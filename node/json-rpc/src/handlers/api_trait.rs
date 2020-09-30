use crate::handlers::FutureResult;
use jsonrpc_derive::rpc;
use serum_node_crank::HealthResponse as CrankHealthResponse;
use serum_node_registry::HealthResponse as RegistryHealthResponse;

/// Api defines the JSON-RPC interface. Handlers must implement this trait.
#[rpc]
pub trait Api {
    #[rpc(name = "serum_crankHealth")]
    fn crank_health(&self) -> FutureResult<CrankHealthResponse>;

    #[rpc(name = "serum_registryHealth")]
    fn registry_health(&self) -> FutureResult<RegistryHealthResponse>;
}
