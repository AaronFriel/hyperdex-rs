use std::sync::Arc;

use anyhow::{Result, anyhow};
use bytes::Bytes;
use control_plane::Catalog;
use data_model::{Check, Mutation, Record};
use placement_core::PlacementStrategy;
use storage_core::{StorageEngine, WriteResult};

pub struct DataPlane {
    catalog: Arc<dyn Catalog>,
    storage: Arc<dyn StorageEngine>,
    placement: Arc<dyn PlacementStrategy>,
}

impl DataPlane {
    pub fn new(
        catalog: Arc<dyn Catalog>,
        storage: Arc<dyn StorageEngine>,
        placement: Arc<dyn PlacementStrategy>,
    ) -> Self {
        Self {
            catalog,
            storage,
            placement,
        }
    }

    pub fn put(&self, space: &str, key: Bytes, mutations: &[Mutation]) -> Result<WriteResult> {
        self.ensure_space_exists(space)?;
        let _ = self.route(space, &key)?;
        self.storage.put(space, key, mutations)
    }

    pub fn get(&self, space: &str, key: &[u8]) -> Result<Option<Record>> {
        self.ensure_space_exists(space)?;
        let _ = self.route(space, key)?;
        self.storage.get(space, key)
    }

    pub fn delete(&self, space: &str, key: &[u8]) -> Result<WriteResult> {
        self.ensure_space_exists(space)?;
        let _ = self.route(space, key)?;
        self.storage.delete(space, key)
    }

    pub fn conditional_put(
        &self,
        space: &str,
        key: Bytes,
        checks: &[Check],
        mutations: &[Mutation],
    ) -> Result<WriteResult> {
        self.ensure_space_exists(space)?;
        let _ = self.route(space, &key)?;
        self.storage.conditional_put(space, key, checks, mutations)
    }

    pub fn search(&self, space: &str, checks: &[Check]) -> Result<Vec<Record>> {
        self.ensure_space_exists(space)?;
        self.storage.search(space, checks)
    }

    pub fn count(&self, space: &str, checks: &[Check]) -> Result<u64> {
        self.ensure_space_exists(space)?;
        self.storage.count(space, checks)
    }

    pub fn delete_matching(&self, space: &str, checks: &[Check]) -> Result<u64> {
        self.ensure_space_exists(space)?;
        self.storage.delete_matching(space, checks)
    }

    fn ensure_space_exists(&self, space: &str) -> Result<()> {
        if self.catalog.get_space(space)?.is_none() {
            return Err(anyhow!("space {space} does not exist"));
        }
        Ok(())
    }

    fn route(&self, _space: &str, key: &[u8]) -> Result<u64> {
        let layout = self.catalog.layout()?;
        Ok(self.placement.locate(key, &layout)?.primary)
    }
}
