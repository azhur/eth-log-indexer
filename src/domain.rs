use alloy::eips::BlockNumberOrTag;
use alloy::primitives::{Address as AlloyAddress, B256, U256};
use alloy::rpc::types::Block as AlloyBlock;
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};
use std::borrow::Cow;
use std::ops::{Add, Sub};
use std::str::FromStr;
use std::string::ToString;

/// Domain (model) objects used across different layers of the application.

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct BlockNumber(pub u64);
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct BlockTimestamp(pub u64);
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Address(pub AlloyAddress);
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct LogIndex(pub u64);
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct TransferValue(pub U256);
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct TxHash(pub B256);
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct BlockHash(pub B256);

#[derive(Debug, Copy, Clone, sqlx::FromRow, Eq, PartialEq)]
pub struct Block {
    pub number: BlockNumber,
    pub hash: BlockHash,
    pub parent_hash: BlockHash,
}

#[derive(Debug, Copy, Clone, sqlx::FromRow, Eq, PartialEq)]
pub struct EthTransfer {
    pub tx_hash: TxHash,
    pub log_index: LogIndex,
    pub contract_address: Address,
    pub block_number: BlockNumber,
    pub block_timestamp: BlockTimestamp,
    pub from_address: Address,
    pub to_address: Address,
    pub value: TransferValue,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EthTransferBatch {
    pub transfers: Vec<EthTransfer>,
    pub last_block: BlockNumber,
}

#[derive(Debug, Copy, Clone)]
pub enum Metadata {
    LastIndexedBlock(BlockNumber),
}

#[derive(Debug, Copy, Clone)]
pub enum MetaKey {
    LastIndexedBlock,
}

impl MetaKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            MetaKey::LastIndexedBlock => "last_indexer_block",
        }
    }
}

impl Metadata {
    pub fn key(&self) -> &'static str {
        match self {
            Metadata::LastIndexedBlock(_) => MetaKey::LastIndexedBlock.as_str(),
        }
    }

    pub fn value(&self) -> String {
        match self {
            Metadata::LastIndexedBlock(block_number) => block_number.0.to_string(),
        }
    }
}

pub enum BlockId {
    Earliest,
    Finalized,
    Safe,
    Latest,
}

#[derive(Debug, Copy, Clone)]
pub enum TipBlock {
    /// the last finalized block, theoretically immutable and reorg-free
    Finalized,
    /// the last safe block, very unlikely to be replaced by a reorg
    Safe,
    /// the latest block, which is the most recent block in the chain
    Latest,
}

impl From<TipBlock> for BlockId {
    fn from(tip_block: TipBlock) -> Self {
        match tip_block {
            TipBlock::Finalized => BlockId::Finalized,
            TipBlock::Safe => BlockId::Safe,
            TipBlock::Latest => BlockId::Latest,
        }
    }
}

impl FromStr for TipBlock {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "finalized" => Ok(TipBlock::Finalized),
            "safe" => Ok(TipBlock::Safe),
            "latest" => Ok(TipBlock::Latest),
            _ => Err(eyre::eyre!("Invalid TipBlock value: {}", s)),
        }
    }
}

impl From<BlockId> for alloy::eips::BlockId {
    fn from(block_id: BlockId) -> Self {
        match block_id {
            BlockId::Earliest => alloy::eips::BlockId::Number(BlockNumberOrTag::Earliest),
            BlockId::Finalized => alloy::eips::BlockId::Number(BlockNumberOrTag::Finalized),
            BlockId::Safe => alloy::eips::BlockId::Number(BlockNumberOrTag::Safe),
            BlockId::Latest => alloy::eips::BlockId::Number(BlockNumberOrTag::Latest),
        }
    }
}

impl From<u64> for BlockNumber {
    fn from(value: u64) -> Self {
        BlockNumber(value)
    }
}

impl From<u64> for BlockTimestamp {
    fn from(value: u64) -> Self {
        BlockTimestamp(value)
    }
}

impl From<u64> for LogIndex {
    fn from(value: u64) -> Self {
        LogIndex(value)
    }
}

impl From<i64> for BlockNumber {
    fn from(value: i64) -> Self {
        BlockNumber(value as u64)
    }
}

impl From<i64> for BlockTimestamp {
    fn from(value: i64) -> Self {
        BlockTimestamp(value as u64)
    }
}

impl From<i64> for LogIndex {
    fn from(value: i64) -> Self {
        LogIndex(value as u64)
    }
}

impl FromStr for Address {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Address(AlloyAddress::from_str(s)?))
    }
}

impl From<Vec<u8>> for TxHash {
    fn from(bytes: Vec<u8>) -> Self {
        TxHash(B256::from_slice(&bytes))
    }
}

impl From<Vec<u8>> for Address {
    fn from(bytes: Vec<u8>) -> Self {
        Address(AlloyAddress::from_slice(&bytes))
    }
}

impl From<Vec<u8>> for TransferValue {
    fn from(bytes: Vec<u8>) -> Self {
        TransferValue(U256::from_le_slice(&bytes))
    }
}

impl Sub<u64> for BlockNumber {
    type Output = BlockNumber;

    fn sub(self, rhs: u64) -> Self::Output {
        BlockNumber(self.0 - rhs)
    }
}

impl Add<u64> for BlockNumber {
    type Output = BlockNumber;

    fn add(self, rhs: u64) -> Self::Output {
        BlockNumber(self.0 + rhs)
    }
}

