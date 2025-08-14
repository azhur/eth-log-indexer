use crate::domain::{Address, Block, BlockId, BlockNumber, EthTransferBatch, TipBlock};
use crate::eth::provider::EthProvider;
use crate::store::Store;
use envconfig::Envconfig;
use itertools::Itertools;
use std::cmp::{max, min};
use std::ops::RangeInclusive;
use std::sync::Arc;

pub struct LogIndexer {
    cfg: IndexerConfig,
    provider: Arc<dyn EthProvider>,
    store: Arc<dyn Store>,
}

impl LogIndexer {
    pub fn new(cfg: IndexerConfig, provider: Arc<dyn EthProvider>, store: Arc<dyn Store>) -> Self {
        Self {
            cfg,
            provider,
            store,
        }
    }

    pub async fn run(&self) -> eyre::Result<()> {
        loop {
            if let Err(e) = self.tick().await {
                tracing::error!("Error running indexer: {:?}", e);
            }

            tracing::info!(
                "Waiting for {} ms before next run",
                self.cfg.run_interval_ms
            );

            tokio::time::sleep(std::time::Duration::from_millis(self.cfg.run_interval_ms)).await;
        }
    }

    pub async fn tick(&self) -> eyre::Result<()> {
        let reconciled_tip = self.reconcile_tip().await?;

        let tip_block = self.get_provider_tip_block().await?;
        let tip_block_number = tip_block.number;
        let start_block_number = self.get_start_block_number(reconciled_tip).await?;
        let finalized_block_number = self
            .provider
            .get_block_by_id(BlockId::Finalized)
            .await?
            .map(|n| n.number);

        tracing::info!(
            "Start block: {:?}, finalized block {:?}, provider tip block {:?}{:?}",
            start_block_number,
            finalized_block_number,
            self.cfg.tip_block,
            tip_block_number,
        );

        // Nothing to do if we’re already past the tip
        if start_block_number > tip_block_number {
            return Ok(());
        }

        let mut batch_start = start_block_number;

        loop {
            let batch_end = min(
                batch_start + self.cfg.block_batch_size - 1,
                tip_block_number,
            );

            // todo: optimize the historical indexing by using parallel batch transfer fetchers
            // sqlite doesn't support concurrent writes, we can use concurrent batch fetchers
            // that sink into a single store writer.
            self.index_batch(batch_start..=batch_end, finalized_block_number)
                .await?;

            // advance; break when we’ve just processed the tip
            if batch_end == tip_block_number {
                break;
            }

            batch_start = batch_end + 1;
        }

        if let Some(final_block_number) = finalized_block_number {
            // cleanup the finalized blocks, we are interested only in non-finalized blocks to handle reorgs
            self.store
                .drop_blocks_before(final_block_number + 1)
                .await?;
        }

        tracing::info!("Reached tip block {:?}", tip_block_number);

        Ok(())
    }

    /// Handles reorgs by reconciling local tip hash vs corresponding provider block hash and rewinding back until hashes for local block and provider block match.
    /// We store locally only non-finalized blocks so rewinding back is possible until all non-finalized blocks are exhausted.
    /// Ideally reorgs are not expected often and not expected to be very deep so this approach should be fine in most cases.
    /// returns reorg-free local tip or None
    async fn reconcile_tip(&self) -> eyre::Result<Option<BlockNumber>> {
        // finalized blocks are reorg-free so no need to check the local tip
        if self.cfg.tip_block == TipBlock::Finalized {
            return Ok(None);
        }

        loop {
            let Some(local_tip) = self.store.get_tip_block().await? else {
                break;
            };

            let Some(remote_tip) = self
                .provider
                .get_block_by_id(BlockId::Number(local_tip.number))
                .await?
            else {
                tracing::warn!(
                    "Remote block not found at {:?}, rewinding back by 1",
                    local_tip.number
                );
                self.store.rewind_back(local_tip.number).await?;
                continue;
            };

            if local_tip != remote_tip {
                tracing::info!(
                    "Reorg detected at {:?}, rewinding back by 1",
                    local_tip.number
                );
                self.store.rewind_back(local_tip.number).await?;
            } else {
                return Ok(Some(local_tip.number));
            }
        }
        Ok(None)
    }

