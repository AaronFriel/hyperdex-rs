use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use cluster_config::ClusterNode;
use data_model::{Space, SpaceName};
use parking_lot::RwLock;
use placement_core::ClusterLayout;

pub trait Catalog: Send + Sync {
    fn create_space(&self, space: Space) -> Result<()>;
    fn drop_space(&self, name: &str) -> Result<()>;
    fn list_spaces(&self) -> Result<Vec<SpaceName>>;
    fn get_space(&self, name: &str) -> Result<Option<Space>>;
    fn layout(&self) -> Result<ClusterLayout>;
}

pub struct InMemoryCatalog {
    spaces: RwLock<BTreeMap<SpaceName, Space>>,
    nodes: Vec<ClusterNode>,
    replicas: usize,
}

impl InMemoryCatalog {
    pub fn new(nodes: Vec<ClusterNode>, replicas: usize) -> Self {
        Self {
            spaces: RwLock::new(BTreeMap::new()),
            nodes,
            replicas,
        }
    }
}

impl Catalog for InMemoryCatalog {
    fn create_space(&self, space: Space) -> Result<()> {
        let mut guard = self.spaces.write();
        if guard.contains_key(&space.name) {
            return Err(anyhow!("space {} already exists", space.name));
        }
        guard.insert(space.name.clone(), space);
        Ok(())
    }

    fn drop_space(&self, name: &str) -> Result<()> {
        self.spaces.write().remove(name);
        Ok(())
    }

    fn list_spaces(&self) -> Result<Vec<SpaceName>> {
        Ok(self.spaces.read().keys().cloned().collect())
    }

    fn get_space(&self, name: &str) -> Result<Option<Space>> {
        Ok(self.spaces.read().get(name).cloned())
    }

    fn layout(&self) -> Result<ClusterLayout> {
        Ok(ClusterLayout {
            replicas: self.replicas,
            nodes: self.nodes.iter().map(|node| node.id).collect(),
        })
    }
}