impl From<AlloyBlock> for Block {
    fn from(block: AlloyBlock) -> Self {
        Block {
            number: BlockNumber(block.header.number),
            hash: BlockHash(block.header.hash),
            parent_hash: BlockHash(block.header.parent_hash),
        }
    }
}

impl FromStr for BlockNumber {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let number = s.parse::<u64>()?;
        Ok(BlockNumber(number))
    }
}

// Those below can be reduced via macros
// NOTE:
//  - sqlx doesn't work with u64, using i64 for ethereum type should be fine as the real values should not exceed i64 max.
//  - the B256 and U256 encoders are using &T instead of T to avoid unnecessary allocations.
impl Type<Sqlite> for TxHash {
    fn type_info() -> SqliteTypeInfo {
        <Vec<u8> as Type<Sqlite>>::type_info()
    }
}
impl Type<Sqlite> for Address {
    fn type_info() -> SqliteTypeInfo {
        <Vec<u8> as Type<Sqlite>>::type_info()
    }
}
impl Type<Sqlite> for TransferValue {
    fn type_info() -> SqliteTypeInfo {
        <Vec<u8> as Type<Sqlite>>::type_info()
    }
}
impl Type<Sqlite> for BlockNumber {
    fn type_info() -> SqliteTypeInfo {
        <i64 as Type<Sqlite>>::type_info()
    }
}
impl Type<Sqlite> for BlockTimestamp {
    fn type_info() -> SqliteTypeInfo {
        <i64 as Type<Sqlite>>::type_info()
    }
}
impl Type<Sqlite> for LogIndex {
    fn type_info() -> SqliteTypeInfo {
        <i64 as Type<Sqlite>>::type_info()
    }
}

impl<'r> Decode<'r, Sqlite> for TxHash {
    fn decode(v: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        let bytes: &[u8] = <&[u8] as Decode<Sqlite>>::decode(v)?;
        Ok(TxHash(B256::from_slice(bytes)))
    }
}

impl<'r> Decode<'r, Sqlite> for BlockHash {
    fn decode(v: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        let bytes: &[u8] = <&[u8] as Decode<Sqlite>>::decode(v)?;
        Ok(BlockHash(B256::from_slice(bytes)))
    }
}

impl<'r> Decode<'r, Sqlite> for Address {
    fn decode(v: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        let bytes: &[u8] = <&[u8] as Decode<Sqlite>>::decode(v)?;
        Ok(Address(AlloyAddress::from_slice(bytes)))
    }
}
impl<'r> Decode<'r, Sqlite> for TransferValue {
    fn decode(v: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        let bytes: &[u8] = <&[u8] as Decode<Sqlite>>::decode(v)?;
        Ok(TransferValue(U256::from_le_slice(bytes)))
    }
}
impl<'r> Decode<'r, Sqlite> for BlockNumber {
    fn decode(v: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        let i: i64 = <i64 as Decode<Sqlite>>::decode(v)?;
        if i < 0 {
            return Err("block_number must be >= 0".into());
        }
        Ok(BlockNumber(i as u64))
    }
}
impl<'r> Decode<'r, Sqlite> for BlockTimestamp {
    fn decode(v: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        let i: i64 = <i64 as Decode<Sqlite>>::decode(v)?;
        if i < 0 {
            return Err("block_timestamp must be >= 0".into());
        }
        Ok(BlockTimestamp(i as u64))
    }
}
impl<'r> Decode<'r, Sqlite> for LogIndex {
    fn decode(v: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        let i: i64 = <i64 as Decode<Sqlite>>::decode(v)?;
        if i < 0 {
            return Err("log_index must be >= 0".into());
        }
        Ok(LogIndex(i as u64))
    }
}

// BLOB encodings
impl<'q> Encode<'q, Sqlite> for &'q TxHash {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Blob(Cow::Borrowed(self.0.as_slice())));
        Ok(IsNull::No)
    }
}

impl<'q> Encode<'q, Sqlite> for &'q BlockHash {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Blob(Cow::Borrowed(self.0.as_slice())));
        Ok(IsNull::No)
    }
}

impl<'q> Encode<'q, Sqlite> for &'q Address {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Blob(Cow::Borrowed(self.0.as_slice())));
        Ok(IsNull::No)
    }
}

impl<'q> Encode<'q, Sqlite> for &'q TransferValue {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        args.push(SqliteArgumentValue::Blob(self.0.as_le_bytes()));
        Ok(IsNull::No)
    }
}

impl<'q> Encode<'q, Sqlite> for BlockNumber {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        let v = i64::try_from(self.0).expect("BlockNumber fits into i64");
        args.push(SqliteArgumentValue::Int64(v));
        Ok(IsNull::No)
    }
}

impl<'q> Encode<'q, Sqlite> for BlockTimestamp {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        let v = i64::try_from(self.0).expect("BlockTimestamp fits into i64");
        args.push(SqliteArgumentValue::Int64(v));
        Ok(IsNull::No)
    }
}

impl<'q> Encode<'q, Sqlite> for LogIndex {
    fn encode_by_ref(
        &self,
        args: &mut Vec<SqliteArgumentValue<'q>>,
    ) -> Result<IsNull, BoxDynError> {
        let v = i64::try_from(self.0).expect("LogIndex fits into i64");
        args.push(SqliteArgumentValue::Int64(v));
        Ok(IsNull::No)
    }
}
