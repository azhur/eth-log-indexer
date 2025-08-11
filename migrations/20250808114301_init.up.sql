CREATE TABLE IF NOT EXISTS transfers
(
    tx_hash          BLOB    NOT NULL CHECK (length(tx_hash) = 32),
    log_index        INTEGER NOT NULL,
    contract_address BLOB    NOT NULL CHECK (length(contract_address) = 20),
    block_number     INTEGER NOT NULL,
    block_timestamp  INTEGER NOT NULL,
    from_address     BLOB NOT NULL CHECK (length(from_address)=20),
    to_address       BLOB NOT NULL CHECK (length(from_address)=20),
    value            BLOB NOT NULL CHECK (length(value)=32),
    PRIMARY KEY (tx_hash, log_index)
) STRICT, WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS metadata
(
    key   TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL
);
