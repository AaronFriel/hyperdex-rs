use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{Result, anyhow};
use bytes::Bytes;
use cityhasher::hash as cityhash64;
use cluster_config::{
    ClusterConfig, ClusterNode, ConsensusBackend, PlacementBackend, StorageBackend,
    TransportBackend,
};
use control_plane::{Catalog, InMemoryCatalog};
use data_model::{
    Attribute, AttributeDefinition, Check, Mutation, NumericOp, Predicate, Record, Space, Value,
    ValueKind, parse_hyperdex_space,
};
use data_plane::DataPlane;
use engine_memory::MemoryEngine;
use engine_rocks::RocksEngine;
use hyperdex_admin_protocol::{
    AdminRequest, AdminResponse, BusyBeeFrame, ConfigView, CoordinatorAdminRequest,
    CoordinatorReturnCode, HyperdexAdminService, LegacyAdminRequest, LegacyAdminReturnCode,
    ReplicantAdminRequestMessage, ReplicantBootstrapConfiguration, ReplicantBootstrapResponse,
    ReplicantBootstrapServer, ReplicantCallCompletion, ReplicantConditionCompletion,
    ReplicantNetworkMsgtype, ReplicantReturnCode, ReplicantRobustParams,
    decode_packed_hyperdex_space,
};
use hyperdex_client_protocol::{ClientRequest, ClientResponse, HyperdexClientService};
use legacy_protocol::{
    AtomicRequest, CountRequest, FUNC_LIST_LPUSH, FUNC_LIST_RPUSH, FUNC_MAP_ADD, FUNC_MAP_REMOVE,
    FUNC_NUM_ADD, FUNC_NUM_AND, FUNC_NUM_DIV, FUNC_NUM_MAX, FUNC_NUM_MIN, FUNC_NUM_MOD,
    FUNC_NUM_MUL, FUNC_NUM_OR, FUNC_NUM_SUB, FUNC_NUM_XOR, FUNC_SET, FUNC_SET_ADD,
    FUNC_SET_INTERSECT, FUNC_SET_REMOVE, FUNC_SET_UNION, FUNC_STRING_APPEND, FUNC_STRING_LTRIM,
    FUNC_STRING_PREPEND, FUNC_STRING_RTRIM, GetAttribute, GetRequest, GetResponse, GetValue,
    HYPERDATATYPE_FLOAT, HYPERDATATYPE_INT64, HYPERDATATYPE_LIST_GENERIC,
    HYPERDATATYPE_MAP_GENERIC, HYPERDATATYPE_SET_GENERIC, HYPERDATATYPE_STRING,
    HYPERPREDICATE_EQUALS, HYPERPREDICATE_GREATER_EQUAL, HYPERPREDICATE_GREATER_THAN,
    HYPERPREDICATE_LESS_EQUAL, HYPERPREDICATE_LESS_THAN, LegacyCheck, LegacyFuncall,
    LegacyFuncallName, LegacyMessageType, LegacyPredicate, LegacyReturnCode,
    ProtocolAttributeCheck, ProtocolFuncall, ProtocolGetResponse, ProtocolKeyChange,
    ProtocolSearchItem, RequestHeader, ResponseHeader, SearchDoneResponse, SearchItemResponse,
    SearchStartRequest, config_mismatch_response, decode_protocol_atomic_request,
    decode_protocol_count_request, decode_protocol_get_request, decode_protocol_search_continue,
    decode_protocol_search_start, encode_protocol_atomic_response, encode_protocol_count_response,
    encode_protocol_get_response, encode_protocol_search_done, encode_protocol_search_item,
};
use placement_core::{
    HyperSpacePlacement, PlacementDecision, PlacementStrategy, RendezvousPlacement,
};
use storage_core::{StorageEngine, WriteResult};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use transport_core::{
    ClusterTransport, DATA_PLANE_METHOD, DataPlaneRequest, DataPlaneResponse, InProcessTransport,
    InternodeRequest, InternodeResponse, RemoteNode,
};

pub const COORDINATOR_CONTROL_HEADER_SIZE: usize = 2 + 4;
pub const COORDINATOR_CONTROL_BODY_LENGTH_SIZE: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LegacyBodyFormat {
    Protocol,
    Named,
}

#[derive(Clone, Debug)]
struct LegacySearchState {
    records: VecDeque<Record>,
    format: LegacyBodyFormat,
}

pub struct ClusterRuntime {
    local_node_id: u64,
    cluster_config: Mutex<ClusterConfig>,
    catalog: Arc<dyn Catalog>,
    storage: Arc<dyn StorageEngine>,
    data_plane: DataPlane,
    placement_strategy: Arc<dyn PlacementStrategy>,
    cluster_transport: Arc<dyn ClusterTransport>,
    consensus: ConsensusRuntime,
    placement: PlacementRuntime,
    storage_backend: StorageRuntime,
    internode_transport: TransportRuntime,
    coordinator_state: Mutex<CoordinatorState>,
    legacy_searches: Mutex<BTreeMap<u64, LegacySearchState>>,
    _ephemeral_storage_dir: Option<TempDir>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CoordinatorState {
    version: u64,
    stable_through: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsensusRuntime {
    SingleNode,
    Mirror,
    OmniPaxos,
    OpenRaft,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlacementRuntime {
    Hyperspace,
    Rendezvous,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageRuntime {
    Memory,
    RocksDb,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportRuntime {
    InProcess,
    Grpc,
}

pub struct CoordinatorControlService {
    listener: TcpListener,
}

type LegacySpaceAddDecoder = Arc<dyn Fn(&[u8]) -> Result<Space> + Send + Sync + 'static>;
type LegacyConfigEncoder = Arc<dyn Fn(&ConfigView) -> Result<Vec<u8>> + Send + Sync + 'static>;

pub struct CoordinatorAdminLegacyService {
    listener: TcpListener,
    space_add_decoder: LegacySpaceAddDecoder,
    config_encoder: LegacyConfigEncoder,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoordinatorControlResponse {
    pub status: [u8; 2],
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CoordinatorAdminSession {
    next_robust_nonce: u64,
    sender_id: u64,
    identified: bool,
    peer_local_id: u64,
    pending_config_waits: BTreeMap<u64, u64>,
    pending_waits: BTreeMap<u64, u64>,
    pending_completions: VecDeque<BusyBeeFrame>,
}

impl ClusterRuntime {
    pub fn new(
        local_node_id: u64,
        cluster_config: ClusterConfig,
        catalog: Arc<dyn Catalog>,
        storage: Arc<dyn StorageEngine>,
        placement: Arc<dyn PlacementStrategy>,
        cluster_transport: Arc<dyn ClusterTransport>,
        consensus: ConsensusRuntime,
        placement_runtime: PlacementRuntime,
        storage_backend: StorageRuntime,
        internode_transport: TransportRuntime,
        ephemeral_storage_dir: Option<TempDir>,
    ) -> Self {
        let data_plane = DataPlane::new(catalog.clone(), storage.clone(), placement.clone());
        Self {
            local_node_id,
            cluster_config: Mutex::new(cluster_config),
            catalog,
            storage,
            data_plane,
            placement_strategy: placement,
            cluster_transport,
            consensus,
            placement: placement_runtime,
            storage_backend,
            internode_transport,
            coordinator_state: Mutex::new(CoordinatorState {
                version: 0,
                stable_through: 0,
            }),
            legacy_searches: Mutex::new(BTreeMap::new()),
            _ephemeral_storage_dir: ephemeral_storage_dir,
        }
    }

    pub fn single_node(config: ClusterConfig) -> Result<Self> {
        Self::single_node_with_data_dir(config, None)
    }

    pub fn single_node_with_data_dir(
        config: ClusterConfig,
        data_dir: Option<&std::path::Path>,
    ) -> Result<Self> {
        let local_node_id = config.nodes.first().map(|node| node.id).unwrap_or(0);
        let catalog: Arc<dyn Catalog> =
            Arc::new(InMemoryCatalog::new(config.nodes.clone(), config.replicas));
        let consensus = select_consensus_backend(&config)?;
        let (placement, placement_runtime) = select_placement_backend(&config);
        let (storage, storage_backend, ephemeral_storage_dir) =
            select_storage_backend(&config, data_dir)?;
        let internode_transport = select_internode_transport(&config);

        Ok(Self::new(
            local_node_id,
            config,
            catalog,
            storage,
            placement,
            Arc::new(InProcessTransport),
            consensus,
            placement_runtime,
            storage_backend,
            internode_transport,
            ephemeral_storage_dir,
        ))
    }

    pub fn for_node(config: ClusterConfig, local_node_id: u64) -> Result<Self> {
        Self::for_node_with_data_dir(config, local_node_id, None)
    }

    pub fn for_node_with_data_dir(
        config: ClusterConfig,
        local_node_id: u64,
        data_dir: Option<&std::path::Path>,
    ) -> Result<Self> {
        if !config.nodes.iter().any(|node| node.id == local_node_id) {
            return Err(anyhow!(
                "cluster config does not define local node {local_node_id}"
            ));
        }
        let catalog: Arc<dyn Catalog> =
            Arc::new(InMemoryCatalog::new(config.nodes.clone(), config.replicas));
        let consensus = select_consensus_backend(&config)?;
        let (placement, placement_runtime) = select_placement_backend(&config);
        let (storage, storage_backend, ephemeral_storage_dir) =
            select_storage_backend(&config, data_dir)?;
        let internode_transport = select_internode_transport(&config);

        Ok(Self::new(
            local_node_id,
            config,
            catalog,
            storage,
            placement,
            Arc::new(InProcessTransport),
            consensus,
            placement_runtime,
            storage_backend,
            internode_transport,
            ephemeral_storage_dir,
        ))
    }

    pub fn install_cluster_transport(
        &mut self,
        transport: Arc<dyn ClusterTransport>,
        runtime: TransportRuntime,
    ) {
        self.cluster_transport = transport;
        self.internode_transport = runtime;
    }

    fn create_space(&self, space: Space) -> Result<()> {
        self.storage.create_space(space.name.clone())?;
        self.catalog.create_space(space)?;
        self.record_config_change()?;
        Ok(())
    }

    fn drop_space(&self, name: &str) -> Result<()> {
        if self.catalog.get_space(name)?.is_none() {
            return Err(anyhow!("space {name} not found"));
        }
        self.catalog.drop_space(name)?;
        self.storage.drop_space(name)?;
        self.record_config_change()?;
        Ok(())
    }

    pub fn consensus_backend_name(&self) -> &'static str {
        self.consensus.name()
    }

    pub fn placement_backend_name(&self) -> &'static str {
        self.placement.name()
    }

    pub fn storage_backend_name(&self) -> &'static str {
        self.storage_backend.name()
    }

    pub fn internode_transport_name(&self) -> &'static str {
        self.internode_transport.name()
    }

    pub fn local_node_id(&self) -> u64 {
        self.local_node_id
    }

    fn cluster_config_guard(&self) -> Result<MutexGuard<'_, ClusterConfig>> {
        lock_mutex(&self.cluster_config, "cluster config")
    }

    fn coordinator_state_guard(&self) -> Result<MutexGuard<'_, CoordinatorState>> {
        lock_mutex(&self.coordinator_state, "coordinator state")
    }

    fn legacy_searches_guard(&self) -> Result<MutexGuard<'_, BTreeMap<u64, LegacySearchState>>> {
        lock_mutex(&self.legacy_searches, "legacy search state")
    }

    fn cluster_node_ids(&self) -> Result<Vec<u64>> {
        Ok(self
            .cluster_config_guard()?
            .nodes
            .iter()
            .map(|node| node.id)
            .collect())
    }

    fn replica_factor(&self) -> Result<u64> {
        Ok(self.cluster_config_guard()?.replicas.max(1) as u64)
    }

    pub fn route_primary(&self, key: &[u8]) -> Result<u64> {
        let spaces = self.catalog.list_spaces()?;
        let Some(space_name) = spaces.first() else {
            anyhow::bail!("route_primary requires at least one space");
        };
        self.route_primary_for_space(space_name, key)
    }

    pub fn route_primary_for_space(&self, space_name: &str, key: &[u8]) -> Result<u64> {
        Ok(self.locate_key(space_name, key)?.primary)
    }

    pub fn route_primary_for_space_definition(&self, space: &Space, key: &[u8]) -> Result<u64> {
        let layout = self.catalog.layout()?;
        Ok(locate_key_in_space(&layout, &*self.placement_strategy, space, key)?.primary)
    }

    fn ensure_local_primary_for_key(&self, space: &str, key: &[u8]) -> Result<()> {
        let primary = self.route_primary_for_space(space, key)?;
        if primary != self.local_node_id {
            anyhow::bail!(
                "stale placement rejected primary-only request for `{space}` on node {}; primary is {}",
                self.local_node_id,
                primary
            );
        }
        Ok(())
    }

    fn locate_key(&self, space_name: &str, key: &[u8]) -> Result<PlacementDecision> {
        let layout = self.catalog.layout()?;
        let Some(space) = self.catalog.get_space(space_name)? else {
            anyhow::bail!("unknown space `{space_name}`");
        };
        locate_key_in_space(&layout, &*self.placement_strategy, &space, key)
    }

    async fn forward_data_request(
        &self,
        node: u64,
        request: DataPlaneRequest,
    ) -> Result<DataPlaneResponse> {
        let node = self.remote_node(node)?;
        let response = self
            .cluster_transport
            .send(
                &node,
                InternodeRequest::encode(DATA_PLANE_METHOD, &request)?,
            )
            .await?;
        if response.status != 200 {
            return Err(anyhow!(
                "internode request to node {} failed with status {}",
                node.id,
                response.status
            ));
        }
        response.decode()
    }

    async fn get_from_replica(
        &self,
        node_id: u64,
        space: &str,
        key: &bytes::Bytes,
    ) -> Result<Option<Record>> {
        if node_id == self.local_node_id {
            return self.data_plane.get(space, key);
        }

        match self
            .forward_data_request(
                node_id,
                DataPlaneRequest::Get {
                    space: space.to_owned(),
                    key: key.clone(),
                },
            )
            .await?
        {
            DataPlaneResponse::Record(record) => Ok(record),
            DataPlaneResponse::Unit
            | DataPlaneResponse::ConditionFailed
            | DataPlaneResponse::SearchResult(_)
            | DataPlaneResponse::Deleted(_) => {
                anyhow::bail!("unexpected response to get on replica {node_id}")
            }
        }
    }

    async fn execute_get_with_replica_fallback(
        &self,
        space: String,
        key: bytes::Bytes,
    ) -> Result<Option<Record>> {
        let decision = self.locate_key(&space, &key)?;
        let mut last_remote_error = None;

        for replica in decision.replicas {
            if replica == self.local_node_id {
                return self.data_plane.get(&space, &key);
            }

            match self.get_from_replica(replica, &space, &key).await {
                Ok(record) => return Ok(record),
                Err(err) => last_remote_error = Some((replica, err)),
            }
        }

        if let Some((replica, err)) = last_remote_error {
            return Err(anyhow!(
                "get failed on all replicas for space `{space}` after remote failure on replica {replica}: {err}"
            ));
        }

        Err(anyhow!(
            "get had no available replicas for space `{space}` and key lookup"
        ))
    }

    async fn replicate_put_to_secondaries(
        &self,
        space: &str,
        key: &bytes::Bytes,
        mutations: &[Mutation],
    ) -> Result<()> {
        let decision = self.locate_key(space, key)?;
        for replica in decision.replicas {
            if replica == self.local_node_id {
                continue;
            }
            match self
                .forward_data_request(
                    replica,
                    DataPlaneRequest::ReplicatedPut {
                        space: space.to_owned(),
                        key: key.clone(),
                        mutations: mutations.to_vec(),
                    },
                )
                .await?
            {
                DataPlaneResponse::Unit => {}
                DataPlaneResponse::ConditionFailed
                | DataPlaneResponse::Record(_)
                | DataPlaneResponse::SearchResult(_)
                | DataPlaneResponse::Deleted(_) => {
                    anyhow::bail!("unexpected response to replicated put on replica {replica}");
                }
            }
        }
        Ok(())
    }

    fn restore_local_record(
        &self,
        space: &str,
        key: &bytes::Bytes,
        previous: Option<Record>,
    ) -> Result<()> {
        match previous {
            Some(record) => {
                self.data_plane.delete(space, key)?;
                let mutations = Self::record_restore_mutations(record);
                match self.data_plane.put(space, key.clone(), &mutations)? {
                    WriteResult::Written | WriteResult::Missing => Ok(()),
                    WriteResult::ConditionFailed => {
                        anyhow::bail!("local rollback conditional failure for space `{space}`")
                    }
                }
            }
            None => {
                self.data_plane.delete(space, key)?;
                Ok(())
            }
        }
    }

    fn record_restore_mutations(record: Record) -> Vec<Mutation> {
        record
            .attributes
            .into_iter()
            .map(|(name, value)| Mutation::Set(Attribute { name, value }))
            .collect()
    }

    async fn restore_record_on_node(
        &self,
        node_id: u64,
        space: &str,
        record: Record,
    ) -> Result<()> {
        let key = record.key.clone();
        let mutations = Self::record_restore_mutations(record);
        if node_id == self.local_node_id {
            match self.data_plane.put(space, key, &mutations)? {
                WriteResult::Written | WriteResult::Missing => Ok(()),
                WriteResult::ConditionFailed => {
                    anyhow::bail!("local rollback conditional failure for space `{space}`")
                }
            }
        } else {
            match self
                .forward_data_request(
                    node_id,
                    DataPlaneRequest::ReplicatedPut {
                        space: space.to_owned(),
                        key,
                        mutations,
                    },
                )
                .await?
            {
                DataPlaneResponse::Unit => Ok(()),
                DataPlaneResponse::ConditionFailed => anyhow::bail!(
                    "replica rollback conditional failure on node {node_id} for space `{space}`"
                ),
                DataPlaneResponse::Record(_)
                | DataPlaneResponse::SearchResult(_)
                | DataPlaneResponse::Deleted(_) => {
                    anyhow::bail!(
                        "unexpected response to replicated rollback put on replica {node_id}"
                    )
                }
            }
        }
    }

    async fn rollback_delete_group_snapshots(
        &self,
        space: &str,
        snapshots: &[(u64, Vec<Record>)],
    ) -> Result<()> {
        for (node_id, records) in snapshots.iter().rev() {
            for record in records.iter().cloned() {
                self.restore_record_on_node(*node_id, space, record).await?;
            }
        }
        Ok(())
    }

    fn delete_group_snapshot_keys(records: &[Record]) -> BTreeSet<Bytes> {
        records.iter().map(|record| record.key.clone()).collect()
    }

    fn ensure_delete_group_snapshots_agree(
        &self,
        space: &str,
        snapshots: &[(u64, Vec<Record>)],
    ) -> Result<()> {
        let Some((expected_node_id, expected_records)) = snapshots.first() else {
            return Ok(());
        };

        let expected_keys = Self::delete_group_snapshot_keys(expected_records);
        for (node_id, records) in snapshots.iter().skip(1) {
            let observed_keys = Self::delete_group_snapshot_keys(records);
            if observed_keys != expected_keys {
                anyhow::bail!(
                    "distributed delete-group snapshot mismatch for space `{space}` between replicas {expected_node_id} and {node_id}"
                );
            }
        }

        Ok(())
    }

    async fn replicate_delete_to_secondaries(&self, space: &str, key: &bytes::Bytes) -> Result<()> {
        let decision = self.locate_key(space, key)?;
        for replica in decision.replicas {
            if replica == self.local_node_id {
                continue;
            }
            match self
                .forward_data_request(
                    replica,
                    DataPlaneRequest::ReplicatedDelete {
                        space: space.to_owned(),
                        key: key.clone(),
                    },
                )
                .await?
            {
                DataPlaneResponse::Unit => {}
                DataPlaneResponse::ConditionFailed
                | DataPlaneResponse::Record(_)
                | DataPlaneResponse::SearchResult(_)
                | DataPlaneResponse::Deleted(_) => {
                    anyhow::bail!("unexpected response to replicated delete on replica {replica}");
                }
            }
        }
        Ok(())
    }

