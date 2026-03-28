use std::collections::BTreeSet;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow};
use openraft::Raft;
use openraft::errors::{RPCError, ReplicationClosed, StreamingError, Unreachable};
use openraft::network::{
    Backoff, NetBackoff, NetSnapshot, NetStreamAppend, NetTransferLeader, NetVote, RPCOption,
    RaftNetworkFactory,
};
use openraft::raft::{
    AppendEntriesRequest, SnapshotResponse, StreamAppendResult, TransferLeaderRequest, VoteRequest,
    VoteResponse,
};
use openraft::type_config::alias::{SnapshotOf, VoteOf};
use openraft_rt::watch::WatchReceiver;

use crate::ReplicatedStateMachine;

type TypeConfig = openraft_memstore::TypeConfig;
type NodeId = openraft_memstore::MemNodeId;

#[derive(Clone, Default)]
struct NullNetworkFactory;

#[derive(Clone)]
struct NullNetwork {
    target: NodeId,
}

impl RaftNetworkFactory<TypeConfig> for NullNetworkFactory {
    type Network = NullNetwork;

    async fn new_client(&mut self, target: NodeId, _node: &()) -> Self::Network {
        NullNetwork { target }
    }
}

impl NetBackoff<TypeConfig> for NullNetwork {
    fn backoff(&self) -> Backoff {
        Backoff::new(std::iter::repeat(Duration::from_millis(25)))
    }
}

impl NetVote<TypeConfig> for NullNetwork {
    async fn vote(
        &mut self,
        _rpc: VoteRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<VoteResponse<TypeConfig>, RPCError<TypeConfig>> {
        Err(RPCError::Unreachable(Unreachable::from_string(format!(
            "NullNetwork does not support vote; target={}",
            self.target
        ))))
    }
}

impl NetStreamAppend<TypeConfig> for NullNetwork {
    fn stream_append<'s, S>(
        &'s mut self,
        _input: S,
        _option: RPCOption,
    ) -> openraft::base::BoxFuture<
        's,
        Result<
            openraft::base::BoxStream<
                's,
                Result<StreamAppendResult<TypeConfig>, RPCError<TypeConfig>>,
            >,
            RPCError<TypeConfig>,
        >,
    >
    where
        S: futures::Stream<Item = AppendEntriesRequest<TypeConfig>>
            + openraft::OptionalSend
            + Unpin
            + 'static,
    {
        let msg = format!(
            "NullNetwork does not support stream_append; target={}",
            self.target
        );
        Box::pin(async move { Err(RPCError::Unreachable(Unreachable::from_string(msg))) })
    }
}

impl NetSnapshot<TypeConfig> for NullNetwork {
    async fn full_snapshot(
        &mut self,
        _vote: VoteOf<TypeConfig>,
        _snapshot: SnapshotOf<TypeConfig>,
        _cancel: impl Future<Output = ReplicationClosed> + openraft::OptionalSend + 'static,
        _option: RPCOption,
    ) -> Result<SnapshotResponse<TypeConfig>, StreamingError<TypeConfig>> {
        Err(StreamingError::Unreachable(Unreachable::from_string(
            format!(
                "NullNetwork does not support full_snapshot; target={}",
                self.target
            ),
        )))
    }
}

impl NetTransferLeader<TypeConfig> for NullNetwork {
    async fn transfer_leader(
        &mut self,
        _req: TransferLeaderRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<(), RPCError<TypeConfig>> {
        Err(RPCError::Unreachable(Unreachable::from_string(format!(
            "NullNetwork does not support transfer_leader; target={}",
            self.target
        ))))
    }
}

pub struct OpenRaftReplicator {
    raft: Raft<TypeConfig, Arc<openraft_memstore::MemStateMachine>>,
}

impl OpenRaftReplicator {
    pub async fn new_single_node(id: NodeId) -> Result<Self> {
        let config = openraft::Config {
            cluster_name: "hyperdex-rs".to_owned(),
            ..Default::default()
        }
        .validate()
        .map_err(|e| anyhow!(e))?;

        let (log_store, state_machine) = openraft_memstore::new_mem_store();

        let raft = Raft::new(
            id,
            Arc::new(config),
            NullNetworkFactory,
            log_store,
            state_machine,
        )
        .await
        .map_err(|e| anyhow!(e))?;

        let mut members = BTreeSet::new();
        members.insert(id);
        raft.initialize(members).await.map_err(|e| anyhow!(e))?;

        let timeout = Duration::from_secs(2);
        raft.wait(Some(timeout))
            .current_leader(id, "wait for single-node leader")
            .await
            .map_err(|e| anyhow!(e))?;
        raft.wait(Some(timeout))
            .applied_index_at_least(Some(1), "wait for initialization apply")
            .await
            .map_err(|e| anyhow!(e))?;

        Ok(Self { raft })
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.raft.shutdown().await.map_err(|e| anyhow!(e))?;
        Ok(())
    }
}

impl ReplicatedStateMachine<openraft_memstore::ClientRequest> for OpenRaftReplicator {
    async fn apply(&self, command: openraft_memstore::ClientRequest) -> Result<()> {
        self.raft
            .client_write(command)
            .await
            .map(|_| ())
            .map_err(|e| anyhow!(e))
    }

    async fn applied_len(&self) -> Result<u64> {
        let metrics = self.raft.metrics().borrow_watched().clone();
        Ok(metrics.last_applied.map(|log_id| log_id.index).unwrap_or(0))
    }

    fn name(&self) -> &'static str {
        "openraft"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn openraft_replicator_smoke() -> Result<()> {
        let replicator = OpenRaftReplicator::new_single_node(1).await?;

        let req = openraft_memstore::ClientRequest {
            client: "client-1".to_owned(),
            serial: 1,
            status: "ok".to_owned(),
        };

        replicator.apply(req).await?;
        replicator
            .raft
            .wait(Some(Duration::from_secs(2)))
            .applied_index_at_least(Some(2), "wait for client write apply")
            .await
            .map_err(|e| anyhow!(e))?;

        let applied = replicator.applied_len().await?;
        assert!(applied >= 2, "expected applied >= 2, got {applied}");

        replicator.shutdown().await?;
        Ok(())
    }
}
