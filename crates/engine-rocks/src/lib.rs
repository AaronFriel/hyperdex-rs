use std::sync::Arc;

use anyhow::Result;
use bytes::Bytes;
use data_model::{Check, Mutation, Record, SpaceName};
use rocksdb::{DBWithThreadMode, MultiThreaded, Options};
use serde_json::{from_slice, to_vec};
use storage_core::{StorageEngine, WriteResult};

pub struct RocksEngine {
    db: Arc<DBWithThreadMode<MultiThreaded>>,
}

impl RocksEngine {
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        let db = DBWithThreadMode::open(&options, path)?;
        Ok(Self { db: Arc::new(db) })
    }

    fn key(space: &str, key: &[u8]) -> Vec<u8> {
        let mut encoded = space.as_bytes().to_vec();
        encoded.push(0);
        encoded.extend_from_slice(key);
        encoded
    }
}

impl StorageEngine for RocksEngine {
    fn put(&self, space: &str, key: Bytes, mutations: &[Mutation]) -> Result<WriteResult> {
        let mut record = self
            .get(space, &key)?
            .unwrap_or_else(|| Record::new(key.clone()));

        for mutation in mutations {
            if let Mutation::Set(attribute) = mutation {
                record
                    .attributes
                    .insert(attribute.name.clone(), attribute.value.clone());
            }
        }

        self.db.put(Self::key(space, &key), to_vec(&record)?)?;
        Ok(WriteResult::Written)
    }

    fn get(&self, space: &str, key: &[u8]) -> Result<Option<Record>> {
        let Some(bytes) = self.db.get(Self::key(space, key))? else {
            return Ok(None);
        };
        Ok(Some(from_slice(&bytes)?))
    }

    fn delete(&self, space: &str, key: &[u8]) -> Result<WriteResult> {
        self.db.delete(Self::key(space, key))?;
        Ok(WriteResult::Written)
    }

    fn conditional_put(
        &self,
        space: &str,
        key: Bytes,
        _checks: &[Check],
        mutations: &[Mutation],
    ) -> Result<WriteResult> {
        self.put(space, key, mutations)
    }

    fn search(&self, _space: &str, _checks: &[Check]) -> Result<Vec<Record>> {
        Ok(Vec::new())
    }

    fn count(&self, _space: &str, _checks: &[Check]) -> Result<u64> {
        Ok(0)
    }

    fn delete_matching(&self, _space: &str, _checks: &[Check]) -> Result<u64> {
        Ok(0)
    }

    fn spaces(&self) -> Result<Vec<SpaceName>> {
        Ok(Vec::new())
    }

    fn create_space(&self, _space: SpaceName) -> Result<()> {
        Ok(())
    }

    fn drop_space(&self, _space: &str) -> Result<()> {
        Ok(())
    }
}