    async fn confirm_local_primary_with_peers(
        &self,
        space: &str,
        key: &bytes::Bytes,
    ) -> Result<()> {
        let expected_cluster_size = self.cluster_node_ids()?.len() as u64;
        let mut confirmed_by_peer = false;
        let mut skipped_unavailable_peer = false;

        for node_id in self.cluster_node_ids()? {
            if node_id == self.local_node_id {
                continue;
            }

            match self
                .forward_data_request(
                    node_id,
                    DataPlaneRequest::ValidatePrimary {
                        space: space.to_owned(),
                        key: key.clone(),
                        expected_primary: self.local_node_id,
                        expected_cluster_size,
                    },
                )
                .await
            {
                Ok(DataPlaneResponse::Unit) => {
                    confirmed_by_peer = true;
                }
                Ok(DataPlaneResponse::ConditionFailed) => {
                    anyhow::bail!(
                        "peer {node_id} rejected local primary ownership for `{space}` on node {}",
                        self.local_node_id
                    );
                }
                Ok(
                    DataPlaneResponse::Record(_)
                    | DataPlaneResponse::SearchResult(_)
                    | DataPlaneResponse::Deleted(_),
                ) => {
                    anyhow::bail!("unexpected response to primary validation on peer {node_id}");
                }
                Err(err) if should_skip_schema_gap_replica(&err) => continue,
                Err(err) if should_skip_unavailable_read(&err) => {
                    skipped_unavailable_peer = true;
                    continue;
                }
                Err(err) => return Err(err),
            }
        }

        if skipped_unavailable_peer && !confirmed_by_peer {
            anyhow::bail!(
                "could not confirm local primary ownership for `{space}` on node {} with any reachable peer",
                self.local_node_id
            );
        }

        Ok(())
    }

    async fn apply_primary_put(
        &self,
        space: String,
        key: bytes::Bytes,
        mutations: Vec<Mutation>,
    ) -> Result<DataPlaneResponse> {
        self.confirm_local_primary_with_peers(&space, &key).await?;
        let mutations = self.materialize_key_mutation(&space, &key, mutations)?;
        let previous = self.data_plane.get(&space, &key)?;
        match self.data_plane.put(&space, key.clone(), &mutations)? {
            WriteResult::Written | WriteResult::Missing => {
                if let Err(err) = self
                    .replicate_put_to_secondaries(&space, &key, &mutations)
                    .await
                {
                    self.restore_local_record(&space, &key, previous)?;
                    return Err(err);
                }
                Ok(DataPlaneResponse::Unit)
            }
            WriteResult::ConditionFailed => Ok(DataPlaneResponse::ConditionFailed),
        }
    }

    async fn execute_distributed_delete_group(
        &self,
        space: String,
        checks: Vec<Check>,
    ) -> Result<u64> {
        let node_ids = self.cluster_node_ids()?;
        let mut snapshots = Vec::with_capacity(node_ids.len());
        for node_id in &node_ids {
            let records = if *node_id == self.local_node_id {
                match self.data_plane.search(&space, &checks) {
                    Ok(records) => records,
                    Err(err) if should_skip_delete_group_snapshot_replica(&err) => continue,
                    Err(err) => return Err(err),
                }
            } else {
                match self
                    .forward_data_request(
                        *node_id,
                        DataPlaneRequest::Search {
                            space: space.clone(),
                            checks: checks.clone(),
                        },
                    )
                    .await
                {
                    Ok(DataPlaneResponse::SearchResult(records)) => records,
                    Ok(
                        DataPlaneResponse::Unit
                        | DataPlaneResponse::ConditionFailed
                        | DataPlaneResponse::Record(_)
                        | DataPlaneResponse::Deleted(_),
                    ) => {
                        anyhow::bail!(
                            "unexpected response to delete-group snapshot search on replica {node_id}"
                        )
                    }
                    Err(err) if should_skip_delete_group_snapshot_replica(&err) => continue,
                    Err(err) => return Err(err),
                }
            };
            snapshots.push((*node_id, records));
        }

        if snapshots.is_empty() {
            anyhow::bail!("distributed delete-group had no reachable replicas for space `{space}`");
        }

        self.ensure_delete_group_snapshots_agree(&space, &snapshots)?;

        let mut deleted_total = 0u64;
        let mut applied_snapshot_count = 0usize;
        for (node_id, _) in &snapshots {
            let deleted = if *node_id == self.local_node_id {
                self.data_plane.delete_matching(&space, &checks)
            } else {
                self.forward_data_request(
                    *node_id,
                    DataPlaneRequest::ReplicatedDeleteGroup {
                        space: space.clone(),
                        checks: checks.clone(),
                    },
                )
                .await
                .and_then(|response| match response {
                    DataPlaneResponse::Deleted(count) => Ok(count),
                    DataPlaneResponse::Unit
                    | DataPlaneResponse::ConditionFailed
                    | DataPlaneResponse::Record(_)
                    | DataPlaneResponse::SearchResult(_) => {
                        anyhow::bail!(
                            "unexpected response to replicated delete-group on replica {node_id}"
                        )
                    }
                })
            };

            match deleted {
                Ok(count) => {
                    deleted_total += count;
                    applied_snapshot_count += 1;
                }
                Err(err) => {
                    let rollback = self
                        .rollback_delete_group_snapshots(
                            &space,
                            &snapshots[..applied_snapshot_count],
                        )
                        .await;
                    return match rollback {
                        Ok(()) => Err(err),
                        Err(rollback_err) => Err(anyhow!(
                            "distributed delete-group failed: {err}; rollback failed: {rollback_err}"
                        )),
                    };
                }
            }
        }

        let replica_factor = self.replica_factor()?;
        if deleted_total % replica_factor != 0 {
            let err = anyhow!(
                "distributed delete-group removed {deleted_total} physical records across replica factor {replica_factor}"
            );
            let rollback = self
                .rollback_delete_group_snapshots(&space, &snapshots[..applied_snapshot_count])
                .await;
            return match rollback {
                Ok(()) => Err(err),
                Err(rollback_err) => Err(anyhow!(
                    "distributed delete-group failed: {err}; rollback failed: {rollback_err}"
                )),
            };
        }

        Ok(deleted_total / replica_factor)
    }

    async fn execute_distributed_search(
        &self,
        space: String,
        checks: Vec<Check>,
    ) -> Result<Vec<Record>> {
        let mut records_by_key = BTreeMap::new();
        let mut successful_replicas = 0usize;
        let mut skipped_replicas = Vec::new();

        for node_id in self.cluster_node_ids()? {
            let records = if node_id == self.local_node_id {
                match self.data_plane.search(&space, &checks) {
                    Ok(records) => {
                        successful_replicas += 1;
                        records
                    }
                    Err(err) if should_skip_distributed_read_replica(&err) => {
                        skipped_replicas.push(node_id);
                        continue;
                    }
                    Err(err) => return Err(err),
                }
            } else {
                match self
                    .forward_data_request(
                        node_id,
                        DataPlaneRequest::Search {
                            space: space.clone(),
                            checks: checks.clone(),
                        },
                    )
                    .await
                {
                    Ok(DataPlaneResponse::SearchResult(records)) => {
                        successful_replicas += 1;
                        records
                    }
                    Ok(
                        DataPlaneResponse::Unit
                        | DataPlaneResponse::ConditionFailed
                        | DataPlaneResponse::Record(_)
                        | DataPlaneResponse::Deleted(_),
                    ) => {
                        anyhow::bail!(
                            "unexpected response to distributed search on replica {node_id}"
                        )
                    }
                    Err(err) if should_skip_distributed_read_replica(&err) => {
                        skipped_replicas.push(node_id);
                        continue;
                    }
                    Err(err) => return Err(err),
                }
            };

            for record in records {
                records_by_key.entry(record.key.clone()).or_insert(record);
            }
        }

        if successful_replicas == 0 {
            anyhow::bail!(
                "distributed search had no reachable replicas for space `{space}`; skipped replicas: {:?}",
                skipped_replicas
            );
        }

        Ok(records_by_key.into_values().collect())
    }

    async fn execute_distributed_count(&self, space: String, checks: Vec<Check>) -> Result<u64> {
        Ok(self.execute_distributed_search(space, checks).await?.len() as u64)
    }

    async fn apply_primary_conditional_put(
        &self,
        space: String,
        key: bytes::Bytes,
        checks: Vec<Check>,
        mutations: Vec<Mutation>,
    ) -> Result<DataPlaneResponse> {
        self.confirm_local_primary_with_peers(&space, &key).await?;
        let mutations = self.materialize_key_mutation(&space, &key, mutations)?;
        let previous = self.data_plane.get(&space, &key)?;
        match self
            .data_plane
            .conditional_put(&space, key.clone(), &checks, &mutations)?
        {
            WriteResult::Written | WriteResult::Missing => {
                if let Err(err) = self
                    .replicate_put_to_secondaries(&space, &key, &mutations)
                    .await
                {
                    self.restore_local_record(&space, &key, previous)?;
                    return Err(err);
                }
                Ok(DataPlaneResponse::Unit)
            }
            WriteResult::ConditionFailed => Ok(DataPlaneResponse::ConditionFailed),
        }
    }

    async fn apply_primary_delete(
        &self,
        space: String,
        key: bytes::Bytes,
    ) -> Result<DataPlaneResponse> {
        self.confirm_local_primary_with_peers(&space, &key).await?;
        let previous = self.data_plane.get(&space, &key)?;
        match self.data_plane.delete(&space, &key)? {
            WriteResult::Written | WriteResult::Missing => {
                if let Err(err) = self.replicate_delete_to_secondaries(&space, &key).await {
                    self.restore_local_record(&space, &key, previous)?;
                    return Err(err);
                }
                Ok(DataPlaneResponse::Unit)
            }
            WriteResult::ConditionFailed => Ok(DataPlaneResponse::ConditionFailed),
        }
    }

    fn materialize_key_mutation(
        &self,
        space_name: &str,
        key: &bytes::Bytes,
        mut mutations: Vec<Mutation>,
    ) -> Result<Vec<Mutation>> {
        let space = self
            .catalog
            .get_space(space_name)?
            .ok_or_else(|| anyhow!("unknown space `{space_name}`"))?;
        let key_attribute = space.key_attribute.clone();
        let already_present = mutations.iter().any(|mutation| match mutation {
            Mutation::Set(attribute) => attribute.name == key_attribute,
            _ => false,
        });

        if !already_present {
            mutations.insert(
                0,
                Mutation::Set(Attribute {
                    name: key_attribute,
                    value: Value::Bytes(key.clone()),
                }),
            );
        }

        Ok(mutations)
    }

    pub async fn handle_internode_request(
        &self,
        request: InternodeRequest,
    ) -> Result<InternodeResponse> {
        match request.method.as_str() {
            DATA_PLANE_METHOD => {
                let response = match request.decode()? {
                    DataPlaneRequest::Put {
                        space,
                        key,
                        mutations,
                    } => {
                        self.ensure_local_primary_for_key(&space, &key)?;
                        self.apply_primary_put(space, key, mutations).await?
                    }
                    DataPlaneRequest::Get { space, key } => {
                        DataPlaneResponse::Record(self.data_plane.get(&space, &key)?)
                    }
                    DataPlaneRequest::Search { space, checks } => {
                        DataPlaneResponse::SearchResult(self.data_plane.search(&space, &checks)?)
                    }
                    DataPlaneRequest::Delete { space, key } => {
                        self.ensure_local_primary_for_key(&space, &key)?;
                        self.apply_primary_delete(space, key).await?
                    }
                    DataPlaneRequest::ConditionalPut {
                        space,
                        key,
                        checks,
                        mutations,
                    } => {
                        self.ensure_local_primary_for_key(&space, &key)?;
                        self.apply_primary_conditional_put(space, key, checks, mutations)
                            .await?
                    }
                    DataPlaneRequest::ValidatePrimary {
                        space,
                        key,
                        expected_primary,
                        expected_cluster_size,
                    } => {
                        let local_cluster_size = self.cluster_node_ids()?.len() as u64;
                        if local_cluster_size < expected_cluster_size
                            || self.route_primary_for_space(&space, &key)? == expected_primary
                        {
                            DataPlaneResponse::Unit
                        } else {
                            DataPlaneResponse::ConditionFailed
                        }
                    }
                    DataPlaneRequest::ReplicatedPut {
                        space,
                        key,
                        mutations,
                    } => match self.data_plane.put(&space, key, &mutations)? {
                        WriteResult::Written | WriteResult::Missing => DataPlaneResponse::Unit,
                        WriteResult::ConditionFailed => DataPlaneResponse::ConditionFailed,
                    },
                    DataPlaneRequest::ReplicatedDelete { space, key } => {
                        match self.data_plane.delete(&space, &key)? {
                            WriteResult::Written | WriteResult::Missing => DataPlaneResponse::Unit,
                            WriteResult::ConditionFailed => DataPlaneResponse::ConditionFailed,
                        }
                    }
                    DataPlaneRequest::ReplicatedDeleteGroup { space, checks } => {
                        DataPlaneResponse::Deleted(
                            self.data_plane.delete_matching(&space, &checks)?,
                        )
                    }
                };
                InternodeResponse::encode(200, &response)
            }
            other => Err(anyhow!("unsupported internode method `{other}`")),
        }
    }

    fn only_space_name(&self) -> Result<String> {
        let spaces = self.catalog.list_spaces()?;
        match spaces.as_slice() {
            [space] => Ok(space.clone()),
            [] => Err(anyhow!(
                "legacy request handling requires one created space"
            )),
            _ => Err(anyhow!(
                "legacy request handling is ambiguous with multiple spaces"
            )),
        }
    }

    fn config_view(&self) -> Result<hyperdex_admin_protocol::ConfigView> {
        let coordinator_state = *self.coordinator_state_guard()?;
        let cluster = self.cluster_config_guard()?.clone();
        let mut spaces = Vec::new();
        for name in self.catalog.list_spaces()? {
            let Some(space) = self.catalog.get_space(&name)? else {
                return Err(anyhow!(
                    "catalog listed space `{name}` but could not fetch its definition"
                ));
            };
            spaces.push(space);
        }

        Ok(hyperdex_admin_protocol::ConfigView {
            version: coordinator_state.version,
            stable_through: coordinator_state.stable_through,
            cluster,
            spaces,
        })
    }

    fn register_daemon(&self, node: ClusterNode) -> Result<()> {
        let catalog_changed = self.catalog.register_daemon(node.clone())?;
        let config_changed = {
            let mut cluster_config = self.cluster_config_guard()?;
            upsert_cluster_node(&mut cluster_config.nodes, node)
        };

        if catalog_changed || config_changed {
            self.record_config_change()?;
        }

        Ok(())
    }

    fn stable_version(&self) -> Result<u64> {
        Ok(self.coordinator_state_guard()?.stable_through)
    }

    fn record_config_change(&self) -> Result<()> {
        let mut coordinator_state = self.coordinator_state_guard()?;
        coordinator_state.version += 1;
        coordinator_state.stable_through = coordinator_state.version;
        Ok(())
    }

    fn apply_config_view(&self, view: &ConfigView) -> Result<()> {
        self.catalog.replace_daemons(view.cluster.nodes.clone())?;

        *self.cluster_config_guard()? = view.cluster.clone();

        let local_spaces = self.catalog.list_spaces()?;
        let remote_spaces = view
            .spaces
            .iter()
            .map(|space| (space.name.clone(), space))
            .collect::<BTreeMap<_, _>>();

        for name in &local_spaces {
            if !remote_spaces.contains_key(name) {
                self.drop_space(name)?;
            }
        }

        for space in &view.spaces {
            match self.catalog.get_space(&space.name)? {
                Some(existing) if existing == *space => {}
                Some(_) => {
                    self.drop_space(&space.name)?;
                    self.create_space(space.clone())?;
                }
                None => {
                    self.create_space(space.clone())?;
                }
            }
        }

        *self.coordinator_state_guard()? = CoordinatorState {
            version: view.version,
            stable_through: view.stable_through,
        };

        Ok(())
    }

    fn remote_node(&self, node_id: u64) -> Result<RemoteNode> {
        let cluster_config = self.cluster_config_guard()?;
        let Some(node) = cluster_config.nodes.iter().find(|node| node.id == node_id) else {
            return Err(anyhow!(
                "cluster config does not define remote node {node_id}"
            ));
        };
        Ok(RemoteNode {
            id: node.id,
            host: node.host.clone(),
            port: match self.internode_transport {
                TransportRuntime::InProcess => node.data_port,
                TransportRuntime::Grpc => node.control_port,
            },
        })
    }
}

fn locate_key_in_space(
    layout: &placement_core::ClusterLayout,
    placement_strategy: &dyn PlacementStrategy,
    space: &Space,
    key: &[u8],
) -> Result<PlacementDecision> {
    if matches!(
        legacy_key_kind(space)?,
        ValueKind::Bytes | ValueKind::String | ValueKind::Document
    ) {
        return legacy_locate_string_key(layout, space, key);
    }
    Ok(placement_strategy.locate(key, layout)?)
}

fn legacy_key_kind(space: &Space) -> Result<&ValueKind> {
    let _ = space;
    Ok(&ValueKind::String)
}

fn legacy_locate_string_key(
    layout: &placement_core::ClusterLayout,
    space: &Space,
    key: &[u8],
) -> Result<PlacementDecision> {
    if layout.nodes.is_empty() {
        anyhow::bail!("space `{}` has no registered nodes", space.name);
    }

    let replica_count = layout
        .nodes
        .len()
        .min(space.options.fault_tolerance.saturating_add(1) as usize)
        .max(1);
    let replica_sets = legacy_replica_sets(&layout.nodes, replica_count, space.options.partitions);
    if replica_sets.is_empty() {
        anyhow::bail!("space `{}` has no legacy replica sets", space.name);
    }

    let partition = legacy_region_for_hash(cityhash64::<u64>(key), space.options.partitions);
    let replica_set_idx = (partition * replica_sets.len()) / space.options.partitions as usize;
    let replicas = replica_sets[replica_set_idx].clone();

    Ok(PlacementDecision {
        partition,
        partitions: space.options.partitions as usize,
        primary: replicas[0],
        replicas,
    })
}

fn legacy_region_for_hash(hash: u64, partitions: u32) -> usize {
    let partitions = partitions.max(1) as usize;
    ((u128::from(hash) * partitions as u128) / (u128::from(u64::MAX) + 1)) as usize
}

impl CoordinatorAdminSession {
    fn new() -> Self {
        Self {
            next_robust_nonce: 1,
            sender_id: LEGACY_COORDINATOR_SERVER_ID,
            identified: false,
            peer_local_id: 0,
            pending_config_waits: BTreeMap::new(),
            pending_waits: BTreeMap::new(),
            pending_completions: VecDeque::new(),
        }
    }

    fn observe_identify(&mut self, peer_local_id: u64, peer_remote_id: u64) -> Result<bool> {
        if !self.identified {
            if peer_remote_id != 0 {
                self.sender_id = peer_remote_id;
            }
            self.peer_local_id = peer_local_id;
            self.identified = true;
            return Ok(true);
        }

        if peer_remote_id != 0 && peer_remote_id != self.sender_id {
            anyhow::bail!(
                "legacy admin identify remote id {} does not match established sender id {}",
                peer_remote_id,
                self.sender_id
            );
        }
        if peer_local_id != 0 && self.peer_local_id != 0 && peer_local_id != self.peer_local_id {
            anyhow::bail!(
                "legacy admin identify local id {} does not match established peer id {}",
                peer_local_id,
                self.peer_local_id
            );
        }

        Ok(false)
    }

    fn queue_identify_response(&mut self, peer_local_id: u64) {
        let mut payload = Vec::with_capacity(16);
        encode_u64_be(&mut payload, self.sender_id);
        encode_u64_be(&mut payload, peer_local_id);
        self.pending_completions
            .push_back(BusyBeeFrame::identify(payload));
    }

    fn allocate_robust_command_nonce(&mut self) -> u64 {
        let nonce = self.next_robust_nonce;
        self.next_robust_nonce = self.next_robust_nonce.saturating_add(1);
        nonce
    }

    fn queue_call_completion(
        &mut self,
        nonce: u64,
        status: ReplicantReturnCode,
        output: Vec<u8>,
    ) -> Result<()> {
        self.pending_completions.push_back(BusyBeeFrame::new(
            ReplicantCallCompletion {
                nonce,
                status,
                output,
            }
            .encode(),
        ));
        Ok(())
    }

    fn queue_robust_params(&mut self, nonce: u64, command_nonce: u64) {
        self.pending_completions.push_back(BusyBeeFrame::new(
            ReplicantRobustParams {
                nonce,
                command_nonce,
                min_slot: 0,
            }
            .encode(),
        ));
    }

    fn queue_condition_completion(
        &mut self,
        nonce: u64,
        status: ReplicantReturnCode,
        state: u64,
        data: Vec<u8>,
    ) {
        self.pending_completions.push_back(BusyBeeFrame::new(
            ReplicantConditionCompletion {
                nonce,
                status,
                state,
                data,
            }
            .encode(),
        ));
    }

    fn queue_bootstrap_response(&mut self, bootstrap_address: SocketAddr) {
        self.pending_completions.push_back(BusyBeeFrame::new(
            ReplicantBootstrapResponse {
                server: ReplicantBootstrapServer {
                    id: self.sender_id,
                    address: bootstrap_address,
                },
                configuration: legacy_bootstrap_configuration(self.sender_id, bootstrap_address),
            }
            .encode(),
        ));
    }

