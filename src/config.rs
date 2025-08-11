use crate::eth::config::EthConfig;
use crate::indexer::IndexerConfig;
use crate::store::sqlite::config::DatabaseConfig;
use envconfig::Envconfig;

#[derive(Debug, Clone, Envconfig)]
pub struct Config {
    #[envconfig(nested)]
    pub eth: EthConfig,
    #[envconfig(nested)]
    pub db: DatabaseConfig,
    #[envconfig(nested)]
    pub indexer: IndexerConfig,
}
