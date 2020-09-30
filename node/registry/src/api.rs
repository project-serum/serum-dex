use anyhow::Result;
use serde::{Deserialize, Serialize};

pub fn health() -> Result<HealthResponse> {
    Ok(HealthResponse { ok: true })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub ok: bool,
}