    fn queue_config_update(
        &mut self,
        runtime: &ClusterRuntime,
        config_encoder: &(dyn Fn(&ConfigView) -> Result<Vec<u8>> + Send + Sync),
        nonce: u64,
    ) -> Result<()> {
        let view = runtime.config_view()?;
        let encoded = config_encoder(&view)?;
        self.queue_condition_completion(
            nonce,
            ReplicantReturnCode::Success,
            legacy_condition_state(view.version),
            encoded,
        );
        Ok(())
    }

    fn queue_ready_config_waits(
        &mut self,
        runtime: &ClusterRuntime,
        config_encoder: &(dyn Fn(&ConfigView) -> Result<Vec<u8>> + Send + Sync),
    ) -> Result<()> {
        let config_state = legacy_condition_state(runtime.config_view()?.version);
        let ready = self
            .pending_config_waits
            .iter()
            .filter_map(|(&nonce, &target_state)| (target_state <= config_state).then_some(nonce))
            .collect::<Vec<_>>();

        for nonce in ready {
            self.pending_config_waits.remove(&nonce);
            self.queue_config_update(runtime, config_encoder, nonce)?;
        }

        Ok(())
    }

    fn queue_ready_waits(&mut self, runtime: &ClusterRuntime) -> Result<()> {
        let stable_version = legacy_condition_state(runtime.stable_version()?);
        let ready = self
            .pending_waits
            .iter()
            .filter_map(|(&nonce, &target_state)| {
                (target_state <= stable_version).then_some((nonce, target_state))
            })
            .collect::<Vec<_>>();

        for (nonce, _) in ready {
            self.pending_waits.remove(&nonce);
            self.queue_condition_completion(
                nonce,
                ReplicantReturnCode::Success,
                stable_version,
                Vec::new(),
            );
        }

        Ok(())
    }

    fn take_pending_frames(&mut self) -> Vec<BusyBeeFrame> {
        self.pending_completions.drain(..).collect()
    }
}

impl CoordinatorAdminLegacyService {
    pub async fn bind(address: SocketAddr) -> Result<Self> {
        Self::bind_with_codecs(
            address,
            Arc::new(default_legacy_space_add_decoder),
            Arc::new(default_legacy_config_encoder),
        )
        .await
    }

    pub async fn bind_with_codecs(
        address: SocketAddr,
        space_add_decoder: LegacySpaceAddDecoder,
        config_encoder: LegacyConfigEncoder,
    ) -> Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(address).await?,
            space_add_decoder,
            config_encoder,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    pub async fn serve_once(&self, runtime: &ClusterRuntime) -> Result<()> {
        let (mut stream, _) = self.listener.accept().await?;
        serve_coordinator_admin_connection(
            &mut stream,
            runtime,
            self.space_add_decoder.clone(),
            self.config_encoder.clone(),
        )
        .await
    }
}

fn default_legacy_space_add_decoder(bytes: &[u8]) -> Result<Space> {
    decode_packed_hyperdex_space(bytes)
}

fn default_legacy_config_encoder(view: &ConfigView) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut ids = LegacyConfigIds::default();
    let mut nodes = view.cluster.nodes.clone();
    let mut spaces = view.spaces.clone();

    nodes.sort_by_key(|node| node.id);
    spaces.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));

    encode_u64_be(&mut out, 0);
    encode_u64_be(&mut out, legacy_condition_state(view.version));
    encode_u64_be(&mut out, 0);
    encode_u64_be(&mut out, nodes.len() as u64);
    encode_u64_be(&mut out, spaces.len() as u64);
    encode_u64_be(&mut out, 0);

    for node in &nodes {
        encode_legacy_server(&mut out, node)?;
    }

    let server_ids = nodes.iter().map(|node| node.id).collect::<Vec<_>>();

    for space in &spaces {
        encode_legacy_space(&mut out, space, &server_ids, &mut ids)?;
    }

    Ok(out)
}

const LEGACY_COORDINATOR_CLUSTER_ID: u64 = 1;
const LEGACY_COORDINATOR_FIRST_SLOT: u64 = 1;
// BusyBee deanonymizes the first anonymous peer connection to sender token 2.
// The original Replicant client accepts a bootstrap reply only when the reply
// body server id matches the BusyBee sender id it observed on that channel.
const LEGACY_COORDINATOR_SERVER_ID: u64 = 2;
const LEGACY_REPLICANT_TICK_STATE: u64 = 1;

fn legacy_condition_state(version: u64) -> u64 {
    version.saturating_add(1)
}

fn legacy_bootstrap_configuration(
    sender_id: u64,
    bootstrap_address: SocketAddr,
) -> ReplicantBootstrapConfiguration {
    let bootstrap_address = effective_legacy_bootstrap_address(bootstrap_address);
    ReplicantBootstrapConfiguration {
        cluster_id: LEGACY_COORDINATOR_CLUSTER_ID,
        version: legacy_condition_state(0),
        first_slot: LEGACY_COORDINATOR_FIRST_SLOT,
        servers: vec![ReplicantBootstrapServer {
            id: sender_id,
            address: bootstrap_address,
        }],
    }
}

fn effective_legacy_bootstrap_address(default_address: SocketAddr) -> SocketAddr {
    std::env::var("HYPERDEX_RS_LEGACY_BOOTSTRAP_ADDR")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default_address)
}

const LEGACY_SERVER_STATE_AVAILABLE: u8 = 3;
const LEGACY_LOCATION_IPV4: u8 = 4;
const LEGACY_LOCATION_IPV6: u8 = 6;
const LEGACY_HYPERDATATYPE_BYTES: u16 = 9216;
const LEGACY_HYPERDATATYPE_STRING: u16 = 9217;
const LEGACY_HYPERDATATYPE_INT64: u16 = 9218;
const LEGACY_HYPERDATATYPE_FLOAT: u16 = 9219;
const LEGACY_HYPERDATATYPE_DOCUMENT: u16 = 9223;
const LEGACY_HYPERDATATYPE_LIST_GENERIC: u16 = 9280;
const LEGACY_HYPERDATATYPE_SET_GENERIC: u16 = 9344;
const LEGACY_HYPERDATATYPE_MAP_GENERIC: u16 = 9408;
const LEGACY_HYPERDATATYPE_TIMESTAMP_GENERIC: u16 = 9472;

struct LegacyConfigIds {
    next_id: u64,
}

impl Default for LegacyConfigIds {
    fn default() -> Self {
        Self { next_id: 1 }
    }
}

impl LegacyConfigIds {
    fn allocate(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

fn encode_legacy_server(out: &mut Vec<u8>, node: &ClusterNode) -> Result<()> {
    out.push(LEGACY_SERVER_STATE_AVAILABLE);
    encode_u64_be(out, node.id);
    encode_legacy_location(out, &node.host, node.data_port)?;
    Ok(())
}

fn encode_legacy_location(out: &mut Vec<u8>, host: &str, port: u16) -> Result<()> {
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => {
            out.push(LEGACY_LOCATION_IPV4);
            out.extend_from_slice(&address.octets());
        }
        Ok(IpAddr::V6(address)) => {
            out.push(LEGACY_LOCATION_IPV6);
            out.extend_from_slice(&address.octets());
        }
        Err(_) => {
            return Err(anyhow!(
                "legacy coordinator config requires an IP address for node host `{host}`"
            ));
        }
    }

    out.extend_from_slice(&port.to_be_bytes());
    Ok(())
}

fn encode_legacy_space(
    out: &mut Vec<u8>,
    space: &Space,
    server_ids: &[u64],
    ids: &mut LegacyConfigIds,
) -> Result<()> {
    let mut attr_positions = BTreeMap::new();
    let mut attrs = Vec::with_capacity(space.attributes.len() + 1);

    attrs.push((space.key_attribute.clone(), LEGACY_HYPERDATATYPE_STRING));
    attr_positions.insert(space.key_attribute.clone(), 0_u16);

    for attribute in &space.attributes {
        let datatype = legacy_hyperdatatype(&attribute.kind)?;
        let attr_index = u16::try_from(attrs.len()).map_err(|_| {
            anyhow!(
                "space `{}` exceeds legacy attribute index width",
                space.name
            )
        })?;
        attr_positions.insert(attribute.name.clone(), attr_index);
        attrs.push((attribute.name.clone(), datatype));
    }

    encode_u64_be(out, ids.allocate());
    encode_legacy_slice(out, space.name.as_bytes())?;
    encode_u64_be(out, u64::from(space.options.fault_tolerance));
    encode_u16_be(
        out,
        u16::try_from(attrs.len())
            .map_err(|_| anyhow!("space `{}` exceeds legacy attribute count", space.name))?,
    );
    encode_u16_be(
        out,
        u16::try_from(space.subspaces.len() + 1)
            .map_err(|_| anyhow!("space `{}` exceeds legacy subspace count", space.name))?,
    );
    encode_u16_be(out, 0);

    for (name, datatype) in &attrs {
        encode_legacy_slice(out, name.as_bytes())?;
        encode_u16_be(out, *datatype);
    }

    let partitions = space.options.partitions;
    let replica_count = if server_ids.is_empty() {
        0
    } else {
        server_ids
            .len()
            .min(space.options.fault_tolerance.saturating_add(1) as usize)
            .max(1)
    };
    let replica_sets = legacy_replica_sets(server_ids, replica_count, space.options.partitions);

    encode_legacy_subspace(out, &[0], partitions, &replica_sets, ids)?;

    for subspace in &space.subspaces {
        let mut attr_indexes = Vec::with_capacity(subspace.dimensions.len());
        for dimension in &subspace.dimensions {
            let Some(attr_index) = attr_positions.get(dimension).copied() else {
                return Err(anyhow!(
                    "space `{}` subspace references unknown attribute `{dimension}`",
                    space.name
                ));
            };
            attr_indexes.push(attr_index);
        }
        encode_legacy_subspace(out, &attr_indexes, partitions, &replica_sets, ids)?;
    }

    Ok(())
}

fn encode_legacy_subspace(
    out: &mut Vec<u8>,
    attr_indexes: &[u16],
    partitions: u32,
    replica_sets: &[Vec<u64>],
    ids: &mut LegacyConfigIds,
) -> Result<()> {
    let regions = legacy_partition_regions(attr_indexes.len(), partitions);
    let subspace_id = ids.allocate();
    let region_ids = regions.iter().map(|_| ids.allocate()).collect::<Vec<_>>();
    let region_count = regions.len();

    encode_u64_be(out, subspace_id);
    encode_u16_be(
        out,
        u16::try_from(attr_indexes.len()).map_err(|_| anyhow!("legacy subspace is too wide"))?,
    );
    encode_u32_be(
        out,
        u32::try_from(regions.len()).map_err(|_| anyhow!("legacy region count exceeds u32"))?,
    );

    for attr_index in attr_indexes {
        encode_u16_be(out, *attr_index);
    }

    for (region_idx, ((lower_coord, upper_coord), region_id)) in
        regions.into_iter().zip(region_ids).enumerate()
    {
        let replica_set: &[u64] = if replica_sets.is_empty() {
            &[]
        } else {
            let replica_set_idx = (region_idx * replica_sets.len()) / region_count;
            replica_sets[replica_set_idx].as_slice()
        };
        encode_u64_be(out, region_id);
        encode_u16_be(
            out,
            u16::try_from(attr_indexes.len()).map_err(|_| anyhow!("legacy region is too wide"))?,
        );
        encode_u8(
            out,
            u8::try_from(replica_set.len())
                .map_err(|_| anyhow!("legacy replica count exceeds u8"))?,
        );

        for (lower_hash, upper_hash) in lower_coord.iter().zip(upper_coord.iter()) {
            encode_u64_be(out, *lower_hash);
            encode_u64_be(out, *upper_hash);
        }

        for server_id in replica_set {
            encode_u64_be(out, *server_id);
            encode_u64_be(out, ids.allocate());
        }
    }

    Ok(())
}

fn legacy_replica_sets(server_ids: &[u64], replicas: usize, partitions: u32) -> Vec<Vec<u64>> {
    if server_ids.is_empty() || replicas == 0 {
        return Vec::new();
    }

    if server_ids.len() <= replicas {
        let mut replica_sets = Vec::with_capacity(server_ids.len());
        for start in 0..server_ids.len() {
            let mut replica_set = Vec::with_capacity(server_ids.len());
            for offset in 0..server_ids.len() {
                let idx = (start + offset) % server_ids.len();
                replica_set.push(server_ids[idx]);
            }
            replica_sets.push(replica_set);
        }
        return replica_sets;
    }

    let partitions = partitions.max(1) as usize;
    let mut replica_sets = Vec::new();

    for start in 0..server_ids.len() {
        for stride in 1..=partitions {
            if start + stride * (replicas - 1) >= server_ids.len() {
                break;
            }

            let mut replica_set = Vec::with_capacity(replicas);
            for replica in 0..replicas {
                let idx = start + stride * replica;
                replica_set.push(server_ids[idx]);
            }
            replica_sets.push(replica_set);
        }
    }

    replica_sets
}

fn legacy_partition_regions(num_attrs: usize, partitions: u32) -> Vec<(Vec<u64>, Vec<u64>)> {
    assert!(
        num_attrs > 0,
        "legacy partitioning requires at least one attribute"
    );
    assert!(
        partitions > 0,
        "legacy partitioning requires at least one partition"
    );

    let mut attrs_per_dimension = f64::from(partitions);
    attrs_per_dimension = attrs_per_dimension.powf(1.0 / num_attrs as f64);
    let mut dimensions = vec![attrs_per_dimension as u64; num_attrs];
    let mut partition_count = dimensions.len() as u64 * dimensions[0];

    for dimension in dimensions.iter_mut().take(num_attrs) {
        if partition_count >= u64::from(partitions) {
            break;
        }
        partition_count = partition_count / *dimension;
        *dimension += 1;
        partition_count *= *dimension;
    }

    let bigger = dimensions[0];
    let (bigger_lbs, bigger_ubs) = legacy_partition_points(bigger);
    let smaller = dimensions[dimensions.len() - 1];
    let (smaller_lbs, smaller_ubs) = legacy_partition_points(smaller);
    let mut lower_coord = vec![0_u64; num_attrs];
    let mut upper_coord = vec![0_u64; num_attrs];
    let mut regions = Vec::new();
    legacy_generate_regions(
        0,
        &dimensions,
        bigger,
        &bigger_lbs,
        &bigger_ubs,
        smaller,
        &smaller_lbs,
        &smaller_ubs,
        &mut lower_coord,
        &mut upper_coord,
        &mut regions,
    );
    regions
}

fn legacy_partition_points(intervals: u64) -> (Vec<u64>, Vec<u64>) {
    let interval = (0x8000_0000_0000_0000_u64 / intervals) * 2;
    let mut lowers = Vec::with_capacity(intervals as usize);
    let mut uppers = Vec::with_capacity(intervals as usize);

    for index in 0..intervals {
        lowers.push(index * interval);
    }

    for lower in lowers.iter().skip(1) {
        uppers.push(*lower - 1);
    }
    uppers.push(u64::MAX);
    (lowers, uppers)
}

#[allow(clippy::too_many_arguments)]
fn legacy_generate_regions(
    idx: usize,
    dimensions: &[u64],
    bigger: u64,
    bigger_lbs: &[u64],
    bigger_ubs: &[u64],
    smaller: u64,
    smaller_lbs: &[u64],
    smaller_ubs: &[u64],
    lower_coord: &mut [u64],
    upper_coord: &mut [u64],
    regions: &mut Vec<(Vec<u64>, Vec<u64>)>,
) {
    if idx >= dimensions.len() {
        regions.push((lower_coord.to_vec(), upper_coord.to_vec()));
        return;
    }

    let (lbs, ubs) = if dimensions[idx] == bigger {
        (bigger_lbs, bigger_ubs)
    } else {
        debug_assert_eq!(dimensions[idx], smaller);
        (smaller_lbs, smaller_ubs)
    };

    for (lower, upper) in lbs.iter().zip(ubs.iter()) {
        lower_coord[idx] = *lower;
        upper_coord[idx] = *upper;
        legacy_generate_regions(
            idx + 1,
            dimensions,
            bigger,
            bigger_lbs,
            bigger_ubs,
            smaller,
            smaller_lbs,
            smaller_ubs,
            lower_coord,
            upper_coord,
            regions,
        );
    }
}

fn legacy_hyperdatatype(kind: &ValueKind) -> Result<u16> {
    match kind {
        ValueKind::Bool => Err(anyhow!(
            "legacy HyperDex coordinator config does not support bool attributes"
        )),
        ValueKind::Int => Ok(LEGACY_HYPERDATATYPE_INT64),
        ValueKind::Float => Ok(LEGACY_HYPERDATATYPE_FLOAT),
        ValueKind::Bytes => Ok(LEGACY_HYPERDATATYPE_BYTES),
        ValueKind::String => Ok(LEGACY_HYPERDATATYPE_STRING),
        ValueKind::Document => Ok(LEGACY_HYPERDATATYPE_DOCUMENT),
        ValueKind::Timestamp(unit) => Ok(match unit {
            data_model::TimeUnit::Second => LEGACY_HYPERDATATYPE_TIMESTAMP_GENERIC,
            data_model::TimeUnit::Minute => LEGACY_HYPERDATATYPE_TIMESTAMP_GENERIC + 2,
            data_model::TimeUnit::Hour => LEGACY_HYPERDATATYPE_TIMESTAMP_GENERIC + 3,
            data_model::TimeUnit::Day => LEGACY_HYPERDATATYPE_TIMESTAMP_GENERIC + 4,
            data_model::TimeUnit::Week => LEGACY_HYPERDATATYPE_TIMESTAMP_GENERIC + 5,
            data_model::TimeUnit::Month => LEGACY_HYPERDATATYPE_TIMESTAMP_GENERIC + 6,
        }),
        ValueKind::List(elem) => {
            Ok(LEGACY_HYPERDATATYPE_LIST_GENERIC | legacy_primitive_code(elem)?)
        }
        ValueKind::Set(elem) => Ok(LEGACY_HYPERDATATYPE_SET_GENERIC | legacy_primitive_code(elem)?),
        ValueKind::Map { key, value } => Ok(LEGACY_HYPERDATATYPE_MAP_GENERIC
            | ((legacy_primitive_code(key)? & 0x003f) << 3)
            | legacy_primitive_code(value)?),
    }
}

fn legacy_primitive_code(kind: &ValueKind) -> Result<u16> {
    match kind {
        ValueKind::Bytes => Ok(LEGACY_HYPERDATATYPE_BYTES & 0x2407),
        ValueKind::String => Ok(LEGACY_HYPERDATATYPE_STRING & 0x2407),
        ValueKind::Int => Ok(LEGACY_HYPERDATATYPE_INT64 & 0x2407),
        ValueKind::Float => Ok(LEGACY_HYPERDATATYPE_FLOAT & 0x2407),
        ValueKind::Document => Ok(LEGACY_HYPERDATATYPE_DOCUMENT & 0x2407),
        other => Err(anyhow!(
            "legacy HyperDex container types require primitive elements, got {other:?}"
        )),
    }
}

fn encode_legacy_slice(out: &mut Vec<u8>, bytes: &[u8]) -> Result<()> {
    let len = u64::try_from(bytes.len()).map_err(|_| anyhow!("legacy byte slice exceeds u64"))?;
    encode_legacy_varint(out, len);
    out.extend_from_slice(bytes);
    Ok(())
}

fn encode_legacy_varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;

        if value != 0 {
            byte |= 0x80;
        }

        out.push(byte);

        if value == 0 {
            break;
        }
    }
}

