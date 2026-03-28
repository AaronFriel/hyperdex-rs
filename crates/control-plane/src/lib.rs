use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use cluster_config::ClusterNode;
use data_model::{NodeId, Space, SpaceName};
use parking_lot::RwLock;
use placement_core::ClusterLayout;

pub trait Catalog: Send + Sync {
    fn create_space(&self, space: Space) -> Result<()>;
    fn drop_space(&self, name: &str) -> Result<()>;
    fn list_spaces(&self) -> Result<Vec<SpaceName>>;
    fn get_space(&self, name: &str) -> Result<Option<Space>>;
    fn register_daemon(&self, node: ClusterNode) -> Result<bool>;
    fn replace_daemons(&self, nodes: Vec<ClusterNode>) -> Result<bool>;
    fn layout(&self) -> Result<ClusterLayout>;
}

pub struct InMemoryCatalog {
    spaces: RwLock<BTreeMap<SpaceName, Space>>,
    nodes: RwLock<BTreeMap<NodeId, ClusterNode>>,
    replicas: usize,
}

impl InMemoryCatalog {
    pub fn new(nodes: Vec<ClusterNode>, replicas: usize) -> Self {
        Self {
            spaces: RwLock::new(BTreeMap::new()),
            nodes: RwLock::new(nodes.into_iter().map(|node| (node.id, node)).collect()),
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

    fn register_daemon(&self, node: ClusterNode) -> Result<bool> {
        let mut nodes = self.nodes.write();
        let changed = match nodes.insert(node.id, node.clone()) {
            Some(existing) => existing != node,
            None => true,
        };
        Ok(changed)
    }

    fn replace_daemons(&self, nodes: Vec<ClusterNode>) -> Result<bool> {
        let mut guard = self.nodes.write();
        let next = nodes
            .into_iter()
            .map(|node| (node.id, node))
            .collect::<BTreeMap<_, _>>();
        let changed = *guard != next;
        *guard = next;
        Ok(changed)
    }

    fn layout(&self) -> Result<ClusterLayout> {
        let nodes = self.nodes.read();
        Ok(ClusterLayout {
            replicas: self.replicas,
            nodes: nodes.values().map(|node| node.id).collect(),
        })
    }
}

#[cfg(test)]
mod tests;
