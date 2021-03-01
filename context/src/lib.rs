//! serum-context defines the global state used by Serum crates, read from
//! a configuration file.

use anyhow::Result;
use anyhow::{anyhow, format_err};
use serde::{Deserialize, Serialize};
use serum_common::client::Cluster;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::fs;
use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct Context {
    pub cluster: Cluster,
    pub wallet_path: WalletPath,
    pub srm_mint: Pubkey,
    pub msrm_mint: Pubkey,
    pub dex_pid: Pubkey,
    pub faucet_pid: Option<Pubkey>,
}

impl Context {
    pub fn from_config(path: ConfigPath) -> Result<Self> {
        Config::from(&path.to_string())?.try_into()
    }

    pub fn rpc_client(&self) -> RpcClient {
        RpcClient::new(self.cluster.url().to_string())
    }

    pub fn wallet(&self) -> Result<Keypair> {
        solana_sdk::signature::read_keypair_file(&self.wallet_path.to_string())
            .map_err(|_| anyhow!("Unable to read provided wallet file"))
    }
}

// Config represents the data read from a config file.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Config {
    pub wallet_path: Option<String>,
    pub network: Network,
    pub mints: Mints,
    pub programs: Programs,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Network {
    pub cluster: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Mints {
    pub srm: String,
    pub msrm: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Programs {
    pub dex_pid: String,
    pub faucet_pid: Option<String>,
}

impl Config {
    fn from(path: &str) -> Result<Self> {
        let rdr = fs::File::open(path)?;
        serde_yaml::from_reader(&rdr)
            .map_err(|e| format_err!("Unable to read yaml config: {}", e.to_string()))
    }
}

impl TryFrom<Config> for Context {
    type Error = anyhow::Error;
    fn try_from(cfg: Config) -> std::result::Result<Self, anyhow::Error> {
        let cluster = cfg
            .network
            .cluster
            .map_or(Ok(Default::default()), |c| c.parse())?;
        let faucet_pid = cfg
            .programs
            .faucet_pid
            .or_else(|| match &cluster {
                Cluster::Devnet => Some("4bXpkKSV8swHSnwqtzuboGPaPDeEgAn4Vt8GfarV5rZt".to_string()),
                _ => None,
            })
            .map(|f| f.parse().unwrap());
        Ok(Self {
            cluster,
            wallet_path: cfg
                .wallet_path
                .map_or(Default::default(), |p| WalletPath(p)),
            srm_mint: cfg.mints.srm.parse()?,
            msrm_mint: cfg.mints.msrm.parse()?,
            dex_pid: cfg.programs.dex_pid.parse()?,
            faucet_pid,
        })
    }
}

// Declare our default file paths, relative to the home directory.
serum_common::home_path!(ConfigPath, ".config/serum/cli/config.yaml");
serum_common::home_path!(WalletPath, ".config/solana/id.json");
serum_common::home_path!(DataDirPath, ".config/serum/cli/data/");
