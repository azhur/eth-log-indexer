pub mod config;
mod query;

use crate::domain::{BlockNumber, EthTransferBatch, MetaKey, Metadata};
use crate::store::Store;
use crate::store::sqlite::config::DatabaseConfig;
use sqlx::Pool;
use std::str::FromStr;

pub struct SqliteStore {
    pool: Pool<sqlx::Sqlite>,
}

impl SqliteStore {
    pub async fn init(config: DatabaseConfig) -> eyre::Result<Self> {
        // todo tune the pool and connection parameters
        let pool = Pool::connect(config.database_url.as_ref()).await?;

        sqlx::migrate!().run(&pool).await?;

        Ok(Self { pool })
    }
}

#[async_trait::async_trait]
impl Store for SqliteStore {
    async fn save_transfer_batch(&self, batch: EthTransferBatch) -> eyre::Result<()> {
        let size = batch.transfers.len();

        let mut tx = self.pool.begin().await?;
        // todo optimize, combine multiple transfer inserts into a single batch insert
        for transfer in &batch.transfers {
            query::save_transfer(&mut *tx, transfer).await?;
        }
        query::save_metadata(&mut *tx, Metadata::LastIndexedBlock(batch.last_block)).await?;
        tx.commit().await?;

        tracing::info!(
            "Stored {:?} transfer logs, last block {:?}",
            size,
            batch.last_block
        );

        Ok(())
    }

    async fn get_last_indexed_block(&self) -> eyre::Result<Option<BlockNumber>> {
        query::get_metadata_value(&self.pool, MetaKey::LastIndexedBlock)
            .await?
            .map(|val| BlockNumber::from_str(val.as_str()))
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        Address, BlockNumber, BlockTimestamp, EthTransfer, EthTransferBatch, LogIndex,
        TransferValue, TxHash,
    };
    use alloy::primitives::{U256, address, b256};

    #[tokio::test]
    async fn test_sqlite_store() -> eyre::Result<()> {
        let config = DatabaseConfig {
            database_url: ":memory:".to_string(),
        };
        let store = SqliteStore::init(config).await?;

        let last_indexed_block = store.get_last_indexed_block().await?;
        assert!(
            last_indexed_block.is_none(),
            "Expected no indexed block in an empty store"
        );

        let transfer = EthTransfer {
            tx_hash: TxHash(b256!(
                "bf7e331f7f7c1dd2e05159666b3bf8bc7a8a3a9eb1d518969eab529dd9b88c1a"
            )),
            log_index: LogIndex(2),
            contract_address: Address(address!("0xd8da6bf26964af9d7eed9e03e53415d37aa96045")),
            block_number: BlockNumber(123456),
            block_timestamp: BlockTimestamp(1622547800),
            from_address: Address(address!("0xd8da6bf26964af9d7eed9e03e53415d37aa96046")),
            to_address: Address(address!("0xd8da6bf26964af9d7eed9e03e53415d37aa96047")),
            value: TransferValue(U256::from(42)),
        };

        let batch = EthTransferBatch {
            transfers: vec![transfer],
            last_block: BlockNumber(123456),
        };

        store.save_transfer_batch(batch).await?;

        let last_indexed_block = store.get_last_indexed_block().await?;
        assert_eq!(last_indexed_block, Some(BlockNumber(123456)));

        Ok(())
    }
}
