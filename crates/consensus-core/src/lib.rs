use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry<C> {
    pub index: u64,
    pub command: C,
}

#[async_trait]
pub trait ReplicatedStateMachine<C>: Send + Sync
where
    C: Send + Sync + Clone + 'static,
{
    async fn apply(&self, command: C) -> Result<()>;
    async fn applied_len(&self) -> Result<u64>;
    fn name(&self) -> &'static str;
}

#[derive(Default)]
pub struct SingleNodeReplicator;

#[derive(Default)]
pub struct MirrorReplicator;

#[async_trait]
impl<C> ReplicatedStateMachine<C> for SingleNodeReplicator
where
    C: Send + Sync + Clone + 'static,
{
    async fn apply(&self, _command: C) -> Result<()> {
        Ok(())
    }

    async fn applied_len(&self) -> Result<u64> {
        Ok(1)
    }

    fn name(&self) -> &'static str {
        "single-node"
    }
}

#[async_trait]
impl<C> ReplicatedStateMachine<C> for MirrorReplicator
where
    C: Send + Sync + Clone + 'static,
{
    async fn apply(&self, _command: C) -> Result<()> {
        Ok(())
    }

    async fn applied_len(&self) -> Result<u64> {
        Ok(2)
    }

    fn name(&self) -> &'static str {
        "mirror"
    }
}

#[cfg(feature = "omnipaxos")]
mod omnipaxos_backend {
    use super::ReplicatedStateMachine;
    use anyhow::{anyhow, Result};
    use async_trait::async_trait;
    use omnipaxos::{
        messages::Message,
        storage::{Entry, NoSnapshot},
        ClusterConfig, OmniPaxos, OmniPaxosConfig, ServerConfig,
    };
    use omnipaxos_storage::memory_storage::MemoryStorage;
    use parking_lot::Mutex;
    use std::{collections::BTreeMap, fmt::Debug};

    #[derive(Clone, Debug)]
    struct CommandEntry<C> {
        #[allow(dead_code)]
        command: C,
    }

    impl<C> Entry for CommandEntry<C>
    where
        C: Clone + Debug,
    {
        type Snapshot = NoSnapshot;
    }

    struct Node<C>
    where
        C: Clone + Debug,
    {
        paxos: OmniPaxos<CommandEntry<C>, MemoryStorage<CommandEntry<C>>>,
    }

    impl<C> Node<C>
    where
        C: Clone + Debug,
    {
        fn outgoing_messages(&mut self) -> Vec<Message<CommandEntry<C>>> {
            self.paxos.outgoing_messages()
        }

        fn handle_incoming(&mut self, msg: Message<CommandEntry<C>>) {
            self.paxos.handle_incoming(msg)
        }

        fn tick(&mut self) {
            self.paxos.tick()
        }

        fn decided_idx(&self) -> u64 {
            self.paxos.get_decided_idx()
        }
    }

    struct InProcessCluster<C>
    where
        C: Clone + Debug,
    {
        nodes: BTreeMap<u64, Node<C>>,
        leader_pid: u64,
    }

