use crate::domain::{Block, BlockNumber, EthTransfer, MetaKey, Metadata};
use sqlx::{Executor, Sqlite};

#[allow(unused)]
pub async fn list_transfers<'c, E>(executor: E, limit: u16) -> eyre::Result<Vec<EthTransfer>>
where
    E: Executor<'c, Database = Sqlite>,
{
    let rows = sqlx::query_as!(EthTransfer, "SELECT * FROM transfers LIMIT ?", limit)
        .fetch_all(executor)
        .await?;

    Ok(rows)
}

pub async fn save_transfer<'c, E>(executor: E, transfer: &EthTransfer) -> eyre::Result<()>
where
    E: Executor<'c, Database = Sqlite>,
{
    let tx_hash = &transfer.tx_hash;
    let transfer_value = &transfer.value;
    let contract_address = &transfer.contract_address;
    let from_address = &transfer.from_address;
    let to_address = &transfer.to_address;
    sqlx::query!(
        "INSERT INTO transfers (
            tx_hash, contract_address, from_address, to_address, value,
            block_number, block_timestamp, log_index
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT (tx_hash, log_index) DO NOTHING",
        tx_hash,
        contract_address,
        from_address,
        to_address,
        transfer_value,
        transfer.block_number,
        transfer.block_timestamp,
        transfer.log_index,
    )
    .execute(executor)
    .await
    .map(|_| ())
    .map_err(Into::into)
}

pub async fn save_metadata<'c, E>(executor: E, metadata: Metadata) -> eyre::Result<()>
where
    E: Executor<'c, Database = Sqlite>,
{
    let meta_key = metadata.key();
    let meta_value = metadata.value();
    sqlx::query!(
        "INSERT INTO metadata(key, value) VALUES (?, ?) ON CONFLICT (key) DO UPDATE SET value = ?",
        meta_key,
        meta_value,
        meta_value,
    )
    .execute(executor)
    .await
    .map(|_| ())
    .map_err(Into::into)
}

pub async fn get_metadata_value<'c, E>(executor: E, key: MetaKey) -> eyre::Result<Option<String>>
where
    E: Executor<'c, Database = Sqlite>,
{
    let str_key = key.as_str();
    let row = sqlx::query!("SELECT value FROM metadata WHERE key = ?", str_key)
        .fetch_optional(executor)
        .await?;

    Ok(row.map(|r| r.value))
}

pub async fn drop_transfers<'c, E>(executor: E, block_number: BlockNumber) -> eyre::Result<u64>
where
    E: Executor<'c, Database = Sqlite>,
{
    let rows = sqlx::query!("DELETE FROM transfers WHERE block_number = ?", block_number)
        .execute(executor)
        .await?
        .rows_affected();

    Ok(rows)
}

pub async fn save_block<'c, E>(executor: E, block: &Block) -> eyre::Result<()>
where
    E: Executor<'c, Database = Sqlite>,
{
    let hash = &block.hash;
    let parent_hash = &block.parent_hash;
    sqlx::query!(
        "INSERT INTO blocks (
            number, hash, parent_hash
        ) VALUES (?, ?, ?) ON CONFLICT (number) DO NOTHING",
        block.number,
        hash,
        parent_hash,
    )
    .execute(executor)
    .await
    .map(|_| ())
    .map_err(Into::into)
}

pub async fn get_latest_block<'c, E>(executor: E) -> eyre::Result<Option<Block>>
where
    E: Executor<'c, Database = Sqlite>,
{
    let row = sqlx::query_as!(Block, "SELECT * FROM blocks ORDER BY number DESC LIMIT 1")
        .fetch_optional(executor)
        .await?;

    Ok(row)
}

pub async fn drop_blocks_before<'c, E>(executor: E, block_number: BlockNumber) -> eyre::Result<u64>
where
    E: Executor<'c, Database = Sqlite>,
{
    let rows = sqlx::query!("DELETE FROM blocks WHERE number < ?", block_number)
        .execute(executor)
        .await?
        .rows_affected();

    Ok(rows)
}

pub async fn drop_block<'c, E>(executor: E, block_number: BlockNumber) -> eyre::Result<u64>
where
    E: Executor<'c, Database = Sqlite>,
{
    let rows = sqlx::query!("DELETE FROM blocks WHERE number = ?", block_number)
        .execute(executor)
        .await?
        .rows_affected();

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use crate::domain::*;
    use alloy::primitives::{U256, address, b256};

    async fn in_mem_pool() -> eyre::Result<sqlx::Pool<sqlx::Sqlite>> {
        let pool = sqlx::sqlite::SqlitePool::connect(":memory:").await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(pool)
    }

    #[tokio::test]
    async fn test_transfers() -> eyre::Result<()> {
        let pool = in_mem_pool().await?;

        let transfers = super::list_transfers(&pool, 10).await?;
        assert!(
            transfers.is_empty(),
            "Expected no transfers in an empty database"
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

        super::save_transfer(&pool, &transfer).await?;

        let transfers = super::list_transfers(&pool, 10).await?;

        assert_eq!(transfers.len(), 1, "Expected one transfer to be saved");
        assert_eq!(&transfers[0], &transfer);

        Ok(())
    }

    #[tokio::test]
    async fn test_metadata() -> eyre::Result<()> {
        let pool = in_mem_pool().await?;

        let value = super::get_metadata_value(&pool, MetaKey::LastIndexedBlock).await?;
        assert_eq!(value, None);

        let meta_value = Metadata::LastIndexedBlock(BlockNumber(123456));

        super::save_metadata(&pool, meta_value).await?;

        let value = super::get_metadata_value(&pool, MetaKey::LastIndexedBlock).await?;
        assert_eq!(value, Some("123456".to_string()));

        Ok(())
    }
}