fn encode_u64_be(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn encode_u32_be(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn encode_u16_be(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn encode_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

async fn read_busybee_frame_from_stream(
    stream: &mut tokio::net::TcpStream,
) -> Result<Option<BusyBeeFrame>> {
    let mut header = [0_u8; hyperdex_admin_protocol::BUSYBEE_HEADER_SIZE];
    match stream.read_exact(&mut header).await {
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err.into()),
    }

    let total_len = (u32::from_be_bytes(header) & 0x00ff_ffff) as usize;
    if total_len < hyperdex_admin_protocol::BUSYBEE_HEADER_SIZE {
        anyhow::bail!("busybee frame size {total_len} is too small");
    }
    if total_len > hyperdex_admin_protocol::MAX_BUSYBEE_FRAME_SIZE {
        anyhow::bail!(
            "busybee frame size {total_len} exceeds max {}",
            hyperdex_admin_protocol::MAX_BUSYBEE_FRAME_SIZE
        );
    }

    let mut encoded = Vec::with_capacity(total_len);
    encoded.extend_from_slice(&header);
    let mut payload = vec![0_u8; total_len - hyperdex_admin_protocol::BUSYBEE_HEADER_SIZE];
    stream.read_exact(&mut payload).await?;
    encoded.extend_from_slice(&payload);
    Ok(Some(BusyBeeFrame::decode(&encoded)?))
}

async fn write_busybee_frame_to_stream(
    stream: &mut tokio::net::TcpStream,
    frame: &BusyBeeFrame,
) -> Result<()> {
    stream.write_all(&frame.encode()?).await?;
    stream.flush().await?;
    Ok(())
}

async fn serve_coordinator_admin_connection(
    stream: &mut TcpStream,
    runtime: &ClusterRuntime,
    space_add_decoder: LegacySpaceAddDecoder,
    config_encoder: LegacyConfigEncoder,
) -> Result<()> {
    let mut session = CoordinatorAdminSession::new();
    let bootstrap_address = stream.local_addr()?;

    while let Some(frame) = read_busybee_frame_from_stream(stream).await? {
        handle_coordinator_admin_frame(
            &mut session,
            runtime,
            bootstrap_address,
            frame,
            space_add_decoder.as_ref(),
            config_encoder.as_ref(),
        )
        .await?;

        for response in session.take_pending_frames() {
            write_busybee_frame_to_stream(stream, &response).await?;
        }
    }

    Ok(())
}

const MAX_COORDINATOR_CONTROL_METHOD_LEN: usize = "wait_until_stable".len();

fn is_coordinator_control_method(method: &str) -> bool {
    matches!(
        method,
        "daemon_register" | "space_add" | "space_rm" | "wait_until_stable" | "config_get"
    )
}

async fn classify_coordinator_public_protocol(
    stream: &TcpStream,
) -> Result<CoordinatorPublicProtocol> {
    let mut probe = [0_u8; COORDINATOR_CONTROL_HEADER_SIZE + MAX_COORDINATOR_CONTROL_METHOD_LEN];

    loop {
        let read = stream.peek(&mut probe).await?;
        if read == 0 {
            anyhow::bail!("coordinator public connection closed before sending a request");
        }

        if read < COORDINATOR_CONTROL_HEADER_SIZE {
            stream.readable().await?;
            continue;
        }

        let method_len = u16::from_be_bytes([probe[0], probe[1]]) as usize;
        if method_len == 0 || method_len > MAX_COORDINATOR_CONTROL_METHOD_LEN {
            return Ok(CoordinatorPublicProtocol::LegacyAdmin);
        }

        let needed = COORDINATOR_CONTROL_HEADER_SIZE + method_len;
        if read < needed {
            stream.readable().await?;
            continue;
        }

        let Ok(method) = std::str::from_utf8(&probe[COORDINATOR_CONTROL_HEADER_SIZE..needed])
        else {
            return Ok(CoordinatorPublicProtocol::LegacyAdmin);
        };

        if is_coordinator_control_method(method) {
            return Ok(CoordinatorPublicProtocol::Control);
        }

        return Ok(CoordinatorPublicProtocol::LegacyAdmin);
    }
}

enum CoordinatorPublicProtocol {
    Control,
    LegacyAdmin,
}

async fn serve_coordinator_control_connection_with<H, F>(
    stream: &mut TcpStream,
    handler: H,
) -> Result<()>
where
    H: Fn(String, CoordinatorAdminRequest) -> F,
    F: std::future::Future<Output = Result<CoordinatorControlResponse>>,
{
    let (method, request) = read_coordinator_control_request(stream).await?;
    let response = handler(method, request).await?;
    write_coordinator_control_response(stream, &response).await?;
    stream.flush().await?;
    Ok(())
}

pub async fn serve_coordinator_public_connection(
    mut stream: TcpStream,
    runtime: Arc<ClusterRuntime>,
) -> Result<()> {
    match classify_coordinator_public_protocol(&stream).await? {
        CoordinatorPublicProtocol::Control => {
            serve_coordinator_control_connection_with(&mut stream, move |method, request| {
                let runtime = runtime.clone();
                async move {
                    handle_coordinator_control_method(runtime.as_ref(), &method, request).await
                }
            })
            .await
        }
        CoordinatorPublicProtocol::LegacyAdmin => {
            serve_coordinator_admin_connection(
                &mut stream,
                runtime.as_ref(),
                Arc::new(default_legacy_space_add_decoder),
                Arc::new(default_legacy_config_encoder),
            )
            .await
        }
    }
}

async fn handle_coordinator_admin_frame(
    session: &mut CoordinatorAdminSession,
    runtime: &ClusterRuntime,
    bootstrap_address: SocketAddr,
    frame: BusyBeeFrame,
    space_add_decoder: &(dyn Fn(&[u8]) -> Result<Space> + Send + Sync),
    config_encoder: &(dyn Fn(&ConfigView) -> Result<Vec<u8>> + Send + Sync),
) -> Result<()> {
    if frame.flags & hyperdex_admin_protocol::BUSYBEE_HEADER_IDENTIFY != 0 {
        if frame.payload.len() != 2 * std::mem::size_of::<u64>() {
            anyhow::bail!("legacy admin identify frame must contain exactly two u64 values");
        }
        let peer_local_id =
            decode_be_u64_exact(&frame.payload[..8], "legacy admin identify local id")?;
        let peer_remote_id =
            decode_be_u64_exact(&frame.payload[8..16], "legacy admin identify remote id")?;
        if session.observe_identify(peer_local_id, peer_remote_id)? {
            session.queue_identify_response(peer_local_id);
        }
        return Ok(());
    }

    if frame.payload.len() == 1
        && ReplicantNetworkMsgtype::decode(frame.payload[0])? == ReplicantNetworkMsgtype::Bootstrap
    {
        session.queue_bootstrap_response(bootstrap_address);
        return Ok(());
    }

    match ReplicantAdminRequestMessage::decode(&frame.payload)? {
        ReplicantAdminRequestMessage::GetRobustParams { nonce } => {
            let command_nonce = session.allocate_robust_command_nonce();
            session.queue_robust_params(nonce, command_nonce);
        }
        ReplicantAdminRequestMessage::CondWait {
            nonce,
            object,
            condition,
            state,
        } => {
            if object == b"replicant" && condition == b"configuration" {
                let configuration =
                    legacy_bootstrap_configuration(session.sender_id, bootstrap_address);
                if state <= configuration.version {
                    session.queue_condition_completion(
                        nonce,
                        ReplicantReturnCode::Success,
                        configuration.version,
                        configuration.encode(),
                    );
                }
            } else if object == b"replicant" && condition == b"tick" {
                if state <= LEGACY_REPLICANT_TICK_STATE {
                    session.queue_condition_completion(
                        nonce,
                        ReplicantReturnCode::Success,
                        LEGACY_REPLICANT_TICK_STATE,
                        Vec::new(),
                    );
                }
            } else if object == b"hyperdex" && condition == b"config" {
                let config_state = legacy_condition_state(runtime.config_view()?.version);
                if state <= config_state {
                    session.queue_config_update(runtime, config_encoder, nonce)?;
                } else {
                    session.pending_config_waits.insert(nonce, state);
                }
            } else if object == b"hyperdex" && condition == b"stable" {
                let stable_version = legacy_condition_state(runtime.stable_version()?);
                if state <= stable_version {
                    session.queue_condition_completion(
                        nonce,
                        ReplicantReturnCode::Success,
                        stable_version,
                        Vec::new(),
                    );
                } else {
                    session.pending_waits.insert(nonce, state);
                }
            } else {
                session.queue_condition_completion(
                    nonce,
                    ReplicantReturnCode::CondNotFound,
                    state,
                    Vec::new(),
                );
            }
        }
        ReplicantAdminRequestMessage::Call {
            nonce,
            object,
            function,
            input,
        } => {
            if object != b"hyperdex" {
                session.queue_call_completion(
                    nonce,
                    ReplicantReturnCode::ObjNotFound,
                    Vec::new(),
                )?;
            } else if function == b"space_add" {
                let code = match space_add_decoder(&input) {
                    Ok(space) => {
                        handle_coordinator_admin_request(
                            runtime,
                            CoordinatorAdminRequest::SpaceAdd(space),
                        )
                        .await?
                    }
                    Err(_) => CoordinatorReturnCode::Malformed,
                };
                session.queue_call_completion(
                    nonce,
                    ReplicantReturnCode::Success,
                    code.encode().to_vec(),
                )?;
            } else if function == b"space_rm" {
                let name = extract_c_string(&input)?;
                let code = handle_coordinator_admin_request(
                    runtime,
                    CoordinatorAdminRequest::SpaceRm(name),
                )
                .await?;
                session.queue_call_completion(
                    nonce,
                    ReplicantReturnCode::Success,
                    code.encode().to_vec(),
                )?;
            } else {
                session.queue_call_completion(
                    nonce,
                    ReplicantReturnCode::FuncNotFound,
                    Vec::new(),
                )?;
            }
        }
        ReplicantAdminRequestMessage::CallRobust {
            nonce,
            object,
            function,
            input,
            ..
        } => {
            if object != b"hyperdex" {
                session.queue_call_completion(
                    nonce,
                    ReplicantReturnCode::ObjNotFound,
                    Vec::new(),
                )?;
            } else if function == b"space_add" {
                let code = match space_add_decoder(&input) {
                    Ok(space) => {
                        handle_coordinator_admin_request(
                            runtime,
                            CoordinatorAdminRequest::SpaceAdd(space),
                        )
                        .await?
                    }
                    Err(_) => CoordinatorReturnCode::Malformed,
                };
                session.queue_call_completion(
                    nonce,
                    ReplicantReturnCode::Success,
                    code.encode().to_vec(),
                )?;
            } else if function == b"space_rm" {
                let name = extract_c_string(&input)?;
                let code = handle_coordinator_admin_request(
                    runtime,
                    CoordinatorAdminRequest::SpaceRm(name),
                )
                .await?;
                session.queue_call_completion(
                    nonce,
                    ReplicantReturnCode::Success,
                    code.encode().to_vec(),
                )?;
            } else {
                session.queue_call_completion(
                    nonce,
                    ReplicantReturnCode::FuncNotFound,
                    Vec::new(),
                )?;
            }
        }
    }

    session.queue_ready_config_waits(runtime, config_encoder)?;
    session.queue_ready_waits(runtime)?;
    Ok(())
}

fn extract_c_string(bytes: &[u8]) -> Result<String> {
    let nul = bytes
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| anyhow!("expected nul-terminated string payload"))?;
    Ok(std::str::from_utf8(&bytes[..nul])?.to_owned())
}

impl CoordinatorControlService {
    pub async fn bind(address: SocketAddr) -> Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(address).await?,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }

    pub async fn serve_once_with<H, F>(&self, handler: H) -> Result<()>
    where
        H: Fn(String, CoordinatorAdminRequest) -> F,
        F: std::future::Future<Output = Result<CoordinatorControlResponse>>,
    {
        let (mut stream, _) = self.listener.accept().await?;
        serve_coordinator_control_connection_with(&mut stream, handler).await
    }

    pub async fn serve_forever_with<H, F>(&self, handler: H) -> Result<()>
    where
        H: Fn(String, CoordinatorAdminRequest) -> F,
        F: std::future::Future<Output = Result<CoordinatorControlResponse>>,
    {
        loop {
            match self.serve_once_with(&handler).await {
                Ok(()) => {}
                Err(err) if is_recoverable_coordinator_control_error(&err) => continue,
                Err(err) => return Err(err),
            }
        }
    }
}

fn is_recoverable_coordinator_control_error(err: &anyhow::Error) -> bool {
    err.downcast_ref::<std::io::Error>().is_some_and(|io_err| {
        matches!(
            io_err.kind(),
            std::io::ErrorKind::UnexpectedEof
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::InvalidData
        )
    }) || err.downcast_ref::<serde_json::Error>().is_some()
        || err.downcast_ref::<std::string::FromUtf8Error>().is_some()
}

impl ConsensusRuntime {
    pub fn name(&self) -> &'static str {
        match self {
            Self::SingleNode => "single-node",
            Self::Mirror => "mirror",
            Self::OmniPaxos => "omnipaxos",
            Self::OpenRaft => "openraft",
        }
    }
}

impl PlacementRuntime {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Hyperspace => "hyperspace",
            Self::Rendezvous => "rendezvous",
        }
    }
}

impl StorageRuntime {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::RocksDb => "rocksdb",
        }
    }
}

impl TransportRuntime {
    pub fn name(&self) -> &'static str {
        match self {
            Self::InProcess => "in-process",
            Self::Grpc => "grpc",
        }
    }
}

pub fn select_consensus_backend(config: &ClusterConfig) -> Result<ConsensusRuntime> {
    match config.consensus {
        ConsensusBackend::SingleNode => Ok(ConsensusRuntime::SingleNode),
        ConsensusBackend::Mirror => Ok(ConsensusRuntime::Mirror),
        ConsensusBackend::OmniPaxos => {
            #[cfg(feature = "omnipaxos")]
            {
                Ok(ConsensusRuntime::OmniPaxos)
            }
            #[cfg(not(feature = "omnipaxos"))]
            {
                Err(anyhow!(
                    "consensus backend `omnipaxos` requires server feature `omnipaxos`"
                ))
            }
        }
        ConsensusBackend::OpenRaft => {
            #[cfg(feature = "openraft")]
            {
                Ok(ConsensusRuntime::OpenRaft)
            }
            #[cfg(not(feature = "openraft"))]
            {
                Err(anyhow!(
                    "consensus backend `openraft` requires server feature `openraft`"
                ))
            }
        }
    }
}

fn select_placement_backend(
    config: &ClusterConfig,
) -> (Arc<dyn PlacementStrategy>, PlacementRuntime) {
    match config.placement {
        PlacementBackend::Hyperspace => (
            Arc::new(HyperSpacePlacement::default()),
            PlacementRuntime::Hyperspace,
        ),
        PlacementBackend::Rendezvous => {
            (Arc::new(RendezvousPlacement), PlacementRuntime::Rendezvous)
        }
    }
}

fn select_storage_backend(
    config: &ClusterConfig,
    data_dir: Option<&std::path::Path>,
) -> Result<(Arc<dyn StorageEngine>, StorageRuntime, Option<TempDir>)> {
    match config.storage {
        StorageBackend::Memory => Ok((Arc::new(MemoryEngine::new()), StorageRuntime::Memory, None)),
        StorageBackend::RocksDb => {
            if let Some(path) = data_dir {
                let engine = RocksEngine::open(path)?;
                Ok((Arc::new(engine), StorageRuntime::RocksDb, None))
            } else {
                let dir = tempfile::tempdir()?;
                let engine = RocksEngine::open(dir.path())?;
                Ok((Arc::new(engine), StorageRuntime::RocksDb, Some(dir)))
            }
        }
    }
}

fn select_internode_transport(config: &ClusterConfig) -> TransportRuntime {
    match config.internode_transport {
        TransportBackend::InProcess => {
            let _ = InProcessTransport;
            TransportRuntime::InProcess
        }
        TransportBackend::Grpc => TransportRuntime::Grpc,
    }
}

fn upsert_cluster_node(nodes: &mut Vec<ClusterNode>, node: ClusterNode) -> bool {
    if let Some(existing) = nodes.iter_mut().find(|existing| existing.id == node.id) {
        if *existing == node {
            return false;
        }
        *existing = node;
    } else {
        nodes.push(node);
    }

    nodes.sort_by_key(|node| node.id);
    true
}

pub async fn handle_legacy_request(
    runtime: &ClusterRuntime,
    header: RequestHeader,
    body: &[u8],
) -> Result<(ResponseHeader, Vec<u8>)> {
    match header.message_type {
        LegacyMessageType::ReqAtomic => {
            let space = legacy_space(runtime)?;
            let (nonce, request_body) = legacy_decode_request_nonce(body)?;
            let request = if let Ok(request) = decode_protocol_atomic_request(request_body) {
                request
            } else {
                legacy_named_atomic_request(&space, &AtomicRequest::decode_body(request_body)?)?
            };
            if let Err(err) = legacy_validate_atomic_request(&space, &request) {
                tracing::warn!("rejecting legacy atomic request with bad dimension spec: {err:#}");
                return Ok(legacy_atomic_response(
                    header.target_virtual_server,
                    nonce,
                    LegacyReturnCode::BadDimensionSpec,
                ));
            }
            let key = request.key.clone();
            let exists = legacy_record_exists(runtime, &space.name, &key).await?;
            let checks = match legacy_checks_from_protocol(&space, &request.checks) {
                Ok(checks) => checks,
                Err(err) => {
                    tracing::warn!(
                        "rejecting legacy atomic checks with bad dimension spec: {err:#}"
                    );
                    return Ok(legacy_atomic_response(
                        header.target_virtual_server,
                        nonce,
                        LegacyReturnCode::BadDimensionSpec,
                    ));
                }
            };

            let status = if request.fail_if_found && exists {
                LegacyReturnCode::CompareFailed
            } else if request.fail_if_not_found && !exists {
                LegacyReturnCode::NotFound
            } else if !request.erase {
                let response = if legacy_atomic_can_use_runtime_mutations(&space, &request.funcalls)
                {
                    let mutations = match legacy_mutations_from_protocol_funcalls(
                        &space,
                        &request.funcalls,
                    ) {
                        Ok(mutations) => mutations,
                        Err(err) => {
                            tracing::warn!(
                                "rejecting legacy atomic funcalls with bad dimension spec: {err:#}"
                            );
                            return Ok(legacy_atomic_response(
                                header.target_virtual_server,
                                nonce,
                                LegacyReturnCode::BadDimensionSpec,
                            ));
                        }
                    };
                    if checks.is_empty() {
                        HyperdexClientService::handle(
                            runtime,
                            ClientRequest::Put {
                                space: space.name.clone(),
                                key: request.key.clone().into(),
                                mutations,
                            },
                        )
                        .await?
                    } else {
                        HyperdexClientService::handle(
                            runtime,
                            ClientRequest::ConditionalPut {
                                space: space.name.clone(),
                                key: request.key.clone().into(),
                                checks: checks.clone(),
                                mutations,
                            },
                        )
                        .await?
                    }
                } else {
                    match legacy_apply_atomic_direct(
                        runtime,
                        &space,
                        request.clone(),
                        checks.clone(),
                    )
                    .await
                    {
                        Ok(response) => response,
                        Err(err) => {
                            tracing::warn!(
                                "rejecting legacy atomic direct funcall with bad dimension spec: {err:#}"
                            );
                            return Ok(legacy_atomic_response(
                                header.target_virtual_server,
                                nonce,
                                LegacyReturnCode::BadDimensionSpec,
                            ));
                        }
                    }
                };
                legacy_atomic_status(response)?
            } else {
                if !checks.is_empty() || !request.funcalls.is_empty() {
                    anyhow::bail!("legacy delete path does not yet support checks or funcalls");
                }
                let response = HyperdexClientService::handle(
                    runtime,
                    ClientRequest::Delete {
                        space: space.name.clone(),
                        key: request.key.into(),
                    },
                )
                .await?;
                legacy_atomic_status(response)?
            };

            Ok(legacy_atomic_response(
                header.target_virtual_server,
                nonce,
                status,
            ))
        }
        LegacyMessageType::ReqSearchStart => {
            let space = legacy_space(runtime)?;
            let (nonce, request_body) = legacy_decode_request_nonce(body)?;
            let (search_id, checks, format) = legacy_search_start(runtime, request_body)?;
            let response = HyperdexClientService::handle(
                runtime,
                ClientRequest::Search {
                    space: space.name.clone(),
                    checks,
                },
            )
            .await?;

            let ClientResponse::SearchResult(records) = response else {
                anyhow::bail!("unexpected runtime response to search request");
            };

            runtime.legacy_searches_guard()?.insert(
                search_id,
                LegacySearchState {
                    records: VecDeque::from(records),
                    format,
                },
            );

            legacy_search_response(runtime, &space, header, nonce, search_id)
        }
        LegacyMessageType::ReqSearchNext => {
            let space = legacy_space(runtime)?;
            let (nonce, request_body) = legacy_decode_request_nonce(body)?;
            let search_id = decode_protocol_search_continue(request_body)?;
            legacy_search_response(runtime, &space, header, nonce, search_id)
        }
        LegacyMessageType::ReqSearchStop => {
            let (nonce, request_body) = legacy_decode_request_nonce(body)?;
            let search_id = decode_protocol_search_continue(request_body)?;
            runtime.legacy_searches_guard()?.remove(&search_id);

            Ok((
                ResponseHeader {
                    message_type: LegacyMessageType::RespSearchDone,
                    target_virtual_server: header.target_virtual_server,
                    nonce,
                },
                encode_protocol_search_done(),
            ))
        }
        LegacyMessageType::ReqCount => {
            let space = legacy_space(runtime)?;
            let (nonce, request_body) = legacy_decode_request_nonce(body)?;
            let response = HyperdexClientService::handle(
                runtime,
                ClientRequest::Count {
                    space: space.name.clone(),
                    checks: legacy_count_checks(runtime, request_body)?,
                },
            )
            .await?;

            let ClientResponse::Count(count) = response else {
                anyhow::bail!("unexpected runtime response to count request");
            };

            Ok((
                ResponseHeader {
                    message_type: LegacyMessageType::RespCount,
                    target_virtual_server: header.target_virtual_server,
                    nonce,
                },
                encode_protocol_count_response(count).to_vec(),
            ))
        }
        LegacyMessageType::ReqGet => {
            let space = legacy_space(runtime)?;
            let (nonce, request_body) = legacy_decode_request_nonce(body)?;
            let (key, format) = legacy_get_request_key(request_body)?;
            let response = HyperdexClientService::handle(
                runtime,
                ClientRequest::Get {
                    space: space.name.clone(),
                    key: key.into(),
                },
            )
            .await?;

            let ClientResponse::Record(record) = response else {
                anyhow::bail!("unexpected runtime response to get request");
            };

            let body = match format {
                LegacyBodyFormat::Protocol => encode_protocol_get_response(&match record {
                    Some(record) => ProtocolGetResponse {
                        status: LegacyReturnCode::Success as u16,
                        values: legacy_protocol_values_from_record(&space, &record)?,
                    },
                    None => ProtocolGetResponse {
                        status: LegacyReturnCode::NotFound as u16,
                        values: Vec::new(),
                    },
                }),
                LegacyBodyFormat::Named => match record {
                    Some(record) => GetResponse {
                        status: LegacyReturnCode::Success,
                        attributes: legacy_named_attributes_from_record(&space, &record)?,
                    }
                    .encode_body(),
                    None => GetResponse {
                        status: LegacyReturnCode::NotFound,
                        attributes: Vec::new(),
                    }
                    .encode_body(),
                },
            };

            Ok((
                ResponseHeader {
                    message_type: LegacyMessageType::RespGet,
                    target_virtual_server: header.target_virtual_server,
                    nonce,
                },
                body,
            ))
        }
        _ => Ok((config_mismatch_response(header), Vec::new())),
    }
}