    async fn index_batch(
        &self,
        range: RangeInclusive<BlockNumber>,
        finalized_block: Option<BlockNumber>,
    ) -> eyre::Result<()> {
        let last_block = *range.end();

        // todo handle provider 'max block range' and 'max return results per range' limits
        // We are indexing a specific contract address so it's unlikely to hit the max returned results limits but anyway we should handle those possible errors.
        // A naive approach would be to set ETH_BLOCK_BATCH_SIZE env var to a value that is way less than the provider limits
        // but this will not be optimal for indexing the historical logs on the very first tick.
        // A more optimal approach would be to use a dynamic batch size that is adjusted depending of the context (ie keep large until limit error, then reduce the size on error).
        //
        // sample error for 'max block range': 'code: -32602 message: query exceeds max block range 100000'
        // sample error for 'max return results per range': 'code: -32602 message: query exceeds max results 20000, retry with the range 23004221-23004288'
        let transfers = self
            .provider
            .fetch_transfer_logs(self.cfg.contract_address, range.clone())
            .await?;

        // we are interested in non-finalized blocks to handle reorgs
        let non_finalized_blocks = finalized_block
            .filter(|&finalized| finalized < last_block)
            .map(|finalized| {
                (max(finalized.0 + 1, range.start().0)..=last_block.0)
                    .map(BlockNumber)
                    .collect_vec()
            })
            .unwrap_or_default();

        let non_final_blocks = self.get_provider_blocks(&non_finalized_blocks).await?;

        let batch = EthTransferBatch {
            transfers,
            last_block,
            non_final_blocks,
        };

        self.store.save_transfer_batch(batch).await
    }

    async fn get_provider_blocks(&self, numbers: &[BlockNumber]) -> eyre::Result<Vec<Block>> {
        let mut blocks = vec![];
        // todo parallelize
        for num in numbers {
            let block = self.provider.get_block_by_id(BlockId::Number(*num)).await?;
            if let Some(block) = block {
                blocks.push(block);
            }
        }
        Ok(blocks)
    }

    async fn get_provider_tip_block(&self) -> eyre::Result<Block> {
        let block = self
            .provider
            .get_block_by_id(self.cfg.tip_block.into())
            .await?
            .ok_or_else(|| eyre::eyre!("Tip block not found"))?;
        Ok(block)
    }

    async fn get_start_block_number(
        &self,
        reconciled_tip: Option<BlockNumber>,
    ) -> eyre::Result<BlockNumber> {
        if let Some(reconciled_tip) = reconciled_tip {
            return Ok(reconciled_tip + 1);
        }

        if let Some(last_block) = self.store.get_last_indexed_block().await? {
            Ok(last_block + 1)
        } else {
            tracing::info!("No indexed blocks found, starting from the earliest block");
            Ok(self
                .provider
                .get_block_by_id(BlockId::Earliest)
                .await?
                .map(|x| x.number)
                .unwrap_or(BlockNumber(0)))
        }
    }
}

