use anyhow::anyhow;
use anyhow::Result;
use clap::Clap;
use serum_common::client::Cluster;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use std::str::FromStr;

#[derive(Clone, Debug, Clap)]
pub struct Context {
    /// Solana cluster to communicate with.
    #[clap(short, long, default_value = "mainnet")]
    pub cluster: Cluster,

    #[clap(short, long = "wallet", default_value)]
    pub wallet_path: WalletPath,

    #[clap(
        short,
        long,
        default_value = "SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt"
    )]
    pub srm_mint: Pubkey,

    #[clap(
        short,
        long,
        default_value = "MSRMcoVyrFxnSgo5uXwone5SKcGhT1KEJMFEkMEWf9L"
    )]
    pub msrm_mint: Pubkey,
}

impl Context {
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

#[derive(Clone, Debug)]
pub struct WalletPath(String);

impl Default for WalletPath {
    fn default() -> Self {
        match dirs::home_dir() {
            None => {
                println!("$HOME doesn't exist. This probably won't do what you want.");
                WalletPath(".".to_string())
            }
            Some(mut path) => {
                path.push(".config/solana/id.json");
                WalletPath(path.as_path().display().to_string())
            }
        }
    }
}

impl ToString for WalletPath {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl FromStr for WalletPath {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}