fn legacy_atomic_response(
    target_virtual_server: u64,
    nonce: u64,
    status: LegacyReturnCode,
) -> (ResponseHeader, Vec<u8>) {
    (
        ResponseHeader {
            message_type: LegacyMessageType::RespAtomic,
            target_virtual_server,
            nonce,
        },
        encode_protocol_atomic_response(status as u16).to_vec(),
    )
}

fn legacy_space(runtime: &ClusterRuntime) -> Result<Space> {
    let space_name = runtime.only_space_name()?;
    runtime
        .catalog
        .get_space(&space_name)?
        .ok_or_else(|| anyhow!("catalog lost space definition for {space_name}"))
}

fn legacy_named_attribute<'a>(space: &'a Space, attribute: &str) -> Result<(u16, &'a ValueKind)> {
    if attribute == space.key_attribute {
        return Ok((0, &ValueKind::Bytes));
    }

    let (index, definition) = space
        .attributes
        .iter()
        .enumerate()
        .find(|(_, definition)| definition.name == attribute)
        .ok_or_else(|| anyhow!("legacy attribute `{attribute}` is not present in the schema"))?;

    Ok(((index + 1) as u16, &definition.kind))
}

fn legacy_named_value(kind: &ValueKind, value: &GetValue) -> Result<Value> {
    match (kind, value) {
        (_, GetValue::Null) => Ok(Value::Null),
        (ValueKind::Bool, GetValue::Bool(value)) => Ok(Value::Bool(*value)),
        (ValueKind::Int, GetValue::Int(value)) => Ok(Value::Int(*value)),
        (ValueKind::Bytes, GetValue::Bytes(value)) => Ok(Value::Bytes(value.clone().into())),
        (ValueKind::Bytes, GetValue::String(value)) => {
            Ok(Value::Bytes(value.as_bytes().to_vec().into()))
        }
        (ValueKind::String | ValueKind::Document, GetValue::String(value)) => {
            Ok(Value::String(value.clone()))
        }
        (ValueKind::String | ValueKind::Document, GetValue::Bytes(value)) => Ok(Value::String(
            std::str::from_utf8(value)
                .map_err(|_| anyhow!("legacy string value is not valid utf-8"))?
                .to_owned(),
        )),
        _ => Err(anyhow!(
            "legacy value {value:?} does not match schema kind {kind:?}"
        )),
    }
}

fn legacy_named_value_to_protocol(kind: &ValueKind, value: &GetValue) -> Result<Vec<u8>> {
    let value = legacy_named_value(kind, value)?;
    legacy_protocol_value_from_kind(kind, &value)
}

fn legacy_named_predicate(predicate: LegacyPredicate) -> u16 {
    predicate as u16
}

fn legacy_named_checks(space: &Space, checks: &[LegacyCheck]) -> Result<Vec<Check>> {
    checks
        .iter()
        .map(|check| {
            let (_, kind) = legacy_named_attribute(space, &check.attribute)?;
            Ok(Check {
                attribute: check.attribute.clone(),
                predicate: legacy_protocol_predicate(legacy_named_predicate(check.predicate))?,
                value: legacy_named_value(kind, &check.value)?,
            })
        })
        .collect()
}

fn legacy_named_check_to_protocol(
    space: &Space,
    check: &LegacyCheck,
) -> Result<ProtocolAttributeCheck> {
    let (attr, kind) = legacy_named_attribute(space, &check.attribute)?;
    Ok(ProtocolAttributeCheck {
        attr,
        value: legacy_named_value_to_protocol(kind, &check.value)?,
        datatype: legacy_hyperdatatype(kind)?,
        predicate: legacy_named_predicate(check.predicate),
    })
}

fn legacy_named_funcall_to_protocol(
    space: &Space,
    funcall: &LegacyFuncall,
) -> Result<ProtocolFuncall> {
    let (attr, attribute_kind) = legacy_named_attribute(space, &funcall.attribute)?;

    let (name, arg1_kind, arg2_kind, arg2_value) = match funcall.name {
        LegacyFuncallName::Set => (FUNC_SET, Some(attribute_kind), None, None),
        LegacyFuncallName::StringAppend => (FUNC_STRING_APPEND, Some(attribute_kind), None, None),
        LegacyFuncallName::StringPrepend => (FUNC_STRING_PREPEND, Some(attribute_kind), None, None),
        LegacyFuncallName::NumAdd => (FUNC_NUM_ADD, Some(attribute_kind), None, None),
        LegacyFuncallName::NumSub => (FUNC_NUM_SUB, Some(attribute_kind), None, None),
        LegacyFuncallName::NumMul => (FUNC_NUM_MUL, Some(attribute_kind), None, None),
        LegacyFuncallName::NumDiv => (FUNC_NUM_DIV, Some(attribute_kind), None, None),
        LegacyFuncallName::NumMod => (FUNC_NUM_MOD, Some(attribute_kind), None, None),
        LegacyFuncallName::NumAnd => (FUNC_NUM_AND, Some(attribute_kind), None, None),
        LegacyFuncallName::NumOr => (FUNC_NUM_OR, Some(attribute_kind), None, None),
        LegacyFuncallName::NumXor => (FUNC_NUM_XOR, Some(attribute_kind), None, None),
        LegacyFuncallName::ListLPush => match attribute_kind {
            ValueKind::List(element_kind) => {
                (FUNC_LIST_LPUSH, Some(element_kind.as_ref()), None, None)
            }
            other => {
                return Err(anyhow!(
                    "legacy list push requires a list attribute, found {other:?}"
                ));
            }
        },
        LegacyFuncallName::ListRPush => match attribute_kind {
            ValueKind::List(element_kind) => {
                (FUNC_LIST_RPUSH, Some(element_kind.as_ref()), None, None)
            }
            other => {
                return Err(anyhow!(
                    "legacy list push requires a list attribute, found {other:?}"
                ));
            }
        },
        LegacyFuncallName::SetAdd => match attribute_kind {
            ValueKind::Set(element_kind) => (FUNC_SET_ADD, Some(element_kind.as_ref()), None, None),
            other => {
                return Err(anyhow!(
                    "legacy set add requires a set attribute, found {other:?}"
                ));
            }
        },
        LegacyFuncallName::SetRemove => match attribute_kind {
            ValueKind::Set(element_kind) => {
                (FUNC_SET_REMOVE, Some(element_kind.as_ref()), None, None)
            }
            other => {
                return Err(anyhow!(
                    "legacy set remove requires a set attribute, found {other:?}"
                ));
            }
        },
        LegacyFuncallName::SetIntersect => (FUNC_SET_INTERSECT, Some(attribute_kind), None, None),
        LegacyFuncallName::SetUnion => (FUNC_SET_UNION, Some(attribute_kind), None, None),
        LegacyFuncallName::MapAdd => match attribute_kind {
            ValueKind::Map { key, value } => (
                FUNC_MAP_ADD,
                Some(value.as_ref()),
                Some(key.as_ref()),
                funcall.arg2.as_ref(),
            ),
            other => {
                return Err(anyhow!(
                    "legacy map add requires a map attribute, found {other:?}"
                ));
            }
        },
        LegacyFuncallName::MapRemove => match attribute_kind {
            ValueKind::Map { key, .. } => (
                FUNC_MAP_REMOVE,
                None,
                Some(key.as_ref()),
                Some(&funcall.arg1),
            ),
            other => {
                return Err(anyhow!(
                    "legacy map remove requires a map attribute, found {other:?}"
                ));
            }
        },
    };

    let (arg1, arg1_datatype) = match arg1_kind {
        Some(kind) => (
            legacy_named_value_to_protocol(kind, &funcall.arg1)?,
            legacy_hyperdatatype(kind)?,
        ),
        None => (Vec::new(), 0),
    };

    let (arg2, arg2_datatype) = match (arg2_kind, arg2_value) {
        (Some(kind), Some(value)) => (
            legacy_named_value_to_protocol(kind, value)?,
            legacy_hyperdatatype(kind)?,
        ),
        (Some(_), None) => {
            return Err(anyhow!(
                "legacy funcall {:?} is missing its second argument",
                funcall.name
            ));
        }
        (None, _) => (Vec::new(), 0),
    };

    Ok(ProtocolFuncall {
        attr,
        name,
        arg1,
        arg1_datatype,
        arg2,
        arg2_datatype,
    })
}

fn legacy_named_atomic_request(
    space: &Space,
    request: &AtomicRequest,
) -> Result<ProtocolKeyChange> {
    Ok(ProtocolKeyChange {
        key: request.key.clone(),
        erase: request.flags & legacy_protocol::LEGACY_ATOMIC_FLAG_WRITE == 0,
        fail_if_not_found: request.flags & legacy_protocol::LEGACY_ATOMIC_FLAG_FAIL_IF_NOT_FOUND
            != 0,
        fail_if_found: request.flags & legacy_protocol::LEGACY_ATOMIC_FLAG_FAIL_IF_FOUND != 0,
        checks: request
            .checks
            .iter()
            .map(|check| legacy_named_check_to_protocol(space, check))
            .collect::<Result<Vec<_>>>()?,
        funcalls: request
            .funcalls
            .iter()
            .map(|funcall| legacy_named_funcall_to_protocol(space, funcall))
            .collect::<Result<Vec<_>>>()?,
    })
}

fn legacy_get_request_key(request_body: &[u8]) -> Result<(Vec<u8>, LegacyBodyFormat)> {
    if let Ok(key) = decode_protocol_get_request(request_body) {
        return Ok((key, LegacyBodyFormat::Protocol));
    }

    Ok((
        GetRequest::decode_body(request_body)?.key,
        LegacyBodyFormat::Named,
    ))
}

fn legacy_search_start(
    runtime: &ClusterRuntime,
    request_body: &[u8],
) -> Result<(u64, Vec<Check>, LegacyBodyFormat)> {
    let space = legacy_space(runtime)?;

    if let Ok(request) = decode_protocol_search_start(request_body) {
        return Ok((
            request.search_id,
            legacy_checks_from_protocol(&space, &request.checks)?,
            LegacyBodyFormat::Protocol,
        ));
    }

    let request = SearchStartRequest::decode_body(request_body)?;
    if request.space != space.name {
        anyhow::bail!(
            "legacy search request targeted unknown space `{}`; runtime only serves `{}`",
            request.space,
            space.name
        );
    }

    Ok((
        request.search_id,
        legacy_named_checks(&space, &request.checks)?,
        LegacyBodyFormat::Named,
    ))
}

fn legacy_named_value_from_record(value: &Value) -> Result<GetValue> {
    match value {
        Value::Null => Ok(GetValue::Null),
        Value::Bool(value) => Ok(GetValue::Bool(*value)),
        Value::Int(value) => Ok(GetValue::Int(*value)),
        Value::Bytes(value) => Ok(GetValue::Bytes(value.to_vec())),
        Value::String(value) => Ok(GetValue::String(value.clone())),
        other => Err(anyhow!(
            "legacy named response does not support {other:?} yet"
        )),
    }
}

fn legacy_default_value_for_kind(kind: &ValueKind) -> Result<Value> {
    match kind {
        ValueKind::Bytes | ValueKind::String | ValueKind::Document => {
            Ok(Value::Bytes(Bytes::new()))
        }
        ValueKind::Int => Ok(Value::Int(0)),
        ValueKind::Float => Ok(Value::Float(0.0.into())),
        ValueKind::List(_) => Ok(Value::List(Vec::new())),
        ValueKind::Set(_) => Ok(Value::Set(BTreeSet::new())),
        ValueKind::Map { .. } => Ok(Value::Map(BTreeMap::new())),
        ValueKind::Bool | ValueKind::Timestamp(_) => Err(anyhow!(
            "legacy daemon protocol does not support {kind:?} yet"
        )),
    }
}

fn legacy_record_value<'a>(
    record: &'a Record,
    attribute: &AttributeDefinition,
) -> Result<Cow<'a, Value>> {
    match record.attributes.get(&attribute.name) {
        Some(value) => Ok(Cow::Borrowed(value)),
        None => Ok(Cow::Owned(legacy_default_value_for_kind(&attribute.kind)?)),
    }
}

fn legacy_named_attributes_from_record(
    space: &Space,
    record: &Record,
) -> Result<Vec<GetAttribute>> {
    space
        .attributes
        .iter()
        .map(|attribute| {
            let value = legacy_record_value(record, attribute)?;
            Ok(GetAttribute {
                name: attribute.name.clone(),
                value: legacy_named_value_from_record(value.as_ref())?,
            })
        })
        .collect()
}

fn legacy_count_checks(runtime: &ClusterRuntime, request_body: &[u8]) -> Result<Vec<Check>> {
    let space = legacy_space(runtime)?;

    if let Ok(checks) = decode_protocol_count_request(request_body) {
        return legacy_checks_from_protocol(&space, &checks);
    }

    let request = CountRequest::decode_body(request_body)?;
    if request.space != space.name {
        anyhow::bail!(
            "legacy count request targeted unknown space `{}`; runtime only serves `{}`",
            request.space,
            space.name
        );
    }

    Ok(Vec::new())
}

async fn legacy_record_exists(runtime: &ClusterRuntime, space: &str, key: &[u8]) -> Result<bool> {
    let response = HyperdexClientService::handle(
        runtime,
        ClientRequest::Get {
            space: space.to_owned(),
            key: key.to_vec().into(),
        },
    )
    .await?;

    let ClientResponse::Record(record) = response else {
        anyhow::bail!("unexpected runtime response to existence check");
    };

    Ok(record.is_some())
}

fn legacy_decode_request_nonce(bytes: &[u8]) -> Result<(u64, &[u8])> {
    if bytes.len() < std::mem::size_of::<u64>() {
        anyhow::bail!("legacy request body is missing nonce");
    }

    let nonce = decode_be_u64_exact(&bytes[..std::mem::size_of::<u64>()], "legacy request nonce")?;
    Ok((nonce, &bytes[std::mem::size_of::<u64>()..]))
}

fn legacy_search_response(
    runtime: &ClusterRuntime,
    space: &Space,
    header: RequestHeader,
    nonce: u64,
    search_id: u64,
) -> Result<(ResponseHeader, Vec<u8>)> {
    let mut searches = runtime.legacy_searches_guard()?;
    let Some(state) = searches.get_mut(&search_id) else {
        return Ok((
            ResponseHeader {
                message_type: LegacyMessageType::RespSearchDone,
                target_virtual_server: header.target_virtual_server,
                nonce,
            },
            SearchDoneResponse { search_id }.encode_body().to_vec(),
        ));
    };

    let response = if let Some(record) = state.records.pop_front() {
        let body = match state.format {
            LegacyBodyFormat::Protocol => encode_protocol_search_item(&ProtocolSearchItem {
                key: record.key.to_vec(),
                values: legacy_protocol_values_from_record(space, &record)?,
            }),
            LegacyBodyFormat::Named => SearchItemResponse {
                search_id,
                key: record.key.to_vec(),
                attributes: legacy_named_attributes_from_record(space, &record)?,
            }
            .encode_body(),
        };
        (
            ResponseHeader {
                message_type: LegacyMessageType::RespSearchItem,
                target_virtual_server: header.target_virtual_server,
                nonce,
            },
            body,
        )
    } else {
        (
            ResponseHeader {
                message_type: LegacyMessageType::RespSearchDone,
                target_virtual_server: header.target_virtual_server,
                nonce,
            },
            SearchDoneResponse { search_id }.encode_body().to_vec(),
        )
    };

    if state.records.is_empty() {
        searches.remove(&search_id);
    }

    Ok(response)
}

fn legacy_atomic_status(response: ClientResponse) -> Result<LegacyReturnCode> {
    match response {
        ClientResponse::Unit => Ok(LegacyReturnCode::Success),
        ClientResponse::ConditionFailed => Ok(LegacyReturnCode::CompareFailed),
        other => anyhow::bail!("unexpected runtime response to atomic request: {other:?}"),
    }
}

fn legacy_validate_atomic_request(space: &Space, request: &ProtocolKeyChange) -> Result<()> {
    if request.erase && !request.funcalls.is_empty() {
        anyhow::bail!("legacy erase requests may not carry funcalls");
    }

    for check in &request.checks {
        let (_, attribute_kind) = legacy_non_key_attribute(space, check.attr)?;
        let expected = legacy_hyperdatatype(attribute_kind)?;
        if check.datatype != expected {
            anyhow::bail!(
                "legacy check datatype {} does not match schema datatype {} for attr {}",
                check.datatype,
                expected,
                check.attr
            );
        }
        let _ = legacy_protocol_predicate(check.predicate)?;
        let _ = legacy_value_from_protocol(check.datatype, &check.value)?;
    }

    for funcall in &request.funcalls {
        legacy_validate_protocol_funcall(space, funcall)?;
    }

    Ok(())
}

