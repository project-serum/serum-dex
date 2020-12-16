use anyhow::Result;
use std::str::FromStr;

pub mod rpc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Cluster {
    Testnet,
    Mainnet,
    VipMainnet,
    Devnet,
    Localnet,
    Debug,
    Custom(String),
}

impl Default for Cluster {
    fn default() -> Self {
        Cluster::Localnet
    }
}

impl FromStr for Cluster {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Cluster> {
        match s.to_lowercase().as_str() {
            url if url.contains("http") => Ok(Cluster::Custom(s.to_owned())),
            "t" | "testnet" => Ok(Cluster::Testnet),
            "m" | "mainnet" => Ok(Cluster::Mainnet),
            "v" | "vipmainnet" => Ok(Cluster::VipMainnet),
            "d" | "devnet" => Ok(Cluster::Devnet),
            "l" | "localnet" => Ok(Cluster::Localnet),
            "g" | "debug" => Ok(Cluster::Debug),
            _ => Err(anyhow::Error::msg(
                "Cluster must be one of [localnet, testnet, mainnet, devnet] or be an http or https url\n",
            )),
        }
    }
}

impl std::fmt::Display for Cluster {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let clust_str = match self {
            Cluster::Testnet => "testnet",
            Cluster::Mainnet => "mainnet",
            Cluster::VipMainnet => "vipmainnet",
            Cluster::Devnet => "devnet",
            Cluster::Localnet => "localnet",
            Cluster::Debug => "debug",
            Cluster::Custom(url) => url,
        };
        write!(f, "{}", clust_str)
    }
}

impl Cluster {
    pub fn url(&self) -> &str {
        match self {
            Cluster::Devnet => "https://devnet.solana.com",
            Cluster::Testnet => "https://testnet.solana.com",
            Cluster::Mainnet => "https://api.mainnet-beta.solana.com",
            Cluster::VipMainnet => "https://vip-api.mainnet-beta.solana.com",
            Cluster::Localnet => "http://127.0.0.1:8899",
            Cluster::Debug => "http://34.90.18.145:8899",
            Cluster::Custom(url) => url,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cluster(name: &str, cluster: Cluster) {
        assert_eq!(Cluster::from_str(name).unwrap(), cluster);
    }

    #[test]
    fn test_cluster_parse() {
        test_cluster("testnet", Cluster::Testnet);
        test_cluster("mainnet", Cluster::Mainnet);
        test_cluster("vipmainnet", Cluster::VipMainnet);
        test_cluster("devnet", Cluster::Devnet);
        test_cluster("localnet", Cluster::Localnet);
        test_cluster("debug", Cluster::Debug);

        let custom_http = "http://my_custom_url.test.net";
        test_cluster(custom_http, Cluster::Custom(custom_http.into()));
        let custom_https = "https://my_custom_url.test.net";
        test_cluster(custom_https, Cluster::Custom(custom_https.into()));
    }

    #[test]
    #[should_panic]
    fn test_cluster_bad_parse() {
        let bad_url = "httq://my_custom_url.test.net";
        Cluster::from_str(bad_url).unwrap();
    }
}