#[derive(Debug, Clone, Envconfig)]
pub struct IndexerConfig {
    #[envconfig(from = "ETH_CONTRACT_ADDRESS")]
    pub contract_address: Address,
    #[envconfig(from = "ETH_BLOCK_BATCH_SIZE")]
    pub block_batch_size: u64,
    #[envconfig(from = "ETH_INDEXER_RUN_INTERVAL_MS")]
    pub run_interval_ms: u64,
    #[envconfig(from = "ETH_INDEXER_TIP_BLOCK")]
    pub tip_block: TipBlock,
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        Address, Block, BlockHash, BlockId, BlockNumber, BlockTimestamp, EthTransfer,
        EthTransferBatch, LogIndex, TipBlock, TransferValue, TxHash,
    };
    use crate::eth::provider::EthProvider;
    use crate::indexer::{IndexerConfig, LogIndexer};
    use crate::store::Store;
    use alloy::primitives::{U256, address, b256};
    use std::ops::RangeInclusive;
    use std::str::FromStr;
    use std::sync::{Arc, Mutex};

    const BLOCK_BATCH_SIZE: u64 = 1000;
    const LATEST_BLOCK_NUMBER: BlockNumber = BlockNumber(21500213);

    #[tokio::test]
    async fn test_run_once() -> eyre::Result<()> {
        tracing_subscriber::fmt::init();

        let cfg = IndexerConfig {
            contract_address: Address(address!("0x68614481aef06e53d23bbe0772343fb555ac40c8")),
            block_batch_size: BLOCK_BATCH_SIZE,
            run_interval_ms: 1000,
            tip_block: TipBlock::Latest,
        };

        let eth_provider = Arc::new(MockEthProvider::new());
        let store = Arc::new(MockStore::new());

        let indexer = LogIndexer::new(cfg, eth_provider.clone(), store.clone());
        let _ = indexer.tick().await?;

        let captured_batches = store.captured_batches();
        assert_eq!(captured_batches, expected_captured_batches());

        let captured_ranges = eth_provider.captured_ranges();
        assert_eq!(captured_ranges, expected_captured_block_ranges());

        Ok(())
    }

    fn expected_captured_block_ranges() -> Vec<RangeInclusive<BlockNumber>> {
        vec![
            BlockNumber(21497214)..=BlockNumber(21498213),
            BlockNumber(21498214)..=BlockNumber(21499213),
            BlockNumber(21499214)..=LATEST_BLOCK_NUMBER,
        ]
    }

    fn expected_captured_batches() -> Vec<EthTransferBatch> {
        vec![
            EthTransferBatch {
                transfers: vec![],
                last_block: BlockNumber(21498213),
                non_final_blocks: vec![],
            },
            EthTransferBatch {
                transfers: vec![],
                last_block: BlockNumber(21499213),
                non_final_blocks: vec![],
            },
            EthTransferBatch {
                transfers: expected_transfers(),
                last_block: LATEST_BLOCK_NUMBER,
                non_final_blocks: vec![],
            },
        ]
    }

    fn expected_transfers() -> Vec<EthTransfer> {
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

    pub struct MockStore {
        pub captured_batches: Mutex<Vec<EthTransferBatch>>,
    }

    impl MockStore {
        pub fn new() -> Self {
            Self {
                captured_batches: Mutex::new(vec![]),
            }
        }

        pub fn captured_batches(&self) -> Vec<EthTransferBatch> {
            self.captured_batches.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl Store for MockStore {
        async fn save_transfer_batch(&self, batch: EthTransferBatch) -> eyre::Result<()> {
            let mut guard = self.captured_batches.lock().unwrap();
            guard.push(batch);
            Ok(())
        }

        async fn get_last_indexed_block(&self) -> eyre::Result<Option<BlockNumber>> {
            let block = BlockNumber(21497213);
            Ok(Some(block))
        }

        async fn get_tip_block(&self) -> eyre::Result<Option<Block>> {
            Ok(None)
        }

        async fn drop_blocks_before(&self, _block_number: BlockNumber) -> eyre::Result<()> {
            Ok(())
        }

        async fn rewind_back(&self, _block_number: BlockNumber) -> eyre::Result<()> {
            Ok(())
        }
    }

    pub struct MockEthProvider {
        pub captured_ranges: Mutex<Vec<RangeInclusive<BlockNumber>>>,
    }

    impl MockEthProvider {
        pub fn new() -> Self {
            Self {
                captured_ranges: Mutex::new(vec![]),
            }
        }

        pub fn captured_ranges(&self) -> Vec<RangeInclusive<BlockNumber>> {
            self.captured_ranges.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl EthProvider for MockEthProvider {
        async fn fetch_transfer_logs(
            &self,
            _contract_address: Address,
            block_range: RangeInclusive<BlockNumber>,
        ) -> eyre::Result<Vec<EthTransfer>> {
            let end_range = block_range.end().clone();

            let mut guard = self.captured_ranges.lock().unwrap();
            guard.push(block_range);

            if end_range == LATEST_BLOCK_NUMBER {
                // Simulate only last range has logs
                Ok(expected_transfers())
            } else {
                Ok(vec![])
            }
        }

        async fn get_block_by_id(&self, block_id: BlockId) -> eyre::Result<Option<Block>> {
            match block_id {
                BlockId::Earliest => Ok(Some(Block {
                    number: BlockNumber(0),
                    hash: BlockHash(b256!(
                        "d4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3"
                    )),
                    parent_hash: BlockHash(b256!(
                        "0000000000000000000000000000000000000000000000000000000000000000"
                    )),
                })),
                BlockId::Finalized => Ok(Some(Block {
                    number: BlockNumber(21500149),
                    hash: BlockHash(b256!(
                        "be64d0f06fe7b5e8ca6d1c74b3f8df805e2f3fb166b2c513cd823e0229ace520"
                    )),
                    parent_hash: BlockHash(b256!(
                        "e09e2b083e7bf3029bdccf54fcd12f90d1d2a2415df66a7bd81b858d2aa2d164"
                    )),
                })),
                BlockId::Safe => Ok(Some(Block {
                    number: BlockNumber(21500181),
                    hash: BlockHash(b256!(
                        "f718f7c7f960b0cef784982561ccb801c41556761434ff76aacbc10adca802dd"
                    )),
                    parent_hash: BlockHash(b256!(
                        "e8621bf13411c9b5ed3888dd72034666c7c283b8d48489c1e72400662bc620bc"
                    )),
                })),
                BlockId::Latest => Ok(Some(Block {
                    number: LATEST_BLOCK_NUMBER,
                    hash: BlockHash(b256!(
                        "09afa661a1c383fe926015a8df4e38d43035e3b33c24167454b9e4ad772312db"
                    )),
                    parent_hash: BlockHash(b256!(
                        "ac5e1f4e9db5a1ab1b8456862d54f9ed74c5fd6a04a5c61b6805af13b322895d"
                    )),
                })),
                BlockId::Number(_) => Ok(None),
            }
        }
    }
}