fn legacy_validate_protocol_funcall(space: &Space, funcall: &ProtocolFuncall) -> Result<()> {
    let (_, attribute_kind) = legacy_non_key_attribute(space, funcall.attr)?;

    match funcall.name {
        FUNC_SET => {
            let expected = legacy_hyperdatatype(attribute_kind)?;
            if funcall.arg1_datatype != expected {
                anyhow::bail!(
                    "legacy set datatype {} does not match schema datatype {} for attr {}",
                    funcall.arg1_datatype,
                    expected,
                    funcall.attr
                );
            }
            if !funcall.arg2.is_empty() || funcall.arg2_datatype != 0 {
                anyhow::bail!("legacy set funcalls may not carry arg2");
            }
            let _ = legacy_value_from_protocol(funcall.arg1_datatype, &funcall.arg1)?;
        }
        FUNC_NUM_ADD | FUNC_NUM_SUB | FUNC_NUM_MUL | FUNC_NUM_DIV | FUNC_NUM_MOD | FUNC_NUM_AND
        | FUNC_NUM_OR | FUNC_NUM_XOR | FUNC_NUM_MAX | FUNC_NUM_MIN => {
            legacy_validate_numeric_protocol_funcall(funcall, attribute_kind)?;
        }
        FUNC_STRING_APPEND | FUNC_STRING_PREPEND | FUNC_STRING_LTRIM | FUNC_STRING_RTRIM => {
            match attribute_kind {
                ValueKind::Bytes | ValueKind::String | ValueKind::Document => {
                    let expected = legacy_hyperdatatype(attribute_kind)?;
                    if funcall.arg1_datatype != expected {
                        anyhow::bail!(
                            "legacy string funcall datatype {} does not match schema datatype {} for attr {}",
                            funcall.arg1_datatype,
                            expected,
                            funcall.attr
                        );
                    }
                    if !funcall.arg2.is_empty() || funcall.arg2_datatype != 0 {
                        anyhow::bail!("legacy string funcalls may not carry arg2");
                    }
                    let _ = legacy_value_from_protocol(funcall.arg1_datatype, &funcall.arg1)?;
                }
                ValueKind::Map { key, value } => {
                    match value.as_ref() {
                        ValueKind::Bytes | ValueKind::String | ValueKind::Document => {}
                        other => anyhow::bail!(
                            "legacy string funcall {} targets map attr with non-string values {:?}",
                            funcall.name,
                            other
                        ),
                    }
                    let expected_key = legacy_hyperdatatype(key.as_ref())?;
                    let expected_value = legacy_hyperdatatype(value.as_ref())?;
                    if funcall.arg1_datatype != expected_value {
                        anyhow::bail!(
                            "legacy map string value datatype {} does not match schema datatype {} for attr {}",
                            funcall.arg1_datatype,
                            expected_value,
                            funcall.attr
                        );
                    }
                    if funcall.arg2_datatype != expected_key {
                        anyhow::bail!(
                            "legacy map string key datatype {} does not match schema datatype {} for attr {}",
                            funcall.arg2_datatype,
                            expected_key,
                            funcall.attr
                        );
                    }
                    let _ = legacy_value_from_protocol(funcall.arg1_datatype, &funcall.arg1)?;
                    let _ = legacy_value_from_protocol(funcall.arg2_datatype, &funcall.arg2)?;
                }
                other => anyhow::bail!(
                    "legacy string funcall {} targets non-string attr {:?}",
                    funcall.name,
                    other
                ),
            }
        }
        FUNC_LIST_LPUSH | FUNC_LIST_RPUSH => {
            let elem_kind = match attribute_kind {
                ValueKind::List(elem_kind) => elem_kind.as_ref(),
                other => anyhow::bail!("legacy list funcall targets non-list attr {other:?}"),
            };
            let expected = legacy_hyperdatatype(elem_kind)?;
            if funcall.arg1_datatype != expected {
                anyhow::bail!(
                    "legacy list funcall datatype {} does not match element datatype {} for attr {}",
                    funcall.arg1_datatype,
                    expected,
                    funcall.attr
                );
            }
            if !funcall.arg2.is_empty() || funcall.arg2_datatype != 0 {
                anyhow::bail!("legacy list funcalls may not carry arg2");
            }
            let _ = legacy_value_from_protocol(funcall.arg1_datatype, &funcall.arg1)?;
        }
        FUNC_SET_ADD | FUNC_SET_REMOVE => {
            let elem_kind = match attribute_kind {
                ValueKind::Set(elem_kind) => elem_kind.as_ref(),
                other => anyhow::bail!("legacy set funcall targets non-set attr {other:?}"),
            };
            let expected = legacy_hyperdatatype(elem_kind)?;
            if funcall.arg1_datatype != expected {
                anyhow::bail!(
                    "legacy set funcall datatype {} does not match element datatype {} for attr {}",
                    funcall.arg1_datatype,
                    expected,
                    funcall.attr
                );
            }
            if !funcall.arg2.is_empty() || funcall.arg2_datatype != 0 {
                anyhow::bail!("legacy set funcalls may not carry arg2");
            }
            let _ = legacy_value_from_protocol(funcall.arg1_datatype, &funcall.arg1)?;
        }
        FUNC_SET_INTERSECT | FUNC_SET_UNION => {
            let expected = legacy_hyperdatatype(attribute_kind)?;
            if funcall.arg1_datatype != expected {
                anyhow::bail!(
                    "legacy set merge datatype {} does not match schema datatype {} for attr {}",
                    funcall.arg1_datatype,
                    expected,
                    funcall.attr
                );
            }
            if !funcall.arg2.is_empty() || funcall.arg2_datatype != 0 {
                anyhow::bail!("legacy set merge funcalls may not carry arg2");
            }
            let _ = legacy_value_from_protocol(funcall.arg1_datatype, &funcall.arg1)?;
        }
        FUNC_MAP_ADD => {
            let (key_kind, value_kind) = match attribute_kind {
                ValueKind::Map { key, value } => (key.as_ref(), value.as_ref()),
                other => anyhow::bail!("legacy map add targets non-map attr {other:?}"),
            };
            let expected_key = legacy_hyperdatatype(key_kind)?;
            let expected_value = legacy_hyperdatatype(value_kind)?;
            if funcall.arg1_datatype != expected_value {
                anyhow::bail!(
                    "legacy map add value datatype {} does not match schema datatype {} for attr {}",
                    funcall.arg1_datatype,
                    expected_value,
                    funcall.attr
                );
            }
            if funcall.arg2_datatype != expected_key {
                anyhow::bail!(
                    "legacy map add key datatype {} does not match schema datatype {} for attr {}",
                    funcall.arg2_datatype,
                    expected_key,
                    funcall.attr
                );
            }
            let _ = legacy_value_from_protocol(funcall.arg1_datatype, &funcall.arg1)?;
            let _ = legacy_value_from_protocol(funcall.arg2_datatype, &funcall.arg2)?;
        }
        FUNC_MAP_REMOVE => {
            let key_kind = match attribute_kind {
                ValueKind::Map { key, .. } => key.as_ref(),
                other => anyhow::bail!("legacy map remove targets non-map attr {other:?}"),
            };
            let expected_key = legacy_hyperdatatype(key_kind)?;
            if funcall.arg2_datatype != expected_key {
                anyhow::bail!(
                    "legacy map remove key datatype {} does not match schema datatype {} for attr {}",
                    funcall.arg2_datatype,
                    expected_key,
                    funcall.attr
                );
            }
            if !funcall.arg1.is_empty() || funcall.arg1_datatype != 0 {
                anyhow::bail!("legacy map remove may not carry arg1");
            }
            let _ = legacy_value_from_protocol(funcall.arg2_datatype, &funcall.arg2)?;
        }
        other => anyhow::bail!("legacy funcall {other} is not implemented"),
    }

    Ok(())
}

fn legacy_non_key_attribute<'a>(space: &'a Space, attr: u16) -> Result<(&'a str, &'a ValueKind)> {
    if attr == 0 {
        return Ok((&space.key_attribute, &ValueKind::Bytes));
    }

    let index = usize::from(attr - 1);
    let attribute = space
        .attributes
        .get(index)
        .ok_or_else(|| anyhow!("legacy attribute index {attr} exceeds schema width"))?;
    Ok((&attribute.name, &attribute.kind))
}

fn legacy_checks_from_protocol(
    space: &Space,
    checks: &[ProtocolAttributeCheck],
) -> Result<Vec<Check>> {
    checks
        .iter()
        .map(|check| {
            let (attribute, _) = legacy_non_key_attribute(space, check.attr)?;
            Ok(Check {
                attribute: attribute.to_owned(),
                predicate: legacy_protocol_predicate(check.predicate)?,
                value: legacy_value_from_protocol(check.datatype, &check.value)?,
            })
        })
        .collect()
}

fn legacy_protocol_predicate(predicate: u16) -> Result<Predicate> {
    match predicate {
        HYPERPREDICATE_EQUALS => Ok(Predicate::Equal),
        HYPERPREDICATE_LESS_THAN => Ok(Predicate::LessThan),
        HYPERPREDICATE_LESS_EQUAL => Ok(Predicate::LessThanOrEqual),
        HYPERPREDICATE_GREATER_EQUAL => Ok(Predicate::GreaterThanOrEqual),
        HYPERPREDICATE_GREATER_THAN => Ok(Predicate::GreaterThan),
        other => Err(anyhow!("legacy predicate {other} is not implemented")),
    }
}

fn legacy_atomic_can_use_runtime_mutations(space: &Space, funcalls: &[ProtocolFuncall]) -> bool {
    funcalls.iter().all(|funcall| {
        if !funcall.arg2.is_empty() {
            return false;
        }

        match funcall.name {
            FUNC_SET => true,
            FUNC_NUM_ADD | FUNC_NUM_SUB | FUNC_NUM_MUL | FUNC_NUM_AND | FUNC_NUM_OR
            | FUNC_NUM_XOR => legacy_non_key_attribute(space, funcall.attr)
                .map(|(_, kind)| matches!(kind, ValueKind::Int))
                .unwrap_or(false),
            _ => false,
        }
    })
}

fn legacy_validate_numeric_protocol_funcall(
    funcall: &ProtocolFuncall,
    attribute_kind: &ValueKind,
) -> Result<()> {
    match attribute_kind {
        ValueKind::Int => {
            legacy_validate_numeric_operand(funcall, attribute_kind)?;
            if !funcall.arg2.is_empty() || funcall.arg2_datatype != 0 {
                anyhow::bail!("legacy scalar numeric funcalls may not carry arg2");
            }
        }
        ValueKind::Float => {
            legacy_validate_numeric_operand(funcall, attribute_kind)?;
            if !funcall.arg2.is_empty() || funcall.arg2_datatype != 0 {
                anyhow::bail!("legacy scalar numeric funcalls may not carry arg2");
            }
            legacy_validate_float_numeric_name(funcall.name)?;
        }
        ValueKind::Map { key, value } => {
            let expected_key = legacy_hyperdatatype(key.as_ref())?;
            if funcall.arg2_datatype != expected_key {
                anyhow::bail!(
                    "legacy numeric map key datatype {} does not match schema datatype {} for attr {}",
                    funcall.arg2_datatype,
                    expected_key,
                    funcall.attr
                );
            }
            let _ = legacy_value_from_protocol(funcall.arg2_datatype, &funcall.arg2)?;
            legacy_validate_numeric_operand(funcall, value.as_ref())?;
            if matches!(value.as_ref(), ValueKind::Float) {
                legacy_validate_float_numeric_name(funcall.name)?;
            }
        }
        other => anyhow::bail!(
            "legacy numeric funcall {} targets non-numeric attr {:?}",
            funcall.name,
            other
        ),
    }

    Ok(())
}

fn legacy_validate_numeric_operand(
    funcall: &ProtocolFuncall,
    target_kind: &ValueKind,
) -> Result<()> {
    match target_kind {
        ValueKind::Int => match funcall.arg1_datatype {
            HYPERDATATYPE_INT64 => {
                let _ = legacy_decode_i64(&funcall.arg1)?;
            }
            HYPERDATATYPE_FLOAT => {
                let _ = legacy_decode_f64(&funcall.arg1)?;
            }
            other => {
                anyhow::bail!(
                    "legacy numeric datatype {} does not match schema datatype {} for attr {}",
                    other,
                    HYPERDATATYPE_INT64,
                    funcall.attr
                );
            }
        },
        ValueKind::Float => match funcall.arg1_datatype {
            HYPERDATATYPE_INT64 => {
                let _ = legacy_decode_i64(&funcall.arg1)?;
            }
            HYPERDATATYPE_FLOAT => {
                let _ = legacy_decode_f64(&funcall.arg1)?;
            }
            other => {
                anyhow::bail!(
                    "legacy numeric datatype {} does not match schema datatype {} for attr {}",
                    other,
                    HYPERDATATYPE_FLOAT,
                    funcall.attr
                );
            }
        },
        other => anyhow::bail!("legacy numeric funcall target is not numeric: {other:?}"),
    }

    Ok(())
}

fn legacy_validate_float_numeric_name(name: u8) -> Result<()> {
    match name {
        FUNC_NUM_ADD | FUNC_NUM_SUB | FUNC_NUM_MUL | FUNC_NUM_DIV | FUNC_NUM_MAX | FUNC_NUM_MIN => {
            Ok(())
        }
        other => anyhow::bail!("legacy float funcall {other} is not implemented"),
    }
}

fn legacy_mutations_from_protocol_funcalls(
    space: &Space,
    funcalls: &[ProtocolFuncall],
) -> Result<Vec<Mutation>> {
    funcalls
        .iter()
        .map(|funcall| match funcall.name {
            FUNC_SET => {
                let (attribute, _) = legacy_non_key_attribute(space, funcall.attr)?;
                Ok(Mutation::Set(Attribute {
                    name: attribute.to_owned(),
                    value: legacy_value_from_protocol(funcall.arg1_datatype, &funcall.arg1)?,
                }))
            }
            FUNC_NUM_ADD | FUNC_NUM_SUB | FUNC_NUM_MUL | FUNC_NUM_DIV | FUNC_NUM_MOD
            | FUNC_NUM_AND | FUNC_NUM_OR | FUNC_NUM_XOR => {
                let (attribute, _) = legacy_non_key_attribute(space, funcall.attr)?;
                Ok(Mutation::Numeric {
                    attribute: attribute.to_owned(),
                    op: legacy_numeric_op(funcall.name)?,
                    operand: legacy_decode_i64(&funcall.arg1)?,
                })
            }
            other => anyhow::bail!("legacy funcall {other} cannot use direct runtime mutations"),
        })
        .collect()
}

fn legacy_numeric_op(name: u8) -> Result<NumericOp> {
    match name {
        FUNC_NUM_ADD => Ok(NumericOp::Add),
        FUNC_NUM_SUB => Ok(NumericOp::Sub),
        FUNC_NUM_MUL => Ok(NumericOp::Mul),
        FUNC_NUM_DIV => Ok(NumericOp::Div),
        FUNC_NUM_MOD => Ok(NumericOp::Mod),
        FUNC_NUM_AND => Ok(NumericOp::And),
        FUNC_NUM_OR => Ok(NumericOp::Or),
        FUNC_NUM_XOR => Ok(NumericOp::Xor),
        other => Err(anyhow!("legacy numeric funcall {other} is not implemented")),
    }
}

async fn legacy_apply_atomic_direct(
    runtime: &ClusterRuntime,
    space: &Space,
    request: ProtocolKeyChange,
    checks: Vec<Check>,
) -> Result<ClientResponse> {
    let existing = HyperdexClientService::handle(
        runtime,
        ClientRequest::Get {
            space: space.name.clone(),
            key: request.key.clone().into(),
        },
    )
    .await?;

    let ClientResponse::Record(record) = existing else {
        anyhow::bail!("unexpected runtime response to atomic prefetch");
    };

    let mut record = record.unwrap_or_else(|| Record::new(request.key.clone().into()));

    for funcall in &request.funcalls {
        legacy_apply_protocol_funcall(space, &mut record, funcall)?;
    }

    let mutations = record
        .attributes
        .into_iter()
        .map(|(name, value)| Mutation::Set(Attribute { name, value }))
        .collect::<Vec<_>>();

    if checks.is_empty() {
        HyperdexClientService::handle(
            runtime,
            ClientRequest::Put {
                space: space.name.clone(),
                key: request.key.into(),
                mutations,
            },
        )
        .await
    } else {
        HyperdexClientService::handle(
            runtime,
            ClientRequest::ConditionalPut {
                space: space.name.clone(),
                key: request.key.into(),
                checks,
                mutations,
            },
        )
        .await
    }
}

fn legacy_apply_protocol_funcall(
    space: &Space,
    record: &mut Record,
    funcall: &ProtocolFuncall,
) -> Result<()> {
    let (attribute_name, attribute_kind) = legacy_non_key_attribute(space, funcall.attr)?;

    match funcall.name {
        FUNC_SET => {
            record.attributes.insert(
                attribute_name.to_owned(),
                legacy_value_from_protocol(funcall.arg1_datatype, &funcall.arg1)?,
            );
        }
        FUNC_STRING_APPEND | FUNC_STRING_PREPEND | FUNC_STRING_LTRIM | FUNC_STRING_RTRIM => {
            match attribute_kind {
                ValueKind::Bytes | ValueKind::String | ValueKind::Document => {
                    let current = legacy_existing_bytes(record, attribute_name);
                    let updated = legacy_apply_string_bytes(funcall.name, current, &funcall.arg1)?;
                    record
                        .attributes
                        .insert(attribute_name.to_owned(), Value::Bytes(updated.into()));
                }
                ValueKind::Map { key, value } => {
                    match value.as_ref() {
                        ValueKind::Bytes | ValueKind::String | ValueKind::Document => {}
                        other => anyhow::bail!(
                            "expected string-valued map for {attribute_name}, got {:?}",
                            other
                        ),
                    }
                    let mut map = match record.attributes.remove(attribute_name) {
                        Some(Value::Map(values)) => values,
                        Some(other) => {
                            anyhow::bail!("expected map attribute {attribute_name}, got {other:?}")
                        }
                        None => Default::default(),
                    };
                    let map_key = legacy_value_from_kind_bytes(key.as_ref(), &funcall.arg2)?;
                    let current = map
                        .remove(&map_key)
                        .map(|value| legacy_value_as_bytes(&value))
                        .transpose()?
                        .unwrap_or_default();
                    let operand = legacy_value_as_bytes(&legacy_value_from_kind_bytes(
                        value.as_ref(),
                        &funcall.arg1,
                    )?)?;
                    let updated = legacy_apply_string_bytes(funcall.name, current, &operand)?;
                    map.insert(map_key, Value::Bytes(updated.into()));
                    record
                        .attributes
                        .insert(attribute_name.to_owned(), Value::Map(map));
                }
                other => anyhow::bail!(
                    "legacy string funcall {} targets non-string attr {:?}",
                    funcall.name,
                    other
                ),
            }
        }
        FUNC_NUM_ADD | FUNC_NUM_SUB | FUNC_NUM_MUL | FUNC_NUM_DIV | FUNC_NUM_MOD | FUNC_NUM_AND
        | FUNC_NUM_OR | FUNC_NUM_XOR | FUNC_NUM_MAX | FUNC_NUM_MIN => match attribute_kind {
            ValueKind::Float => {
                let current = legacy_existing_f64(record, attribute_name)?;
                let operand = legacy_numeric_operand_f64(funcall)?;
                let updated = match funcall.name {
                    FUNC_NUM_ADD => current + operand,
                    FUNC_NUM_SUB => current - operand,
                    FUNC_NUM_MUL => current * operand,
                    FUNC_NUM_DIV => current / operand,
                    FUNC_NUM_MAX => current.max(operand),
                    FUNC_NUM_MIN => current.min(operand),
                    other => anyhow::bail!("legacy float funcall {other} is not implemented"),
                };
                record
                    .attributes
                    .insert(attribute_name.to_owned(), Value::Float(updated.into()));
            }
            ValueKind::Int => {
                let current = legacy_existing_i64(record, attribute_name)?;
                let operand = legacy_numeric_operand_i64(funcall)?;
                let updated = legacy_apply_i64_numeric_funcall(funcall.name, current, operand)?;
                record
                    .attributes
                    .insert(attribute_name.to_owned(), Value::Int(updated));
            }
            ValueKind::Map { key, value } => {
                let mut map = match record.attributes.remove(attribute_name) {
                    Some(Value::Map(values)) => values,
                    Some(other) => {
                        anyhow::bail!("expected map attribute {attribute_name}, got {other:?}")
                    }
                    None => Default::default(),
                };
                let map_key = legacy_value_from_kind_bytes(key.as_ref(), &funcall.arg2)?;
                let updated_value = match value.as_ref() {
                    ValueKind::Float => {
                        let current =
                            legacy_existing_map_entry_f64(&map, &map_key, attribute_name)?;
                        let operand = legacy_numeric_operand_f64(funcall)?;
                        let updated = match funcall.name {
                            FUNC_NUM_ADD => current + operand,
                            FUNC_NUM_SUB => current - operand,
                            FUNC_NUM_MUL => current * operand,
                            FUNC_NUM_DIV => current / operand,
                            FUNC_NUM_MAX => current.max(operand),
                            FUNC_NUM_MIN => current.min(operand),
                            other => {
                                anyhow::bail!("legacy float funcall {other} is not implemented")
                            }
                        };
                        Value::Float(updated.into())
                    }
                    ValueKind::Int => {
                        let current =
                            legacy_existing_map_entry_i64(&map, &map_key, attribute_name)?;
                        let operand = legacy_numeric_operand_i64(funcall)?;
                        let updated =
                            legacy_apply_i64_numeric_funcall(funcall.name, current, operand)?;
                        Value::Int(updated)
                    }
                    other => anyhow::bail!(
                        "legacy numeric funcall {} targets non-numeric map values {:?}",
                        funcall.name,
                        other
                    ),
                };
                map.insert(map_key, updated_value);
                record
                    .attributes
                    .insert(attribute_name.to_owned(), Value::Map(map));
            }
            other => anyhow::bail!(
                "legacy numeric funcall {} targets non-numeric attr {:?}",
                funcall.name,
                other
            ),
        },
        FUNC_LIST_LPUSH | FUNC_LIST_RPUSH => {
            let mut list = match record.attributes.remove(attribute_name) {
                Some(Value::List(values)) => values,
                Some(other) => {
                    anyhow::bail!("expected list attribute {attribute_name}, got {other:?}")
                }
                None => Vec::new(),
            };
            let elem_kind = match attribute_kind {
                ValueKind::List(elem_kind) => elem_kind.as_ref(),
                other => anyhow::bail!("expected list kind for {attribute_name}, got {other:?}"),
            };
            let value = legacy_value_from_kind_bytes(elem_kind, &funcall.arg1)?;
            if funcall.name == FUNC_LIST_LPUSH {
                list.insert(0, value);
            } else {
                list.push(value);
            }
            record
                .attributes
                .insert(attribute_name.to_owned(), Value::List(list));
        }
        FUNC_SET_ADD | FUNC_SET_REMOVE => {
            let mut set = match record.attributes.remove(attribute_name) {
                Some(Value::Set(values)) => values,
                Some(other) => {
                    anyhow::bail!("expected set attribute {attribute_name}, got {other:?}")
                }
                None => Default::default(),
            };
            let elem_kind = match attribute_kind {
                ValueKind::Set(elem_kind) => elem_kind.as_ref(),
                other => anyhow::bail!("expected set kind for {attribute_name}, got {other:?}"),
            };
            let value = legacy_value_from_kind_bytes(elem_kind, &funcall.arg1)?;
            if funcall.name == FUNC_SET_ADD {
                set.insert(value);
            } else {
                set.remove(&value);
            }
            record
                .attributes
                .insert(attribute_name.to_owned(), Value::Set(set));
        }
        FUNC_SET_INTERSECT | FUNC_SET_UNION => {
            let current = match record.attributes.remove(attribute_name) {
                Some(Value::Set(values)) => values,
                Some(other) => {
                    anyhow::bail!("expected set attribute {attribute_name}, got {other:?}")
                }
                None => Default::default(),
            };
            let operand = match legacy_value_from_protocol(funcall.arg1_datatype, &funcall.arg1)? {
                Value::Set(values) => values,
                other => anyhow::bail!("expected set operand for {attribute_name}, got {other:?}"),
            };
            let updated = if funcall.name == FUNC_SET_INTERSECT {
                current.intersection(&operand).cloned().collect()
            } else {
                current.union(&operand).cloned().collect()
            };
            record
                .attributes
                .insert(attribute_name.to_owned(), Value::Set(updated));
        }
        FUNC_MAP_ADD => {
            let mut map = match record.attributes.remove(attribute_name) {
                Some(Value::Map(values)) => values,
                Some(other) => {
                    anyhow::bail!("expected map attribute {attribute_name}, got {other:?}")
                }
                None => Default::default(),
            };
            let (key_kind, value_kind) = match attribute_kind {
                ValueKind::Map { key, value } => (key.as_ref(), value.as_ref()),
                other => anyhow::bail!("expected map kind for {attribute_name}, got {other:?}"),
            };
            let map_key = legacy_value_from_kind_bytes(key_kind, &funcall.arg2)?;
            let map_value = legacy_value_from_kind_bytes(value_kind, &funcall.arg1)?;
            map.insert(map_key, map_value);
            record
                .attributes
                .insert(attribute_name.to_owned(), Value::Map(map));
        }
        FUNC_MAP_REMOVE => {
            let mut map = match record.attributes.remove(attribute_name) {
                Some(Value::Map(values)) => values,
                Some(other) => {
                    anyhow::bail!("expected map attribute {attribute_name}, got {other:?}")
                }
                None => Default::default(),
            };
            let key_kind = match attribute_kind {
                ValueKind::Map { key, .. } => key.as_ref(),
                other => anyhow::bail!("expected map kind for {attribute_name}, got {other:?}"),
            };
            let map_key = legacy_value_from_kind_bytes(key_kind, &funcall.arg2)?;
            map.remove(&map_key);
            record
                .attributes
                .insert(attribute_name.to_owned(), Value::Map(map));
        }
        other => anyhow::bail!("legacy funcall {other} is not implemented"),
    }

    Ok(())
}

