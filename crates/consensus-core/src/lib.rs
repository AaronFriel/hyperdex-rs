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
