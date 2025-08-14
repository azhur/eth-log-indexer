use crate::domain::{Block, BlockNumber, EthTransferBatch};

pub mod sqlite;

#[async_trait::async_trait]
pub trait Store {
    async fn save_transfer_batch(&self, batch: EthTransferBatch) -> eyre::Result<()>;
    async fn get_last_indexed_block(&self) -> eyre::Result<Option<BlockNumber>>;
    async fn get_tip_block(&self) -> eyre::Result<Option<Block>>;
    async fn drop_blocks_before(&self, block_number: BlockNumber) -> eyre::Result<()>;
    async fn rewind_back(&self, block_number: BlockNumber) -> eyre::Result<()>;
}
