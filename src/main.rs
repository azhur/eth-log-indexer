use crate::eth::provider::AlloyEthProviderBuilder;
use envconfig::Envconfig;
use std::sync::Arc;
use tracing::level_filters::LevelFilter;

mod config;
mod domain;
mod eth;
mod indexer;
mod store;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenvy::dotenv()?;

    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::INFO) // move level to config
        .init();

    let config = config::Config::init_from_env()?;

    let eth_provider = AlloyEthProviderBuilder::new_http(config.eth)?;

    let store = store::sqlite::SqliteStore::init(config.db).await?;

    let indexer = indexer::LogIndexer::new(
        config.indexer.clone(),
        Arc::new(eth_provider),
        Arc::new(store),
    );

    indexer.run().await
}
