use crate::domain::{BlockNumber, EthTransferBatch};

pub mod sqlite;

#[async_trait::async_trait]
pub trait Store {
    async fn save_transfer_batch(&self, batch: EthTransferBatch) -> eyre::Result<()>;
    async fn get_last_indexed_block(&self) -> eyre::Result<Option<BlockNumber>>;
}