fn legacy_protocol_values_from_record(space: &Space, record: &Record) -> Result<Vec<Vec<u8>>> {
    space
        .attributes
        .iter()
        .map(|attribute| {
            let value = legacy_record_value(record, attribute)?;
            legacy_protocol_value_from_kind(&attribute.kind, value.as_ref())
        })
        .collect()
}

fn legacy_protocol_value_from_kind(kind: &ValueKind, value: &Value) -> Result<Vec<u8>> {
    match kind {
        ValueKind::Bytes | ValueKind::String | ValueKind::Document => legacy_value_as_bytes(value),
        ValueKind::Int => match value {
            Value::Int(number) => Ok(number.to_le_bytes().to_vec()),
            other => Err(anyhow!("cannot encode {other:?} as legacy int")),
        },
        ValueKind::Float => match value {
            Value::Float(number) => Ok(number.into_inner().to_le_bytes().to_vec()),
            other => Err(anyhow!("cannot encode {other:?} as legacy float")),
        },
        ValueKind::List(elem_kind) => {
            let Value::List(values) = value else {
                return Err(anyhow!("cannot encode {value:?} as legacy list"));
            };
            let mut out = Vec::new();
            for value in values {
                out.extend_from_slice(&legacy_encode_container_value(elem_kind, value)?);
            }
            Ok(out)
        }
        ValueKind::Set(elem_kind) => {
            let Value::Set(values) = value else {
                return Err(anyhow!("cannot encode {value:?} as legacy set"));
            };
            let mut out = Vec::new();
            for value in values {
                out.extend_from_slice(&legacy_encode_container_value(elem_kind, value)?);
            }
            Ok(out)
        }
        ValueKind::Map {
            key,
            value: map_value,
        } => {
            let Value::Map(values) = value else {
                return Err(anyhow!("cannot encode {value:?} as legacy map"));
            };
            let mut out = Vec::new();
            for (map_key, map_value_item) in values {
                out.extend_from_slice(&legacy_encode_container_value(key, map_key)?);
                out.extend_from_slice(&legacy_encode_container_value(map_value, map_value_item)?);
            }
            Ok(out)
        }
        ValueKind::Bool | ValueKind::Timestamp(_) => Err(anyhow!(
            "legacy daemon protocol does not support {kind:?} yet"
        )),
    }
}

fn legacy_value_from_protocol(datatype: u16, bytes: &[u8]) -> Result<Value> {
    let kind = legacy_kind_from_protocol_datatype(datatype)?;
    legacy_value_from_kind_bytes(&kind, bytes)
}

fn legacy_value_from_kind_bytes(kind: &ValueKind, bytes: &[u8]) -> Result<Value> {
    match kind {
        ValueKind::Bytes | ValueKind::String | ValueKind::Document => {
            Ok(Value::Bytes(bytes::Bytes::copy_from_slice(bytes)))
        }
        ValueKind::Int => Ok(Value::Int(legacy_decode_i64(bytes)?)),
        ValueKind::Float => Ok(Value::Float(legacy_decode_f64(bytes)?.into())),
        ValueKind::List(elem_kind) => {
            let mut offset = 0;
            let mut values = Vec::new();
            while offset < bytes.len() {
                let (value, consumed) = legacy_decode_container_value(elem_kind, &bytes[offset..])?;
                values.push(value);
                offset += consumed;
            }
            Ok(Value::List(values))
        }
        ValueKind::Set(elem_kind) => {
            let mut offset = 0;
            let mut values = std::collections::BTreeSet::new();
            while offset < bytes.len() {
                let (value, consumed) = legacy_decode_container_value(elem_kind, &bytes[offset..])?;
                values.insert(value);
                offset += consumed;
            }
            Ok(Value::Set(values))
        }
        ValueKind::Map { key, value } => {
            let mut offset = 0;
            let mut map = BTreeMap::new();
            while offset < bytes.len() {
                let (map_key, key_size) = legacy_decode_container_value(key, &bytes[offset..])?;
                offset += key_size;
                let (map_value, value_size) =
                    legacy_decode_container_value(value, &bytes[offset..])?;
                offset += value_size;
                map.insert(map_key, map_value);
            }
            Ok(Value::Map(map))
        }
        ValueKind::Bool | ValueKind::Timestamp(_) => Err(anyhow!(
            "legacy daemon protocol does not support {kind:?} yet"
        )),
    }
}

fn legacy_kind_from_protocol_datatype(datatype: u16) -> Result<ValueKind> {
    match datatype {
        HYPERDATATYPE_STRING => Ok(ValueKind::String),
        HYPERDATATYPE_INT64 => Ok(ValueKind::Int),
        HYPERDATATYPE_FLOAT => Ok(ValueKind::Float),
        _ if datatype & !0x003f == HYPERDATATYPE_LIST_GENERIC => Ok(ValueKind::List(Box::new(
            legacy_primitive_kind(datatype & 0x003f)?,
        ))),
        _ if datatype & !0x003f == HYPERDATATYPE_SET_GENERIC => Ok(ValueKind::Set(Box::new(
            legacy_primitive_kind(datatype & 0x003f)?,
        ))),
        _ if datatype & !0x003f == HYPERDATATYPE_MAP_GENERIC => Ok(ValueKind::Map {
            key: Box::new(legacy_primitive_kind((datatype >> 3) & 0x0007)?),
            value: Box::new(legacy_primitive_kind(datatype & 0x0007)?),
        }),
        other => Err(anyhow!("legacy datatype {other} is not implemented")),
    }
}

fn legacy_primitive_kind(code: u16) -> Result<ValueKind> {
    match code {
        0 => Ok(ValueKind::Bytes),
        1 => Ok(ValueKind::String),
        2 => Ok(ValueKind::Int),
        3 => Ok(ValueKind::Float),
        other => Err(anyhow!(
            "legacy primitive datatype code {other} is not implemented"
        )),
    }
}

fn legacy_decode_container_value(kind: &ValueKind, bytes: &[u8]) -> Result<(Value, usize)> {
    match kind {
        ValueKind::Bytes | ValueKind::String | ValueKind::Document => {
            if bytes.len() < 4 {
                anyhow::bail!("legacy container string element is truncated");
            }
            let len = decode_le_u32_exact(&bytes[..4], "legacy container string length")? as usize;
            if bytes.len() < 4 + len {
                anyhow::bail!("legacy container string element is truncated");
            }
            Ok((
                Value::Bytes(bytes::Bytes::copy_from_slice(&bytes[4..4 + len])),
                4 + len,
            ))
        }
        ValueKind::Int => {
            if bytes.len() < 8 {
                anyhow::bail!("legacy int element is truncated");
            }
            Ok((Value::Int(legacy_decode_i64(bytes)?), 8))
        }
        ValueKind::Float => {
            if bytes.len() < 8 {
                anyhow::bail!("legacy float element is truncated");
            }
            Ok((Value::Float(legacy_decode_f64(bytes)?.into()), 8))
        }
        other => Err(anyhow!(
            "legacy container element kind {other:?} is not supported"
        )),
    }
}

fn legacy_encode_container_value(kind: &ValueKind, value: &Value) -> Result<Vec<u8>> {
    match kind {
        ValueKind::Bytes | ValueKind::String | ValueKind::Document => {
            let bytes = legacy_value_as_bytes(value)?;
            let len = u32::try_from(bytes.len())
                .map_err(|_| anyhow!("legacy string element exceeds u32"))?;
            let mut out = Vec::with_capacity(4 + bytes.len());
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&bytes);
            Ok(out)
        }
        ValueKind::Int => match value {
            Value::Int(number) => Ok(number.to_le_bytes().to_vec()),
            other => Err(anyhow!("cannot encode {other:?} as legacy int element")),
        },
        ValueKind::Float => match value {
            Value::Float(number) => Ok(number.into_inner().to_le_bytes().to_vec()),
            other => Err(anyhow!("cannot encode {other:?} as legacy float element")),
        },
        other => Err(anyhow!(
            "legacy container element kind {other:?} is not supported"
        )),
    }
}

fn legacy_existing_bytes(record: &Record, attribute: &str) -> Vec<u8> {
    match record.attributes.get(attribute) {
        Some(Value::Bytes(bytes)) => bytes.to_vec(),
        Some(Value::String(text)) => text.as_bytes().to_vec(),
        Some(_) | None => Vec::new(),
    }
}

fn legacy_apply_string_bytes(name: u8, current: Vec<u8>, operand: &[u8]) -> Result<Vec<u8>> {
    Ok(match name {
        FUNC_STRING_APPEND => [current, operand.to_vec()].concat(),
        FUNC_STRING_PREPEND => [operand.to_vec(), current].concat(),
        FUNC_STRING_LTRIM => current
            .strip_prefix(operand)
            .map_or(current.clone(), Vec::from),
        FUNC_STRING_RTRIM => current
            .strip_suffix(operand)
            .map_or(current.clone(), Vec::from),
        other => anyhow::bail!("legacy string funcall {other} is not implemented"),
    })
}

fn legacy_div_i64(current: i64, operand: i64) -> Result<i64> {
    Ok(legacy_signed_div_mod_i64(current, operand)?.0)
}

fn legacy_mod_i64(current: i64, operand: i64) -> Result<i64> {
    Ok(legacy_signed_div_mod_i64(current, operand)?.1)
}

fn legacy_apply_i64_numeric_funcall(name: u8, current: i64, operand: i64) -> Result<i64> {
    match name {
        FUNC_NUM_ADD => Ok(current.saturating_add(operand)),
        FUNC_NUM_SUB => Ok(current.saturating_sub(operand)),
        FUNC_NUM_MUL => Ok(current.saturating_mul(operand)),
        FUNC_NUM_DIV => legacy_div_i64(current, operand),
        FUNC_NUM_MOD => legacy_mod_i64(current, operand),
        FUNC_NUM_AND => Ok(current & operand),
        FUNC_NUM_OR => Ok(current | operand),
        FUNC_NUM_XOR => Ok(current ^ operand),
        FUNC_NUM_MAX => Ok(current.max(operand)),
        FUNC_NUM_MIN => Ok(current.min(operand)),
        other => anyhow::bail!("legacy integer funcall {other} is not implemented"),
    }
}

fn legacy_signed_div_mod_i64(current: i64, operand: i64) -> Result<(i64, i64)> {
    anyhow::ensure!(operand != 0, "legacy integer division by zero");

    if current == i64::MIN && operand == -1 {
        return Ok((i64::MIN, 0));
    }

    let truncated_quotient = current / operand;
    let truncated_remainder = current % operand;

    if truncated_remainder != 0 && truncated_remainder.signum() != operand.signum() {
        Ok((truncated_quotient - 1, truncated_remainder + operand))
    } else {
        Ok((truncated_quotient, truncated_remainder))
    }
}

fn legacy_existing_i64(record: &Record, attribute: &str) -> Result<i64> {
    match record.attributes.get(attribute) {
        Some(Value::Int(number)) => Ok(*number),
        Some(other) => Err(anyhow!("expected int attribute {attribute}, got {other:?}")),
        None => Ok(0),
    }
}

fn legacy_existing_map_entry_i64(
    map: &BTreeMap<Value, Value>,
    key: &Value,
    attribute: &str,
) -> Result<i64> {
    match map.get(key) {
        Some(Value::Int(number)) => Ok(*number),
        Some(other) => Err(anyhow!(
            "expected int map value for attribute {attribute}, got {other:?}"
        )),
        None => Ok(0),
    }
}

fn legacy_existing_f64(record: &Record, attribute: &str) -> Result<f64> {
    match record.attributes.get(attribute) {
        Some(Value::Float(number)) => Ok(number.into_inner()),
        Some(other) => Err(anyhow!(
            "expected float attribute {attribute}, got {other:?}"
        )),
        None => Ok(0.0),
    }
}

fn legacy_existing_map_entry_f64(
    map: &BTreeMap<Value, Value>,
    key: &Value,
    attribute: &str,
) -> Result<f64> {
    match map.get(key) {
        Some(Value::Float(number)) => Ok(number.into_inner()),
        Some(Value::Int(number)) => Ok(*number as f64),
        Some(other) => Err(anyhow!(
            "expected float map value for attribute {attribute}, got {other:?}"
        )),
        None => Ok(0.0),
    }
}

fn legacy_numeric_operand_i64(funcall: &ProtocolFuncall) -> Result<i64> {
    match funcall.arg1_datatype {
        HYPERDATATYPE_INT64 => legacy_decode_i64(&funcall.arg1),
        HYPERDATATYPE_FLOAT => Ok(legacy_decode_f64(&funcall.arg1)?.round() as i64),
        other => anyhow::bail!("legacy int operand datatype {other} is not implemented"),
    }
}

fn legacy_numeric_operand_f64(funcall: &ProtocolFuncall) -> Result<f64> {
    match funcall.arg1_datatype {
        HYPERDATATYPE_INT64 => Ok(legacy_decode_i64(&funcall.arg1)? as f64),
        HYPERDATATYPE_FLOAT => legacy_decode_f64(&funcall.arg1),
        other => anyhow::bail!("legacy float operand datatype {other} is not implemented"),
    }
}

fn legacy_value_as_bytes(value: &Value) -> Result<Vec<u8>> {
    match value {
        Value::Bytes(bytes) => Ok(bytes.to_vec()),
        Value::String(text) => Ok(text.as_bytes().to_vec()),
        other => Err(anyhow!("cannot encode {other:?} as legacy bytes")),
    }
}

fn legacy_decode_i64(bytes: &[u8]) -> Result<i64> {
    if bytes.len() < 8 {
        anyhow::bail!("legacy int payload is truncated");
    }
    decode_le_i64_exact(&bytes[..8], "legacy int payload")
}

fn legacy_decode_f64(bytes: &[u8]) -> Result<f64> {
    if bytes.len() < 8 {
        anyhow::bail!("legacy float payload is truncated");
    }
    decode_le_f64_exact(&bytes[..8], "legacy float payload")
}

fn decode_be_u64_exact(bytes: &[u8], label: &str) -> Result<u64> {
    let raw = bytes.try_into().map_err(|_| {
        anyhow!(
            "{label} is not exactly {} bytes",
            std::mem::size_of::<u64>()
        )
    })?;
    Ok(u64::from_be_bytes(raw))
}

fn lock_mutex<'a, T>(mutex: &'a Mutex<T>, label: &str) -> Result<MutexGuard<'a, T>> {
    mutex
        .lock()
        .map_err(|_| anyhow!("{label} poisoned while handling request"))
}

fn decode_le_u32_exact(bytes: &[u8], label: &str) -> Result<u32> {
    let raw = bytes.try_into().map_err(|_| {
        anyhow!(
            "{label} is not exactly {} bytes",
            std::mem::size_of::<u32>()
        )
    })?;
    Ok(u32::from_le_bytes(raw))
}

fn decode_le_i64_exact(bytes: &[u8], label: &str) -> Result<i64> {
    let raw = bytes.try_into().map_err(|_| {
        anyhow!(
            "{label} is not exactly {} bytes",
            std::mem::size_of::<i64>()
        )
    })?;
    Ok(i64::from_le_bytes(raw))
}

fn decode_le_f64_exact(bytes: &[u8], label: &str) -> Result<f64> {
    let raw = bytes.try_into().map_err(|_| {
        anyhow!(
            "{label} is not exactly {} bytes",
            std::mem::size_of::<f64>()
        )
    })?;
    Ok(f64::from_le_bytes(raw))
}

pub async fn handle_coordinator_admin_request(
    runtime: &ClusterRuntime,
    request: CoordinatorAdminRequest,
) -> Result<CoordinatorReturnCode> {
    match request {
        CoordinatorAdminRequest::DaemonRegister(node) => {
            match HyperdexAdminService::handle(runtime, AdminRequest::RegisterDaemon(node)).await {
                Ok(AdminResponse::Unit) => Ok(CoordinatorReturnCode::Success),
                Ok(other) => {
                    anyhow::bail!("unexpected admin response to daemon_register: {other:?}")
                }
                Err(err) => Ok(map_admin_error_to_coordinator(&err)),
            }
        }
        CoordinatorAdminRequest::SpaceAdd(space) => {
            match HyperdexAdminService::handle(runtime, AdminRequest::CreateSpace(space)).await {
                Ok(AdminResponse::Unit) => Ok(CoordinatorReturnCode::Success),
                Ok(other) => anyhow::bail!("unexpected admin response to space_add: {other:?}"),
                Err(err) => Ok(map_admin_error_to_coordinator(&err)),
            }
        }
        CoordinatorAdminRequest::SpaceRm(name) => {
            match HyperdexAdminService::handle(runtime, AdminRequest::DropSpace(name)).await {
                Ok(AdminResponse::Unit) => Ok(CoordinatorReturnCode::Success),
                Ok(other) => anyhow::bail!("unexpected admin response to space_rm: {other:?}"),
                Err(err) => Ok(map_admin_error_to_coordinator(&err)),
            }
        }
        CoordinatorAdminRequest::WaitUntilStable | CoordinatorAdminRequest::ConfigGet => {
            Ok(CoordinatorReturnCode::Malformed)
        }
    }
}

