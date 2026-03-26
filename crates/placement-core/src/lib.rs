use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use data_model::NodeId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterLayout {
    pub replicas: usize,
    pub nodes: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementDecision {
    pub primary: NodeId,
    pub replicas: Vec<NodeId>,
}

pub trait PlacementStrategy: Send + Sync {
    fn locate(&self, key: &[u8], layout: &ClusterLayout) -> PlacementDecision;
    fn name(&self) -> &'static str;
}

#[derive(Default)]
pub struct RendezvousPlacement;

#[derive(Default)]
pub struct HyperSpacePlacement;

impl PlacementStrategy for RendezvousPlacement {
    fn locate(&self, key: &[u8], layout: &ClusterLayout) -> PlacementDecision {
        let mut ranked = layout.nodes.clone();
        ranked.sort_by_key(|node| std::cmp::Reverse(score(key, *node)));
        build_decision(ranked, layout.replicas)
    }

    fn name(&self) -> &'static str {
        "rendezvous"
    }
}

impl PlacementStrategy for HyperSpacePlacement {
    fn locate(&self, key: &[u8], layout: &ClusterLayout) -> PlacementDecision {
        let start = (score(key, 0) as usize) % layout.nodes.len();
        let mut ranked = Vec::with_capacity(layout.nodes.len());

        for offset in 0..layout.nodes.len() {
            ranked.push(layout.nodes[(start + offset) % layout.nodes.len()]);
        }

        build_decision(ranked, layout.replicas)
    }

    fn name(&self) -> &'static str {
        "hyperspace"
    }
}

fn build_decision(ranked: Vec<NodeId>, replicas: usize) -> PlacementDecision {
    let replicas: Vec<NodeId> = ranked.into_iter().take(replicas.max(1)).collect();
    PlacementDecision {
        primary: replicas[0],
        replicas,
    }
}

fn score(key: &[u8], node: NodeId) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    node.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendezvous_returns_requested_replica_count() {
        let layout = ClusterLayout {
            replicas: 2,
            nodes: vec![1, 2, 3],
        };

        let decision = RendezvousPlacement.locate(b"alpha", &layout);

        assert_eq!(decision.replicas.len(), 2);
        assert_eq!(decision.primary, decision.replicas[0]);
    }
}
