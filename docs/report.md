# Implementation report

## Met requirements 
- `JSON-RPC use via ethers-rs` - using `alloy` crate which is `ethers-rs`successor.
- `Log filtering and decoding` - done in fetch_transfer_logs in AlloyEthProvider implementation.
- `Efficient data storage and deduplication` - BLOB column type is used for efficient storage, deduplication handled on the database layer via compound primary key.
- `Data integrity via tx hash + log index` - added transfer table composite primary key.
- `Bonus: Handling reorgs and finality` - not handled yet, one of the possible solution described in [Indexer](#Indexer).
- `Bonus: provide a CLI to query the data you collected` - not implemented yet :(

## Codebase structure
### Application components
 - `domain` - contains light-weight structures for the domain model and the different converters that allow conversion from/to other layers like store and provider.
 - `store` - storage layer abstracted by `Store` trait, currently has only a single SQLite implementation.
 - `eth` - the ethereum provider layer abstracted by `EthProvider` trait.
 - `indexer` - the indexer logic, uses `EthProvider` and `Store` to fetch and store the data.
   
The traits expose only needed functionality for store and ethereum provider and allow easy mocking in indexer tests.</br>
The configuration is injected via environment variables. All of them are described in `.env.sample file` <./br>

### Crates
 - sqlx: compile-time checked queries
 - tokio: most popular async runtime
 - alloy: Ethereum provider, used for fetching transfer logs
 - eyre: convenient error handling
 - envconfig: environment variables configuration style

## Tests
Every component has its own set of tests.</br>
Currently, test coverage is not great, but the code is designed and structured in a way that allows easy addition of more tests.

## Limitations and future improvements
### Indexer
 - use dynamic batch size for fetching transfer logs
    - allows a more optimal data fetch (reducing the number of rpc requests) and handles a possible provider limit error like: max fetch results limit exceeded
 - handle accidental multiple parallel index processes
   - the issue with parallel indexer processes is just the xN pressure on the rpc provider and the database.
 - run parallel rpc fetchers that sink into a single writer.
   - should improve the throughput of the indexer (especially for the initial tick when historical data is fetched)
   - The sqlite doesn't support concurrent writes so there is no sense in sharding the historical range and run multiple parallel indexer processes.
 - handle reorgs and finality
   - NOTE: setting env var `ETH_INDEXER_TIP_BLOCK=finalized` will index only finalized blocks, but the downside is the fall-behind indexer with no fresh logs.
   - a possible simple solution that pops-up right now would be to introduce a new `blocks` table that will store the block number, block hashes and is_finalized() flag.
   - the indexer will store the last finalized block and all it's child blocks until the tip (safe or latest, depending on `ETH_INDEXER_TIP_BLOCK` env var). Shouldn't be many blocks.
   - the historical blocks might be skipped unless they are needed to be queried for other purposes.
   - the reorg process will occur before the index tick starts (for the very first tick it will be no-op).
   - the hash reconciliation will occur for the stored locally tip block against the same block fetched by number from the rpc provider.
   - in case of hash non-match: the block should be re-indexed (blocks record updated and transfers records for that number re-indexed).
   - repeat the process for parent if parent hashes don't match.
   - if the rpc provider returns a finalized block: update as finalized all parent blocks until the first finalized block backwards.

### Ethereum Provider
 - add auth support for RPC endpoints
 - add throttle layer to the rpc client

### Sqlite optimizations
 - The task requirements don't specify the collected data read patterns so it's hard to optimize the database schema/indices without them
 - As for the write operations - a performance test should be done and the sqlite connection parameters should be tuned accordingly
 - Use batch inserts in a single statement

### Others
 - improve error reporting by introducing dedicated errors in all components using `thiserror` crate.
 - configure logging, currently it is a little verbose.
 - add metrics/traces around each component, this will allow to observe the components behaviour.
 - CI/CD

