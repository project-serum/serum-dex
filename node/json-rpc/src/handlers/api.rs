use crate::handlers::api_trait;
use crate::handlers::FutureResult;
use futures::channel::oneshot;
use futures::future::TryFutureExt;
use jsonrpc_core::Error as RpcError;
use serum_node_crank::{
    HealthResponse as CrankHealthResponse, Request as CrankRequest, Response as CrankResponse,
};
use serum_node_logging::{trace, Logger};
use serum_node_registry::{
    HealthResponse as RegistryHealthResponse, Request as RegistryRequest,
    Response as RegistryResponse,
};
use std::convert::Into;

pub struct Api {
    logger: Logger,
    crank: serum_node_crank::Sender,
    registry: serum_node_registry::Sender,
}

impl Api {
    pub fn new(
        logger: Logger,
        crank: serum_node_crank::Sender,
        registry: serum_node_registry::Sender,
    ) -> Self {
        Self {
            logger,
            crank,
            registry,
        }
    }
}

impl api_trait::Api for Api {
    fn crank_health(&self) -> FutureResult<CrankHealthResponse> {
        trace!(self.logger, "serum_startCrank");

        // Send request to the crank.
        let fut = {
            let mut crank = self.crank.clone();
            async move {
                let (tx, rx) = oneshot::channel();
                crank
                    .try_send((CrankRequest::Health, tx))
                    .map_err(Into::into)
                    .map_err(jsonrpc_error)?;

                let resp = rx
                    .await
                    .map_err(Into::into)
                    .map_err(jsonrpc_error)?
                    .map_err(jsonrpc_error)?;

                match resp {
                    CrankResponse::Health(r) => Ok(r),
                }
            }
        };

        // Convert to pre-async/await future.
        let rpc_fut = Box::pin(fut).compat();

        // Response.
        Box::new(rpc_fut)
    }

    fn registry_health(&self) -> FutureResult<RegistryHealthResponse> {
        trace!(self.logger, "serum_createEntity");

        // Send request to the registry.
        let fut = {
            let mut registry = self.registry.clone();
            async move {
                let (tx, rx) = oneshot::channel();
                registry
                    .try_send((RegistryRequest::Health, tx))
                    .map_err(Into::into)
                    .map_err(jsonrpc_error)?;

                let resp = rx
                    .await
                    .map_err(Into::into)
                    .map_err(jsonrpc_error)?
                    .map_err(jsonrpc_error)?;

                match resp {
                    RegistryResponse::Health(r) => Ok(r),
                }
            }
        };

        // Convert to pre-async/await future.
        let rpc_fut = Box::pin(fut).compat();

        // Response.
        Box::new(rpc_fut)
    }
}

/// Constructs a JSON-RPC error from a string message, with error code -32603.
pub fn jsonrpc_error(err: anyhow::Error) -> RpcError {
    RpcError {
        code: jsonrpc_core::ErrorCode::InternalError,
        message: format!("{}", err),
        data: None,
    }
}
