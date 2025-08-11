use envconfig::Envconfig;
use std::str::FromStr;
use url::Url;

#[derive(Debug, Clone, Envconfig)]
pub struct EthConfig {
    #[envconfig(from = "ETH_RPC_URLS")]
    pub rpc_urls: RpcUrls,
    #[envconfig(from = "ETH_RPC_RETRY_MAX", default = "10")]
    pub rpc_retry_max: u32,
    #[envconfig(from = "ETH_RPC_RETRY_INIT_BACKOFF_MS", default = "1000")]
    pub rpc_retry_init_backoff_ms: u64,
    #[envconfig(from = "ETH_RPC_RETRY_CPUS", default = "100")]
    pub rpc_retry_cpus: u64,
}

#[derive(Debug, Clone)]
pub struct RpcUrls(pub Vec<Url>);

impl FromStr for RpcUrls {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.split(',')
            .map(|url| url.trim().parse::<Url>())
            .collect::<Result<Vec<_>, _>>()
            .map(RpcUrls)
            .map_err(|e| format!("Invalid url: {}", e))
    }
}
