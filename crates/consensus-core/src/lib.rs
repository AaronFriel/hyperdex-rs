use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry<C> {
    pub index: u64,
    pub command: C,
}

#[allow(async_fn_in_trait)]
pub trait ReplicatedStateMachine<C>: Send + Sync
where
    C: Send + Sync + Clone + 'static,
{
    #[allow(async_fn_in_trait)]
    async fn apply(&self, command: C) -> Result<()>;
    #[allow(async_fn_in_trait)]
    async fn applied_len(&self) -> Result<u64>;
    fn name(&self) -> &'static str;
}

#[derive(Default)]
pub struct SingleNodeReplicator;

#[derive(Default)]
pub struct MirrorReplicator;

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
mod omnipaxos_backend;

#[cfg(feature = "omnipaxos")]
pub use omnipaxos_backend::OmniPaxosReplicator;

#[cfg(feature = "openraft")]
mod openraft_backend;

#[cfg(feature = "openraft")]
pub use openraft_backend::OpenRaftReplicator;
