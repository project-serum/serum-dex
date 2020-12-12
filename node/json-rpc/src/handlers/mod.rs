use api::Api;
use api_trait::Api as ApiTrait;
use serum_node_logging::Logger;

mod api;
pub(crate) mod api_trait;

pub type FutureResult<T> =
    Box<dyn jsonrpc_core::futures::Future<Item = T, Error = jsonrpc_core::Error> + Send>;

pub fn build(logger: Logger, crank: serum_node_crank::Sender) -> jsonrpc_core::IoHandler {
    let mut io = jsonrpc_core::IoHandler::new();
    io.extend_with(Api::new(logger, crank).to_delegate());
    io
}
