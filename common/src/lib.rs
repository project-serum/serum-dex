#![cfg_attr(feature = "strict", deny(warnings))]

#[cfg(feature = "client")]
pub mod client;
mod path;
#[cfg(feature = "program")]
pub mod program;
