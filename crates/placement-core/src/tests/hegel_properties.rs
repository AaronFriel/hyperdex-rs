use std::collections::{BTreeSet, HashSet};

use ::hegel::{TestCase, generators};

use super::*;

fn normalized_nodes(raw_nodes: Vec<u8>) -> Vec<NodeId> {
    let mut nodes = BTreeSet::new();
    for raw in raw_nodes {
        nodes.insert(u64::from(raw) + 1);
    }

    if nodes.is_empty() {
        vec![1]
    } else {
        nodes.into_iter().collect()
    }
}

fn rotated_nodes(nodes: &[NodeId], rotation: usize) -> Vec<NodeId> {
    let mut rotated = nodes.to_vec();
    if !rotated.is_empty() {
        let len = rotated.len();
        rotated.rotate_left(rotation % len);
    }
    rotated
}

fn assert_decision_invariants(
    decision: &PlacementDecision,
    layout: &ClusterLayout,
    expected_partitions: usize,
) {
    let expected_replicas = layout.replicas.max(1).min(layout.nodes.len());

    assert_eq!(decision.primary, decision.replicas[0]);
    assert_eq!(decision.replicas.len(), expected_replicas);
    assert_eq!(decision.partitions, expected_partitions);
    assert!(decision.partition < decision.partitions);

    let node_set: HashSet<_> = layout.nodes.iter().copied().collect();
    let replica_set: HashSet<_> = decision.replicas.iter().copied().collect();
    assert_eq!(replica_set.len(), decision.replicas.len());
    assert!(
        decision
            .replicas
            .iter()
            .all(|replica| node_set.contains(replica))
    );
}

#[::hegel::test(test_cases = 25)]
fn hegel_placement_strategies_preserve_replica_invariants_and_input_order_independence(
    tc: TestCase,
) {
    let raw_nodes: Vec<u8> = tc.draw(
        generators::vecs(generators::integers::<u8>().max_value(15))
            .min_size(1)
            .max_size(6),
    );
    let requested_replicas: u8 = tc.draw(generators::integers::<u8>().max_value(6));
    let key_bytes: Vec<u8> = tc.draw(
        generators::vecs(generators::integers::<u8>())
            .min_size(1)
            .max_size(16),
    );
    let rotation: u8 = tc.draw(generators::integers::<u8>().max_value(5));
    let tokens_per_node: u8 = tc.draw(generators::integers::<u8>().max_value(3));

    let nodes = normalized_nodes(raw_nodes);
    let layout = ClusterLayout {
        replicas: usize::from(requested_replicas),
        nodes: nodes.clone(),
    };
    let rotated_layout = ClusterLayout {
        replicas: layout.replicas,
        nodes: rotated_nodes(&nodes, usize::from(rotation)),
    };

    let rendezvous = RendezvousPlacement;
    let rendezvous_a = rendezvous.locate(&key_bytes, &layout).unwrap();
    let rendezvous_b = rendezvous.locate(&key_bytes, &rotated_layout).unwrap();
    assert_decision_invariants(&rendezvous_a, &layout, layout.nodes.len());
    assert_decision_invariants(&rendezvous_b, &rotated_layout, rotated_layout.nodes.len());
    assert_eq!(rendezvous_a.primary, rendezvous_b.primary);
    assert_eq!(rendezvous_a.replicas, rendezvous_b.replicas);

    let hyperspace = HyperSpacePlacement::with_tokens_per_node(usize::from(tokens_per_node) + 1);
    let hyperspace_a = hyperspace.locate(&key_bytes, &layout).unwrap();
    let hyperspace_b = hyperspace.locate(&key_bytes, &rotated_layout).unwrap();
    let expected_partitions = layout.nodes.len() * (usize::from(tokens_per_node) + 1);
    assert_decision_invariants(&hyperspace_a, &layout, expected_partitions);
    assert_decision_invariants(&hyperspace_b, &rotated_layout, expected_partitions);
    assert_eq!(hyperspace_a.primary, hyperspace_b.primary);
    assert_eq!(hyperspace_a.replicas, hyperspace_b.replicas);
}
