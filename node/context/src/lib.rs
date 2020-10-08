use clap::Clap;
use serum_common::client::Cluster;

#[derive(Clone, Debug, Clap)]
pub struct Context {
    /// Solana cluster to communicate with.
    #[clap(long, default_value = "localnet")]
    pub cluster: Cluster,

    /// Path to the node's wallet [optional].
    #[clap(long)]
    pub wallet: Option<String>,
}