pub async fn handle_coordinator_admin_method(
    runtime: &ClusterRuntime,
    method: &str,
    request: CoordinatorAdminRequest,
) -> Result<[u8; 2]> {
    Ok(handle_coordinator_control_method(runtime, method, request)
        .await?
        .status)
}

pub async fn handle_coordinator_control_method(
    runtime: &ClusterRuntime,
    method: &str,
    request: CoordinatorAdminRequest,
) -> Result<CoordinatorControlResponse> {
    let (status, body) = match (method, request) {
        ("daemon_register", CoordinatorAdminRequest::DaemonRegister(node)) => (
            handle_coordinator_admin_request(
                runtime,
                CoordinatorAdminRequest::DaemonRegister(node),
            )
            .await?,
            Vec::new(),
        ),
        ("space_add", CoordinatorAdminRequest::SpaceAdd(space)) => (
            handle_coordinator_admin_request(runtime, CoordinatorAdminRequest::SpaceAdd(space))
                .await?,
            Vec::new(),
        ),
        ("space_rm", CoordinatorAdminRequest::SpaceRm(name)) => (
            handle_coordinator_admin_request(runtime, CoordinatorAdminRequest::SpaceRm(name))
                .await?,
            Vec::new(),
        ),
        ("wait_until_stable", CoordinatorAdminRequest::WaitUntilStable) => (
            CoordinatorReturnCode::Success,
            serde_json::to_vec(&runtime.stable_version()?)?,
        ),
        ("config_get", CoordinatorAdminRequest::ConfigGet) => (
            CoordinatorReturnCode::Success,
            serde_json::to_vec(&runtime.config_view()?)?,
        ),
        _ => (CoordinatorReturnCode::Malformed, Vec::new()),
    };

    Ok(CoordinatorControlResponse {
        status: status.encode(),
        body,
    })
}

async fn read_coordinator_control_request(
    stream: &mut tokio::net::TcpStream,
) -> Result<(String, CoordinatorAdminRequest)> {
    let mut header = [0u8; COORDINATOR_CONTROL_HEADER_SIZE];
    stream.read_exact(&mut header).await?;

    let method_len = u16::from_be_bytes([header[0], header[1]]) as usize;
    let body_len = u32::from_be_bytes([header[2], header[3], header[4], header[5]]) as usize;
    let mut method = vec![0u8; method_len];
    let mut body = vec![0u8; body_len];
    stream.read_exact(&mut method).await?;
    stream.read_exact(&mut body).await?;

    let method = String::from_utf8(method)?;
    let request = serde_json::from_slice(&body)?;
    Ok((method, request))
}

pub fn encode_coordinator_control_request(
    method: &str,
    request: &CoordinatorAdminRequest,
) -> Result<Vec<u8>> {
    let method_bytes = method.as_bytes();
    let body = serde_json::to_vec(request)?;
    let mut bytes =
        Vec::with_capacity(COORDINATOR_CONTROL_HEADER_SIZE + method_bytes.len() + body.len());
    bytes.extend_from_slice(&(method_bytes.len() as u16).to_be_bytes());
    bytes.extend_from_slice(&(body.len() as u32).to_be_bytes());
    bytes.extend_from_slice(method_bytes);
    bytes.extend_from_slice(&body);
    Ok(bytes)
}

pub async fn request_coordinator_control_once(
    address: SocketAddr,
    method: &str,
    request: &CoordinatorAdminRequest,
) -> Result<[u8; 2]> {
    let mut stream = tokio::net::TcpStream::connect(address).await?;
    stream
        .write_all(&encode_coordinator_control_request(method, request)?)
        .await?;
    stream.flush().await?;

    let mut response = [0u8; 2];
    stream.read_exact(&mut response).await?;
    Ok(response)
}

pub async fn request_coordinator_control_with_body_once(
    address: SocketAddr,
    method: &str,
    request: &CoordinatorAdminRequest,
) -> Result<CoordinatorControlResponse> {
    let mut stream = tokio::net::TcpStream::connect(address).await?;
    stream
        .write_all(&encode_coordinator_control_request(method, request)?)
        .await?;
    stream.flush().await?;

    let mut status = [0u8; 2];
    stream.read_exact(&mut status).await?;

    let mut body_len = [0u8; COORDINATOR_CONTROL_BODY_LENGTH_SIZE];
    match stream.read_exact(&mut body_len).await {
        Ok(_) => {
            let body_len = u32::from_be_bytes(body_len) as usize;
            let mut body = vec![0u8; body_len];
            stream.read_exact(&mut body).await?;
            Ok(CoordinatorControlResponse { status, body })
        }
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
            Ok(CoordinatorControlResponse {
                status,
                body: Vec::new(),
            })
        }
        Err(err) => Err(err.into()),
    }
}

pub async fn sync_runtime_with_coordinator(
    runtime: &ClusterRuntime,
    address: SocketAddr,
) -> Result<ConfigView> {
    let response = request_coordinator_control_with_body_once(
        address,
        CoordinatorAdminRequest::ConfigGet.method_name(),
        &CoordinatorAdminRequest::ConfigGet,
    )
    .await?;
    let status = CoordinatorReturnCode::decode(&response.status)?;
    if status != CoordinatorReturnCode::Success {
        return Err(anyhow!(
            "coordinator config_get returned unexpected status {status:?}"
        ));
    }

    let view: ConfigView = serde_json::from_slice(&response.body)?;
    runtime.apply_config_view(&view)?;
    Ok(view)
}

async fn write_coordinator_control_response(
    stream: &mut tokio::net::TcpStream,
    response: &CoordinatorControlResponse,
) -> Result<()> {
    stream.write_all(&response.status).await?;
    if !response.body.is_empty() {
        stream
            .write_all(&(response.body.len() as u32).to_be_bytes())
            .await?;
        stream.write_all(&response.body).await?;
    }
    Ok(())
}

pub async fn handle_legacy_admin_request(
    runtime: &ClusterRuntime,
    request: LegacyAdminRequest,
) -> Result<LegacyAdminReturnCode> {
    match request {
        LegacyAdminRequest::SpaceAddDsl(schema) => {
            let Ok(space) = parse_hyperdex_space(&schema) else {
                return Ok(LegacyAdminReturnCode::BadSpace);
            };
            Ok(
                handle_coordinator_admin_request(runtime, CoordinatorAdminRequest::SpaceAdd(space))
                    .await?
                    .legacy_admin_status(),
            )
        }
        LegacyAdminRequest::SpaceRm(name) => Ok(handle_coordinator_admin_request(
            runtime,
            CoordinatorAdminRequest::SpaceRm(name),
        )
        .await?
        .legacy_admin_status()),
    }
}

pub async fn handle_replicant_admin_request(
    runtime: &ClusterRuntime,
    request: ReplicantAdminRequestMessage,
) -> Result<Vec<u8>> {
    let nonce = request.nonce();
    match request.into_coordinator_request()? {
        CoordinatorAdminRequest::SpaceAdd(space) => {
            let status =
                handle_coordinator_admin_request(runtime, CoordinatorAdminRequest::SpaceAdd(space))
                    .await?;
            Ok(ReplicantCallCompletion {
                nonce,
                status: ReplicantReturnCode::Success,
                output: status.encode().to_vec(),
            }
            .encode())
        }
        CoordinatorAdminRequest::SpaceRm(name) => {
            let status =
                handle_coordinator_admin_request(runtime, CoordinatorAdminRequest::SpaceRm(name))
                    .await?;
            Ok(ReplicantCallCompletion {
                nonce,
                status: ReplicantReturnCode::Success,
                output: status.encode().to_vec(),
            }
            .encode())
        }
        CoordinatorAdminRequest::WaitUntilStable => Ok(ReplicantConditionCompletion {
            nonce,
            status: ReplicantReturnCode::Success,
            state: legacy_condition_state(runtime.stable_version()?),
            data: Vec::new(),
        }
        .encode()),
        CoordinatorAdminRequest::ConfigGet => {
            let view = runtime.config_view()?;
            Ok(ReplicantConditionCompletion {
                nonce,
                status: ReplicantReturnCode::Success,
                state: legacy_condition_state(view.version),
                data: default_legacy_config_encoder(&view)?,
            }
            .encode())
        }
        CoordinatorAdminRequest::DaemonRegister(_) => {
            anyhow::bail!("daemon registration is not part of the legacy admin client surface")
        }
    }
}

fn map_admin_error_to_coordinator(err: &anyhow::Error) -> CoordinatorReturnCode {
    let msg = err.to_string();
    if msg.contains("already exists") {
        CoordinatorReturnCode::Duplicate
    } else if msg.contains("not found") {
        CoordinatorReturnCode::NotFound
    } else {
        CoordinatorReturnCode::NoCanDo
    }
}

impl HyperdexAdminService for ClusterRuntime {
    async fn handle(&self, request: AdminRequest) -> Result<AdminResponse> {
        match request {
            AdminRequest::RegisterDaemon(node) => {
                self.register_daemon(node)?;
                Ok(AdminResponse::Unit)
            }
            AdminRequest::CreateSpace(space) => {
                self.create_space(space)?;
                Ok(AdminResponse::Unit)
            }
            AdminRequest::CreateSpaceDsl(schema) => {
                self.create_space(parse_hyperdex_space(&schema)?)?;
                Ok(AdminResponse::Unit)
            }
            AdminRequest::DropSpace(space) => {
                self.drop_space(&space)?;
                Ok(AdminResponse::Unit)
            }
            AdminRequest::ListSpaces => Ok(AdminResponse::Spaces(self.catalog.list_spaces()?)),
            AdminRequest::DumpConfig => Ok(AdminResponse::Config(self.config_view()?)),
            AdminRequest::WaitUntilStable => Ok(AdminResponse::Stable {
                version: self.stable_version()?,
            }),
        }
    }
}

impl HyperdexClientService for ClusterRuntime {
    async fn handle(&self, request: ClientRequest) -> Result<ClientResponse> {
        match request {
            ClientRequest::Put {
                space,
                key,
                mutations,
            } => {
                let primary = self.route_primary_for_space(&space, &key)?;
                if primary == self.local_node_id {
                    Ok(match self.apply_primary_put(space, key, mutations).await? {
                        DataPlaneResponse::Unit => ClientResponse::Unit,
                        DataPlaneResponse::ConditionFailed => ClientResponse::ConditionFailed,
                        DataPlaneResponse::Record(_)
                        | DataPlaneResponse::SearchResult(_)
                        | DataPlaneResponse::Deleted(_) => {
                            anyhow::bail!("unexpected record response to local put")
                        }
                    })
                } else {
                    Ok(
                        match self
                            .forward_data_request(
                                primary,
                                DataPlaneRequest::Put {
                                    space,
                                    key,
                                    mutations,
                                },
                            )
                            .await?
                        {
                            DataPlaneResponse::Unit => ClientResponse::Unit,
                            DataPlaneResponse::ConditionFailed => ClientResponse::ConditionFailed,
                            DataPlaneResponse::Record(_)
                            | DataPlaneResponse::SearchResult(_)
                            | DataPlaneResponse::Deleted(_) => {
                                anyhow::bail!("unexpected record response to remote put")
                            }
                        },
                    )
                }
            }
            ClientRequest::Get { space, key } => Ok(ClientResponse::Record(
                self.execute_get_with_replica_fallback(space, key).await?,
            )),
            ClientRequest::Delete { space, key } => {
                let primary = self.route_primary_for_space(&space, &key)?;
                if primary == self.local_node_id {
                    Ok(match self.apply_primary_delete(space, key).await? {
                        DataPlaneResponse::Unit => ClientResponse::Unit,
                        DataPlaneResponse::ConditionFailed => ClientResponse::ConditionFailed,
                        DataPlaneResponse::Record(_)
                        | DataPlaneResponse::SearchResult(_)
                        | DataPlaneResponse::Deleted(_) => {
                            anyhow::bail!("unexpected record response to local delete")
                        }
                    })
                } else {
                    Ok(
                        match self
                            .forward_data_request(primary, DataPlaneRequest::Delete { space, key })
                            .await?
                        {
                            DataPlaneResponse::Unit => ClientResponse::Unit,
                            DataPlaneResponse::ConditionFailed => ClientResponse::ConditionFailed,
                            DataPlaneResponse::Record(_)
                            | DataPlaneResponse::SearchResult(_)
                            | DataPlaneResponse::Deleted(_) => {
                                anyhow::bail!("unexpected record response to remote delete")
                            }
                        },
                    )
                }
            }
            ClientRequest::ConditionalPut {
                space,
                key,
                checks,
                mutations,
            } => {
                let primary = self.route_primary_for_space(&space, &key)?;
                if primary == self.local_node_id {
                    Ok(
                        match self
                            .apply_primary_conditional_put(space, key, checks, mutations)
                            .await?
                        {
                            DataPlaneResponse::Unit => ClientResponse::Unit,
                            DataPlaneResponse::ConditionFailed => ClientResponse::ConditionFailed,
                            DataPlaneResponse::Record(_)
                            | DataPlaneResponse::SearchResult(_)
                            | DataPlaneResponse::Deleted(_) => {
                                anyhow::bail!("unexpected record response to local conditional put")
                            }
                        },
                    )
                } else {
                    Ok(
                        match self
                            .forward_data_request(
                                primary,
                                DataPlaneRequest::ConditionalPut {
                                    space,
                                    key,
                                    checks,
                                    mutations,
                                },
                            )
                            .await?
                        {
                            DataPlaneResponse::Unit => ClientResponse::Unit,
                            DataPlaneResponse::ConditionFailed => ClientResponse::ConditionFailed,
                            DataPlaneResponse::Record(_)
                            | DataPlaneResponse::SearchResult(_)
                            | DataPlaneResponse::Deleted(_) => {
                                anyhow::bail!(
                                    "unexpected record response to remote conditional put"
                                )
                            }
                        },
                    )
                }
            }
            ClientRequest::Search { space, checks } => Ok(ClientResponse::SearchResult(
                self.execute_distributed_search(space, checks).await?,
            )),
            ClientRequest::Count { space, checks } => Ok(ClientResponse::Count(
                self.execute_distributed_count(space, checks).await?,
            )),
            ClientRequest::DeleteGroup { space, checks } => Ok(ClientResponse::Deleted(
                self.execute_distributed_delete_group(space, checks).await?,
            )),
        }
    }
}

fn should_skip_unavailable_read(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_ascii_lowercase();
    msg.contains("connection refused")
        || msg.contains("connection reset")
        || msg.contains("broken pipe")
        || msg.contains("transport error")
        || msg.contains("tcp connect error")
        || msg.contains("channel closed")
        || msg.contains("deadline has elapsed")
}

fn should_skip_distributed_read_replica(err: &anyhow::Error) -> bool {
    if should_skip_unavailable_read(err) {
        return true;
    }

    should_skip_schema_gap_replica(err)
}

fn should_skip_delete_group_snapshot_replica(err: &anyhow::Error) -> bool {
    should_skip_schema_gap_replica(err)
}

fn should_skip_schema_gap_replica(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_ascii_lowercase();
    msg.contains("unknown space") || msg.contains("space ") && msg.contains("does not exist")
}

pub fn bootstrap_runtime() -> Result<ClusterRuntime> {
    ClusterRuntime::single_node(ClusterConfig::default())
}

pub fn coordinator_cluster_config() -> ClusterConfig {
    let mut config = ClusterConfig::default();
    config.nodes.clear();
    config
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessMode {
    Coordinator {
        data_dir: String,
        listen_host: String,
        listen_port: u16,
    },
    Daemon {
        node_id: u64,
        threads: usize,
        data_dir: String,
        listen_host: String,
        listen_port: u16,
        control_port: u16,
        coordinator_host: String,
        coordinator_port: u16,
        consensus: ConsensusBackend,
        placement: PlacementBackend,
        storage: StorageBackend,
        internode_transport: TransportBackend,
    },
}

pub fn parse_process_mode(args: &[String]) -> Result<ProcessMode> {
    let Some(mode) = args.first() else {
        return Err(anyhow!("expected `coordinator` or `daemon` subcommand"));
    };

    match mode.as_str() {
        "coordinator" => Ok(ProcessMode::Coordinator {
            data_dir: required_option(args, "--data")?,
            listen_host: required_option(args, "--listen")?,
            listen_port: required_option(args, "--listen-port")?.parse()?,
        }),
        "daemon" => {
            let listen_port = required_option(args, "--listen-port")?.parse()?;
            Ok(ProcessMode::Daemon {
                node_id: required_option(args, "--node-id")?.parse()?,
                threads: required_option(args, "--threads")?.parse()?,
                data_dir: required_option(args, "--data")?,
                listen_host: required_option(args, "--listen")?,
                listen_port,
                control_port: optional_option(args, "--control-port")
                    .map(|value| value.parse())
                    .transpose()?
                    .unwrap_or(listen_port),
                coordinator_host: required_option(args, "--coordinator")?,
                coordinator_port: required_option(args, "--coordinator-port")?.parse()?,
                consensus: optional_consensus_backend(args)?,
                placement: optional_placement_backend(args)?,
                storage: optional_storage_backend(args)?,
                internode_transport: optional_transport_backend(args)?,
            })
        }
        other => Err(anyhow!("unknown subcommand `{other}`")),
    }
}

fn required_option(args: &[String], name: &str) -> Result<String> {
    for arg in args {
        if let Some(value) = arg.strip_prefix(&format!("{name}=")) {
            return Ok(value.to_owned());
        }
    }

    Err(anyhow!("missing required option `{name}=...`"))
}

fn optional_consensus_backend(args: &[String]) -> Result<ConsensusBackend> {
    Ok(match optional_option(args, "--consensus").as_deref() {
        None | Some("single-node") => ConsensusBackend::SingleNode,
        Some("mirror") => ConsensusBackend::Mirror,
        Some("omnipaxos") => ConsensusBackend::OmniPaxos,
        Some("openraft") => ConsensusBackend::OpenRaft,
        Some(other) => return Err(anyhow!("unknown consensus backend `{other}`")),
    })
}

fn optional_placement_backend(args: &[String]) -> Result<PlacementBackend> {
    Ok(match optional_option(args, "--placement").as_deref() {
        None | Some("hyperspace") => PlacementBackend::Hyperspace,
        Some("rendezvous") => PlacementBackend::Rendezvous,
        Some(other) => return Err(anyhow!("unknown placement backend `{other}`")),
    })
}

fn optional_storage_backend(args: &[String]) -> Result<StorageBackend> {
    Ok(match optional_option(args, "--storage").as_deref() {
        None | Some("memory") => StorageBackend::Memory,
        Some("rocksdb") => StorageBackend::RocksDb,
        Some(other) => return Err(anyhow!("unknown storage backend `{other}`")),
    })
}

fn optional_transport_backend(args: &[String]) -> Result<TransportBackend> {
    Ok(match optional_option(args, "--transport").as_deref() {
        None | Some("in-process") => TransportBackend::InProcess,
        Some("grpc") => TransportBackend::Grpc,
        Some(other) => return Err(anyhow!("unknown transport backend `{other}`")),
    })
}

fn optional_option(args: &[String], name: &str) -> Option<String> {
    for arg in args {
        if let Some(value) = arg.strip_prefix(&format!("{name}=")) {
            return Some(value.to_owned());
        }
    }

    None
}

pub fn daemon_registration_node(mode: &ProcessMode) -> Option<ClusterNode> {
    match mode {
        ProcessMode::Daemon {
            node_id,
            listen_host,
            listen_port,
            control_port,
            ..
        } => Some(ClusterNode {
            id: *node_id,
            host: listen_host.clone(),
            control_port: *control_port,
            data_port: *listen_port,
        }),
        ProcessMode::Coordinator { .. } => None,
    }
}

pub fn daemon_cluster_config(mode: &ProcessMode) -> ClusterConfig {
    let mut config = ClusterConfig::default();

    if let ProcessMode::Daemon {
        node_id,
        listen_host,
        listen_port,
        control_port,
        consensus,
        placement,
        storage,
        internode_transport,
        ..
    } = mode
    {
        config.nodes = vec![ClusterNode {
            id: *node_id,
            host: listen_host.clone(),
            control_port: *control_port,
            data_port: *listen_port,
        }];
        config.consensus = consensus.clone();
        config.placement = placement.clone();
        config.storage = storage.clone();
        config.internode_transport = internode_transport.clone();
    }

    config
}

#[cfg(test)]
mod tests;
