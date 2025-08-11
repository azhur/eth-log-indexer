use crate::domain::{
    Address, Block, BlockId, BlockNumber, BlockTimestamp, EthTransfer, LogIndex, TransferValue,
    TxHash,
};
use crate::eth::config::EthConfig;
use alloy::network::Ethereum;
use alloy::primitives::LogData;
use alloy::providers::Provider;
use alloy::providers::ProviderBuilder;
use alloy::rpc::client::RpcClient;
use alloy::rpc::types::{Filter, Log};
use alloy::sol;
use alloy::sol_types::SolEvent;
use alloy::transports::http::Http;
use alloy::transports::layers::{FallbackLayer, RetryBackoffLayer};
use eyre::eyre;
use itertools::Itertools;
use reqwest::Client;
use std::num::NonZeroUsize;
use std::ops::RangeInclusive;
use tower::ServiceBuilder;

sol! {
    event Transfer(address indexed from, address indexed to, uint256 value);
}

/// A trait defining the api for interacting with Ethereum providers.
#[async_trait::async_trait]
pub trait EthProvider {
    /// Fetches transfer logs for a specific contract address within a given block range.
    async fn fetch_transfer_logs(
        &self,
        contract_address: Address,
        block_range: RangeInclusive<BlockNumber>,
    ) -> eyre::Result<Vec<EthTransfer>>;

    async fn get_block_by_id(&self, block_id: BlockId) -> eyre::Result<Option<Block>>;
}

pub struct AlloyEthProvider<P> {
    provider: P,
}

pub struct AlloyEthProviderBuilder;
impl AlloyEthProviderBuilder {
    /// Creates a new AlloyEthProvider using HTTP transport based on the provided EthConfig.
    pub fn new_http(cfg: EthConfig) -> eyre::Result<AlloyEthProvider<impl Provider<Ethereum>>> {
        let transports: Vec<Http<Client>> = cfg.rpc_urls.0.into_iter().map(Http::new).collect_vec();

        let retry_layer = RetryBackoffLayer::new(
            cfg.rpc_retry_max,
            cfg.rpc_retry_init_backoff_ms,
            cfg.rpc_retry_cpus,
        );

        let fallback_layer = FallbackLayer::default().with_active_transport_count(
            NonZeroUsize::new(transports.len()).ok_or_else(|| eyre!("No transports provided"))?,
        );

        let transport = ServiceBuilder::new()
            .layer(retry_layer)
            .layer(fallback_layer)
            .service(transports);

        let client = RpcClient::builder().transport(transport, false);
        let provider = ProviderBuilder::new().connect_client(client);

        Ok(AlloyEthProvider { provider })
    }
}

#[async_trait::async_trait]
impl<P> EthProvider for AlloyEthProvider<P>
where
    P: Provider + Send + Sync,
{
    async fn fetch_transfer_logs(
        &self,
        contract_address: Address,
        block_range: RangeInclusive<BlockNumber>,
    ) -> eyre::Result<Vec<EthTransfer>> {
        let filter = Filter::new()
            .address(contract_address.0)
            .from_block(block_range.start().0)
            .to_block(block_range.end().0)
            .event("Transfer(address,address,uint256)");

        let logs = self.provider.get_logs(&filter).await?;

        Ok(logs
            .into_iter()
            .filter_map(|log| EthTransfer::try_from(log).ok())
            .collect())
    }

    async fn get_block_by_id(&self, block_id: BlockId) -> eyre::Result<Option<Block>> {
        self.provider
            .get_block(block_id.into())
            .await
            .map(|block_opt| block_opt.map(Block::from))
            .map_err(Into::into)
    }
}

impl TryFrom<Log<LogData>> for EthTransfer {
    type Error = eyre::Error;

