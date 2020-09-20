use anyhow::Result;
use std::str::FromStr;

pub mod rpc;

#[derive(Debug)]
pub enum Cluster {
    Testnet,
    Mainnet,
    VipMainnet,
    Devnet,
    Localnet,
    Debug,
}

impl FromStr for Cluster {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Cluster> {
        match s.to_lowercase().as_str() {
            "t" | "testnet" => Ok(Cluster::Testnet),
            "m" | "mainnet" => Ok(Cluster::Mainnet),
            "v" | "vipmainnet" => Ok(Cluster::VipMainnet),
            "d" | "devnet" => Ok(Cluster::Devnet),
            "l" | "localnet" => Ok(Cluster::Localnet),
            "g" | "debug" => Ok(Cluster::Debug),
            _ => Err(anyhow::Error::msg(
                "Cluster must be one of [testnet, mainnet, devnet]\n",
            )),
        }
    }
}

impl Cluster {
    pub fn url(&self) -> &'static str {
        match self {
            Cluster::Devnet => "https://devnet.solana.com",
            Cluster::Testnet => "https://testnet.solana.com",
            Cluster::Mainnet => "https://api.mainnet-beta.solana.com",
            Cluster::VipMainnet => "https://vip-api.mainnet-beta.solana.com",
            Cluster::Localnet => "http://127.0.0.1:8899",
            Cluster::Debug => "http://34.90.18.145:8899",
        }
    }
}
