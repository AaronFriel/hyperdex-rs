use super::*;

mod hegel;

#[test]
fn rendezvous_returns_requested_replica_count() {
    let layout = ClusterLayout {
        replicas: 2,
        nodes: vec![1, 2, 3],
    };

    let decision = RendezvousPlacement.locate(b"alpha", &layout).unwrap();

    assert_eq!(decision.replicas.len(), 2);
    assert_eq!(decision.primary, decision.replicas[0]);
}

#[test]
fn hyperspace_is_independent_of_node_input_order() {
    let placement = HyperSpacePlacement::default();
    let key = b"alpha";

    let a = ClusterLayout {
        replicas: 2,
        nodes: vec![10, 20, 30, 40],
    };
    let b = ClusterLayout {
        replicas: 2,
        nodes: vec![40, 10, 30, 20],
    };

    let da = placement.locate(key, &a).unwrap();
    let db = placement.locate(key, &b).unwrap();

    assert_eq!(da.primary, db.primary);
    assert_eq!(da.replicas, db.replicas);
    assert_eq!(da.partitions, db.partitions);
}

#[test]
fn hyperspace_partition_and_replicas_match_ring_successors() {
    let placement = HyperSpacePlacement::with_tokens_per_node(2);
    let layout = ClusterLayout {
        replicas: 3,
        nodes: vec![1, 2, 3],
    };
    let key = b"beta";

    let decision = placement.locate(key, &layout).unwrap();
    assert_eq!(decision.replicas.len(), 3);
    assert_eq!(decision.primary, decision.replicas[0]);

    let ring = build_hyperspace_ring(&layout.nodes, 2);
    let key_pos = hyperspace_key_pos(key);
    let start = hyperspace_ring_start(&ring, key_pos);

    assert_eq!(decision.partition, start);
    assert_eq!(decision.partitions, ring.len());

    let mut expected = Vec::new();
    let mut seen = HashSet::new();
    for offset in 0..ring.len() {
        let owner = ring[(start + offset) % ring.len()].owner;
        if seen.insert(owner) {
            expected.push(owner);
            if expected.len() == 3 {
                break;
            }
        }
    }

    assert_eq!(decision.replicas, expected);
}

#[test]
fn hyperspace_wraps_at_end_of_ring() {
    let layout = ClusterLayout {
        replicas: 1,
        nodes: vec![7, 8, 9],
    };

    let ring = build_hyperspace_ring(&layout.nodes, 1);
    let max_pos = ring.iter().map(|t| t.pos).max().unwrap();

    // Construct a key position that is larger than any token position to force wrap.
    let start = hyperspace_ring_start(&ring, max_pos.wrapping_add(1));
    assert_eq!(start, 0);

    let placement = HyperSpacePlacement::default();
    let mut wrap_key = None;
    for i in 0..2048u32 {
        let candidate = format!("wrap-{i:04}");
        if hyperspace_key_pos(candidate.as_bytes()) > max_pos {
            wrap_key = Some(candidate);
            break;
        }
    }
    let wrap_key = wrap_key.expect("expected to find a wrap key quickly");

    let decision = placement.locate(wrap_key.as_bytes(), &layout).unwrap();
    assert!(decision.partition < decision.partitions);
    assert_eq!(decision.partition, 0);
    assert_eq!(decision.replicas.len(), 1);
}

#[test]
fn hyperspace_rejects_empty_layout() {
    let layout = ClusterLayout {
        replicas: 1,
        nodes: Vec::new(),
    };

    let err = HyperSpacePlacement::default()
        .locate(b"alpha", &layout)
        .unwrap_err();

    assert_eq!(err, PlacementError::EmptyLayout);
}
