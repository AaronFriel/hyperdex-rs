use std::cmp::Ordering;
use std::collections::HashSet;

use data_model::NodeId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterLayout {
    pub replicas: usize,
    pub nodes: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementDecision {
    pub partition: usize,
    pub partitions: usize,
    pub primary: NodeId,
    pub replicas: Vec<NodeId>,
}

pub trait PlacementStrategy: Send + Sync {
    fn locate(&self, key: &[u8], layout: &ClusterLayout) -> PlacementDecision;
    fn name(&self) -> &'static str;
}

#[derive(Default)]
pub struct RendezvousPlacement;

#[derive(Clone, Debug)]
pub struct HyperSpacePlacement {
    tokens_per_node: usize,
}

impl Default for HyperSpacePlacement {
    fn default() -> Self {
        Self { tokens_per_node: 1 }
    }
}

impl HyperSpacePlacement {
    pub fn with_tokens_per_node(tokens_per_node: usize) -> Self {
        Self {
            tokens_per_node: tokens_per_node.max(1),
        }
    }
}

impl PlacementStrategy for RendezvousPlacement {
    fn locate(&self, key: &[u8], layout: &ClusterLayout) -> PlacementDecision {
        let mut ranked = layout.nodes.clone();
        ranked.sort_by(|a, b| {
            let sa = rendezvous_score(key, *a);
            let sb = rendezvous_score(key, *b);
            sb.cmp(&sa).then_with(|| a.cmp(b))
        });
        build_decision(0, ranked.len(), ranked, layout.replicas)
    }

    fn name(&self) -> &'static str {
        "rendezvous"
    }
}

impl PlacementStrategy for HyperSpacePlacement {
    fn locate(&self, key: &[u8], layout: &ClusterLayout) -> PlacementDecision {
        let ring = build_hyperspace_ring(&layout.nodes, self.tokens_per_node);
        let key_pos = hyperspace_key_pos(key);
        let start = hyperspace_ring_start(&ring, key_pos);

        let desired = layout.replicas.max(1).min(layout.nodes.len().max(1));
        let mut seen = HashSet::new();
        let mut replicas = Vec::with_capacity(desired);

        for offset in 0..ring.len() {
            let owner = ring[(start + offset) % ring.len()].owner;
            if seen.insert(owner) {
                replicas.push(owner);
                if replicas.len() == desired {
                    break;
                }
            }
        }

        if replicas.is_empty() {
            // All callers in this workspace currently treat an empty layout as a bug.
            // Keep behavior explicit instead of returning a dummy node id.
            panic!("hyperspace placement requires at least one node");
        }

        PlacementDecision {
            partition: start,
            partitions: ring.len(),
            primary: replicas[0],
            replicas,
        }
    }

    fn name(&self) -> &'static str {
        "hyperspace"
    }
}

fn build_decision(
    partition: usize,
    partitions: usize,
    ranked: Vec<NodeId>,
    replicas: usize,
) -> PlacementDecision {
    let desired = replicas.max(1).min(ranked.len().max(1));
    let replicas: Vec<NodeId> = ranked.into_iter().take(desired).collect();
    PlacementDecision {
        partition,
        partitions,
        primary: replicas[0],
        replicas,
    }
}

fn rendezvous_score(key: &[u8], node: NodeId) -> u64 {
    let mut hasher = Fnv1a64::new();
    hasher.update(b"rendezvous\0");
    hasher.update(key);
    hasher.update(&node.to_le_bytes());
    hasher.finish()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HyperToken {
    pos: u64,
    owner: NodeId,
    token_index: u32,
}

fn build_hyperspace_ring(nodes: &[NodeId], tokens_per_node: usize) -> Vec<HyperToken> {
    let tokens_per_node = tokens_per_node.max(1);
    let mut ring = Vec::with_capacity(nodes.len() * tokens_per_node);
    for &owner in nodes {
        for token_index in 0..tokens_per_node {
            let pos = hyperspace_token_pos(owner, token_index as u32);
            ring.push(HyperToken {
                pos,
                owner,
                token_index: token_index as u32,
            });
        }
    }

    ring.sort_by(|a, b| {
        a.pos
            .cmp(&b.pos)
            .then_with(|| a.owner.cmp(&b.owner))
            .then_with(|| a.token_index.cmp(&b.token_index))
    });
    ring
}

fn hyperspace_ring_start(ring: &[HyperToken], key_pos: u64) -> usize {
    match ring.binary_search_by(|t| {
        if t.pos < key_pos {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    }) {
        Ok(_) => unreachable!("binary_search_by never returns Ok for this comparator"),
        Err(idx) => {
            if idx == ring.len() {
                0
            } else {
                idx
            }
        }
    }
}

fn hyperspace_key_pos(key: &[u8]) -> u64 {
    let mut hasher = Fnv1a64::new();
    hasher.update(b"hyperspace-key\0");
    hasher.update(key);
    hasher.finish()
}

fn hyperspace_token_pos(owner: NodeId, token_index: u32) -> u64 {
    let mut hasher = Fnv1a64::new();
    hasher.update(b"hyperspace-token\0");
    hasher.update(&owner.to_le_bytes());
    hasher.update(&token_index.to_le_bytes());
    hasher.finish()
}

struct Fnv1a64 {
    state: u64,
}

impl Fnv1a64 {
    fn new() -> Self {
        Self {
            state: 0xcbf29ce484222325,
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.state ^= b as u64;
            self.state = self.state.wrapping_mul(0x100000001b3);
        }
    }

    fn finish(self) -> u64 {
        self.state
    }
}

#[cfg(test)]
mod tests;