    fn try_from(log: Log<LogData>) -> Result<Self, Self::Error> {
        let transfer = Transfer::decode_log_validate(&log.inner)?;
        // NOTE: for historical transfers the log optional fields should be present so unwrap_or_default is safe
        Ok(EthTransfer {
            contract_address: Address(log.inner.address),
            block_number: BlockNumber(log.block_number.unwrap_or_default()),
            block_timestamp: BlockTimestamp(log.block_timestamp.unwrap_or_default()),
            log_index: LogIndex(log.log_index.unwrap_or_default()),
            tx_hash: TxHash(log.transaction_hash.unwrap_or_default()),
            from_address: Address(transfer.from),
            to_address: Address(transfer.to),
            value: TransferValue(transfer.value),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::*;
    use crate::eth::config::{EthConfig, RpcUrls};
    use crate::eth::provider::{AlloyEthProviderBuilder, EthProvider};
    use alloy::primitives::{U256, address, b256};
    use std::str::FromStr;
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::anvil::AnvilNode;

    #[tokio::test]
    async fn test_provider() -> eyre::Result<()> {
        let node = AnvilNode::default()
            .with_fork_url("https://reth-ethereum.ithaca.xyz/rpc")
            .with_fork_block_number(21500213)
            .start()
            .await?;

        let port = node.get_host_port_ipv4(8545).await?;

        let config = EthConfig {
            rpc_urls: RpcUrls(vec![format!("http://localhost:{}", port).parse()?]),
            rpc_retry_max: 2,
            rpc_retry_init_backoff_ms: 10,
            rpc_retry_cpus: 100,
        };

        let provider = AlloyEthProviderBuilder::new_http(config)?;

        let earliest_block = provider.get_block_by_id(BlockId::Earliest).await?;
        assert_eq!(earliest_block, Some(expected_earliest_block()));

        let latest_block = provider.get_block_by_id(BlockId::Latest).await?;
        assert_eq!(latest_block, Some(expected_latest_block()));

        let finalized_block = provider.get_block_by_id(BlockId::Finalized).await?;
        assert_eq!(finalized_block, Some(expected_finalized_block()));

        let safe_block = provider.get_block_by_id(BlockId::Safe).await?;
        assert_eq!(safe_block, Some(expected_safe_block()));

        let logs = provider
            .fetch_transfer_logs(
                "0x68614481AeF06e53D23bbe0772343fB555ac40c8".parse()?,
                BlockNumber(21376982)..=BlockNumber(21376984),
            )
            .await?;
        assert_eq!(&logs, &expected_logs());

        Ok(())
    }

    fn expected_latest_block() -> Block {
        Block {
            number: BlockNumber(21500213),
            hash: BlockHash(b256!(
                "09afa661a1c383fe926015a8df4e38d43035e3b33c24167454b9e4ad772312db"
            )),
            parent_hash: BlockHash(b256!(
                "ac5e1f4e9db5a1ab1b8456862d54f9ed74c5fd6a04a5c61b6805af13b322895d"
            )),
        }
    }

    fn expected_earliest_block() -> Block {
        Block {
            number: BlockNumber(0),
            hash: BlockHash(b256!(
                "d4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3"
            )),
            parent_hash: BlockHash(b256!(
                "0000000000000000000000000000000000000000000000000000000000000000"
            )),
        }
    }

    fn expected_finalized_block() -> Block {
        Block {
            number: BlockNumber(21500149),
            hash: BlockHash(b256!(
                "be64d0f06fe7b5e8ca6d1c74b3f8df805e2f3fb166b2c513cd823e0229ace520"
            )),
            parent_hash: BlockHash(b256!(
                "e09e2b083e7bf3029bdccf54fcd12f90d1d2a2415df66a7bd81b858d2aa2d164"
            )),
        }
    }

    fn expected_safe_block() -> Block {
        Block {
            number: BlockNumber(21500181),
            hash: BlockHash(b256!(
                "f718f7c7f960b0cef784982561ccb801c41556761434ff76aacbc10adca802dd"
            )),
            parent_hash: BlockHash(b256!(
                "e8621bf13411c9b5ed3888dd72034666c7c283b8d48489c1e72400662bc620bc"
            )),
        }
    }

    fn expected_logs() -> Vec<EthTransfer> {
        vec![EthTransfer {
            tx_hash: TxHash(b256!(
                "d3e218036b9bd2561a797341806baf6b04f37cec9b6ce16288a24f12742da8e0"
            )),
            log_index: LogIndex(0),
            contract_address: Address(address!("0x68614481aef06e53d23bbe0772343fb555ac40c8")),
            block_number: BlockNumber(21376983),
            block_timestamp: BlockTimestamp(1733892491),
            from_address: Address(address!("0x0000000000000000000000000000000000000000")),
            to_address: Address(address!("0x2b6ec277bec8b7b1b19efca00c1969cac63c9f0f")),
            value: TransferValue(U256::from_str("1000000000000000000000000000").unwrap()),
        }]
    }
}
