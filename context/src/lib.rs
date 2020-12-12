//! serum-context defines the global state used by Serum crates, read from
//! a configuration file.

use anyhow::Result;
use anyhow::{anyhow, format_err};
use serde::{Deserialize, Serialize};
use serum_common::client::Cluster;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_client::rpc_client::RpcClient;
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
    pub data_dir_path: DataDirPath,
    pub srm_mint: Pubkey,
    pub msrm_mint: Pubkey,
    pub rewards_pid: Pubkey,
    pub registry_pid: Pubkey,
    pub meta_entity_pid: Pubkey,
    pub lockup_pid: Pubkey,
    pub dex_pid: Pubkey,
}

impl Context {
    pub fn from_config(path: ConfigPath) -> Result<Self> {
        Config::from(&path.to_string())?.try_into()
    }

    pub fn connect<T: ClientGen>(&self, program_id: Pubkey) -> Result<T> {
        let wallet = &self.wallet_path.to_string();
        let c = T::from_keypair_file(program_id, &wallet, self.cluster.url())?.with_options(
            RequestOptions {
                commitment: CommitmentConfig::single(),
                tx: RpcSendTransactionConfig {
                    skip_preflight: true,
                    ..RpcSendTransactionConfig::default()
                },
            },
        );
        Ok(c)
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
    pub data_dir: Option<String>,
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
    pub rewards_pid: String,
    pub registry_pid: String,
    pub meta_entity_pid: String,
    pub lockup_pid: String,
    pub dex_pid: String,
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
        Ok(Self {
            cluster: cfg
                .network
                .cluster
                .map_or(Ok(Default::default()), |c| c.parse())?,
            wallet_path: cfg
                .wallet_path
                .map_or(Default::default(), |p| WalletPath(p)),
            data_dir_path: cfg.data_dir.map_or(Default::default(), |p| DataDirPath(p)),
            srm_mint: cfg.mints.srm.parse()?,
            msrm_mint: cfg.mints.msrm.parse()?,
            rewards_pid: cfg.programs.rewards_pid.parse()?,
            registry_pid: cfg.programs.registry_pid.parse()?,
            lockup_pid: cfg.programs.lockup_pid.parse()?,
            meta_entity_pid: cfg.programs.meta_entity_pid.parse()?,
            dex_pid: cfg.programs.dex_pid.parse()?,
        })
    }
}

// Declare our default file paths, relative to the home directory.
serum_common::home_path!(ConfigPath, ".config/serum/cli/config.yaml");
serum_common::home_path!(WalletPath, ".config/solana/id.json");
serum_common::home_path!(DataDirPath, ".config/serum/cli/data/");
