# ERC-20 Transfer Indexer
- [Task requirements](docs/requirements.md)
- [Implementation report](docs/report.md)

Indexes the entire chain for ERC‑20 Transfer events for a specific contract address.</br>
Persists the index progress in SQLite database, safely resumes from the last processed block.

## Running locally
1. install rust toolchain: https://www.rust-lang.org/tools/install
2. `make` - inits the project and prepares the local environment
3. `make test` - runs all tests
4. `make run` - runs the indexer locally, the configuration is loaded from .env file.
5. `make lint` - runs the linter
6. `make fmt-check` - formats the code
