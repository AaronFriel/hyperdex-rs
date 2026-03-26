use anyhow::Result;
use bytes::Bytes;
use data_model::{Check, Mutation, Record, SpaceName};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteResult {
    Written,
    ConditionFailed,
    Missing,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("space {0} does not exist")]
    UnknownSpace(String),
    #[error("numeric mutation attempted on a non-integer value")]
    NonNumericValue,
}

pub trait StorageEngine: Send + Sync {
    fn put(&self, space: &str, key: Bytes, mutations: &[Mutation]) -> Result<WriteResult>;
    fn get(&self, space: &str, key: &[u8]) -> Result<Option<Record>>;
    fn delete(&self, space: &str, key: &[u8]) -> Result<WriteResult>;
    fn conditional_put(
        &self,
        space: &str,
        key: Bytes,
        checks: &[Check],
        mutations: &[Mutation],
    ) -> Result<WriteResult>;
    fn search(&self, space: &str, checks: &[Check]) -> Result<Vec<Record>>;
    fn count(&self, space: &str, checks: &[Check]) -> Result<u64>;
    fn delete_matching(&self, space: &str, checks: &[Check]) -> Result<u64>;
    fn spaces(&self) -> Result<Vec<SpaceName>>;
    fn create_space(&self, space: SpaceName) -> Result<()>;
    fn drop_space(&self, space: &str) -> Result<()>;
}
