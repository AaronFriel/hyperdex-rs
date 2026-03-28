use super::*;
use futures::executor::block_on;

#[test]
fn omnipaxos_replicator_advances_decided_len() {
    let rsm = OmniPaxosReplicator::<u64>::new_in_process().unwrap();

    let before = block_on(rsm.applied_len()).unwrap();
    block_on(rsm.apply(42)).unwrap();
    let after = block_on(rsm.applied_len()).unwrap();

    assert!(
        after > before,
        "expected decided_len to advance: before={before} after={after}"
    );
}