    impl<C> InProcessCluster<C>
    where
        C: Clone + Debug,
    {
        fn new_three_node(leader_pid: u64) -> Result<Self> {
            let pids = vec![1_u64, 2, 3];
            if !pids.contains(&leader_pid) {
                return Err(anyhow!("leader_pid must be one of {pids:?}"));
            }

            let cluster_config = ClusterConfig {
                configuration_id: 1,
                nodes: pids.clone(),
                flexible_quorum: None,
            };

            let mut nodes = BTreeMap::new();
            for pid in &pids {
                let mut server_config = ServerConfig::default();
                server_config.pid = *pid;
                server_config.election_tick_timeout = 1;
                server_config.resend_message_tick_timeout = 1;
                server_config.buffer_size = 1024;
                server_config.batch_size = 1;
                server_config.leader_priority = if *pid == leader_pid { 10 } else { 0 };

                let config = OmniPaxosConfig {
                    cluster_config: cluster_config.clone(),
                    server_config,
                };
                let storage = MemoryStorage::default();
                let paxos = config.build(storage).map_err(|e| anyhow!("{e}"))?;
                nodes.insert(*pid, Node { paxos });
            }

            let mut cluster = Self { nodes, leader_pid };
            cluster.pump(25);
            Ok(cluster)
        }

        fn pump(&mut self, rounds: usize) {
            for _ in 0..rounds {
                for node in self.nodes.values_mut() {
                    node.tick();
                }

                let mut outgoing = Vec::new();
                for node in self.nodes.values_mut() {
                    outgoing.extend(node.outgoing_messages());
                }

                for msg in outgoing {
                    let to = msg.get_receiver();
                    if let Some(dest) = self.nodes.get_mut(&to) {
                        dest.handle_incoming(msg);
                    }
                }
            }
        }

        fn append_and_wait_decide(&mut self, entry: CommandEntry<C>) -> Result<()> {
            let before = self
                .nodes
                .get(&self.leader_pid)
                .ok_or_else(|| anyhow!("missing leader node {}", self.leader_pid))?
                .decided_idx();
            let target = before + 1;

            for _ in 0..10 {
                let res = self
                    .nodes
                    .get_mut(&self.leader_pid)
                    .ok_or_else(|| anyhow!("missing leader node {}", self.leader_pid))?
                    .paxos
                    .append(entry.clone());

                if res.is_ok() {
                    break;
                }

                self.pump(10);
            }

            for _ in 0..100 {
                self.pump(1);
                let now = self
                    .nodes
                    .get(&self.leader_pid)
                    .ok_or_else(|| anyhow!("missing leader node {}", self.leader_pid))?
                    .decided_idx();
                if now >= target {
                    return Ok(());
                }
            }

            Err(anyhow!(
                "entry did not reach decided state after bounded pumping"
            ))
        }

        fn decided_len(&self) -> Result<u64> {
            Ok(self
                .nodes
                .get(&self.leader_pid)
                .ok_or_else(|| anyhow!("missing leader node {}", self.leader_pid))?
                .decided_idx())
        }
    }

    /// OmniPaxos-backed `ReplicatedStateMachine` implementation.
    ///
    /// This is currently an in-process 3-node cluster with deterministic leader preference.
    /// It is intentionally narrow scaffolding: it proves `consensus-core` can host a Paxos-family
    /// backend without introducing network or durable storage concerns.
    pub struct OmniPaxosReplicator<C>
    where
        C: Clone + Debug,
    {
        inner: Mutex<InProcessCluster<C>>,
    }

    impl<C> OmniPaxosReplicator<C>
    where
        C: Clone + Debug,
    {
        pub fn new_in_process() -> Result<Self> {
            Ok(Self {
                inner: Mutex::new(InProcessCluster::new_three_node(1)?),
            })
        }
    }

    #[async_trait]
    impl<C> ReplicatedStateMachine<C> for OmniPaxosReplicator<C>
    where
        C: Send + Sync + Clone + Debug + 'static,
    {
        async fn apply(&self, command: C) -> Result<()> {
            let mut cluster = self.inner.lock();
            cluster.append_and_wait_decide(CommandEntry { command })
        }

        async fn applied_len(&self) -> Result<u64> {
            let cluster = self.inner.lock();
            cluster.decided_len()
        }

        fn name(&self) -> &'static str {
            "omnipaxos"
        }
    }

    #[cfg(test)]
    mod tests {
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
    }
}

#[cfg(feature = "omnipaxos")]
pub use omnipaxos_backend::OmniPaxosReplicator;

#[cfg(feature = "openraft")]
mod openraft_backend;

#[cfg(feature = "openraft")]
pub use openraft_backend::OpenRaftReplicator;
