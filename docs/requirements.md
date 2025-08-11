# Requirements
Ethereum Log Indexer and Verifier

Challenge: Build a service in Rust that connects to an Ethereum RPC,
scans blocks, and stores Transfer events from an ERC-20 contract in a local
SQLite database.

Skills Tested:
 - JSON-RPC use via ethers-rs (alloy)
 - Log filtering and decoding
 - Efficient data storage and deduplication
 - Data integrity via tx hash + log index
 - Bonus: Handling reorgs and finality
 - Bonus: provide a CLI to query the data you collected

For submission, you will need to provide:
 - A Git repository containing all the source code, scripts, and configuration files.
 - Include detailed documentation
 - Optionally, a short video presentation that walks through your solution
 - A report (max 2-3 pages) explaining your approach, your other ideas, what went well or not, etc.