use crate::api::{self, HealthResponse};
use anyhow::Result;
use crossbeam::sync::WaitGroup;
use futures::channel::{mpsc, oneshot};
use futures::future;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serum_node_logging::{error, info};

pub async fn dispatch(rpc_recv: Receiver, start_wg: WaitGroup) {
    let logger = serum_node_logging::get_logger("crank");
    info!(logger, "Starting crank api dispatch");

    drop(start_wg);

    rpc_recv
        .for_each(
            move |(req, resp_ch): (Request, oneshot::Sender<Result<Response>>)| {
                info!(logger, "Dispatch request {:?}", req);

                let resp = {
                    match req {
                        Request::Health => api::health().map(|r| Response::Health(r)),
                    }
                };

                if let Err(e) = resp_ch.send(resp) {
                    error!(logger, "Unable to send api response: {:?}", e);
                }

                future::ready(())
            },
        )
        .await;
}

pub type Sender = mpsc::Sender<(Request, oneshot::Sender<Result<Response>>)>;
pub type Receiver = mpsc::Receiver<(Request, oneshot::Sender<Result<Response>>)>;

#[derive(Debug)]
pub enum Request {
    Health,
}
#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Health(HealthResponse),
}
