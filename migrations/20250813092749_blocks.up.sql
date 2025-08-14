CREATE TABLE IF NOT EXISTS blocks
(
    number       INTEGER NOT NULL PRIMARY KEY,
    hash         BLOB    NOT NULL CHECK (length(hash) = 32),
    parent_hash  BLOB    NOT NULL CHECK (length(hash) = 32)
);

CREATE INDEX IF NOT EXISTS transfers_block_num_dx on transfers(block_number DESC);