use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cluster_config::{
    ClusterConfig, ClusterNode, ConsensusBackend, PlacementBackend, StorageBackend,
    TransportBackend,
};
use control_plane::{Catalog, InMemoryCatalog};
use data_model::{
    parse_hyperdex_space, Attribute, Check, Mutation, NumericOp, Predicate, Record, Space, Value,
};
use data_plane::DataPlane;
use engine_memory::MemoryEngine;
use engine_rocks::RocksEngine;
use hyperdex_admin_protocol::{
    AdminRequest, AdminResponse, ConfigView, CoordinatorAdminRequest, CoordinatorReturnCode,
    HyperdexAdminService, LegacyAdminRequest, LegacyAdminReturnCode,
};
use hyperdex_client_protocol::{ClientRequest, ClientResponse, HyperdexClientService};
use legacy_protocol::{
    config_mismatch_response, AtomicRequest, AtomicResponse, CountRequest, CountResponse,
    GetAttribute, GetRequest, GetResponse, GetValue, LegacyCheck, LegacyFuncall, LegacyFuncallName,
    LegacyMessageType, LegacyPredicate, LegacyReturnCode, RequestHeader, ResponseHeader,
    SearchContinueRequest, SearchDoneResponse, SearchItemResponse, SearchStartRequest,
    LEGACY_ATOMIC_FLAG_FAIL_IF_FOUND, LEGACY_ATOMIC_FLAG_FAIL_IF_NOT_FOUND,
    LEGACY_ATOMIC_FLAG_WRITE,
};
use placement_core::{HyperSpacePlacement, PlacementStrategy, RendezvousPlacement};
use storage_core::{StorageEngine, WriteResult};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
<<<<<<< HEAD
use transport_core::{
    ClusterTransport, DataPlaneRequest, DataPlaneResponse, InProcessTransport, InternodeRequest,
    InternodeResponse, RemoteNode, DATA_PLANE_METHOD,
};
||||||| parent of 30354c7 (Add gRPC internode forwarding for data plane)
use transport_core::InProcessTransport;
=======
use transport_core::{
    ClusterTransport, DataPlaneRequest, DataPlaneResponse, InProcessTransport, InternodeRequest,
    InternodeResponse, RemoteNode, DATA_PLANE_METHOD,
};
>>>>>>> 30354c7 (Add gRPC internode forwarding for data plane)

pub const COORDINATOR_CONTROL_HEADER_SIZE: usize = 2 + 4;
pub const COORDINATOR_CONTROL_BODY_LENGTH_SIZE: usize = 4;

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
    legacy_searches: Mutex<BTreeMap<u64, VecDeque<Record>>>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoordinatorControlResponse {
    pub status: [u8; 2],
    pub body: Vec<u8>,
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
        let Some(local_node_id) = config.nodes.first().map(|node| node.id) else {
            return Err(anyhow!("cluster config must contain at least one node"));
        };
        Self::for_node_with_data_dir(config, local_node_id, data_dir)
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
        self.record_config_change();
        Ok(())
    }

    fn drop_space(&self, name: &str) -> Result<()> {
        if self.catalog.get_space(name)?.is_none() {
            return Err(anyhow!("space {name} not found"));
        }
        self.catalog.drop_space(name)?;
        self.storage.drop_space(name)?;
        self.record_config_change();
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

    pub fn route_primary(&self, key: &[u8]) -> Result<u64> {
        let layout = self.catalog.layout()?;
        Ok(self.placement_strategy.locate(key, &layout).primary)
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
                    } => match self.data_plane.put(&space, key, &mutations)? {
                        WriteResult::Written | WriteResult::Missing => DataPlaneResponse::Unit,
                        WriteResult::ConditionFailed => DataPlaneResponse::ConditionFailed,
                    },
                    DataPlaneRequest::Get { space, key } => {
                        DataPlaneResponse::Record(self.data_plane.get(&space, &key)?)
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
        let coordinator_state = *self
            .coordinator_state
            .lock()
            .expect("coordinator state poisoned");
        let cluster = self
            .cluster_config
            .lock()
            .expect("cluster config poisoned")
            .clone();
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
            let mut cluster_config = self.cluster_config.lock().expect("cluster config poisoned");
            upsert_cluster_node(&mut cluster_config.nodes, node)
        };

        if catalog_changed || config_changed {
            self.record_config_change();
        }

        Ok(())
    }

    fn stable_version(&self) -> u64 {
        self.coordinator_state
            .lock()
            .expect("coordinator state poisoned")
            .stable_through
    }

    fn record_config_change(&self) {
        let mut coordinator_state = self
            .coordinator_state
            .lock()
            .expect("coordinator state poisoned");
        coordinator_state.version += 1;
        coordinator_state.stable_through = coordinator_state.version;
    }

    fn apply_config_view(&self, view: &ConfigView) -> Result<()> {
        for node in &view.cluster.nodes {
            self.catalog.register_daemon(node.clone())?;
        }

        *self
            .cluster_config
            .lock()
            .expect("cluster config poisoned") = view.cluster.clone();

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

        *self
            .coordinator_state
            .lock()
            .expect("coordinator state poisoned") = CoordinatorState {
            version: view.version,
            stable_through: view.stable_through,
        };

        Ok(())
    }

    fn remote_node(&self, node_id: u64) -> Result<RemoteNode> {
        let cluster_config = self.cluster_config.lock().expect("cluster config poisoned");
        let Some(node) = cluster_config
            .nodes
            .iter()
            .find(|node| node.id == node_id)
        else {
            return Err(anyhow!(
                "cluster config does not define remote node {node_id}"
            ));
        };
        Ok(RemoteNode {
            id: node.id,
            host: node.host.clone(),
            port: node.data_port,
        })
    }
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
        let (method, request) = read_coordinator_control_request(&mut stream).await?;
        let response = handler(method, request).await?;
        write_coordinator_control_response(&mut stream, &response).await?;
        stream.flush().await?;
        Ok(())
    }

    pub async fn serve_forever_with<H, F>(&self, handler: H) -> Result<()>
    where
        H: Fn(String, CoordinatorAdminRequest) -> F,
        F: std::future::Future<Output = Result<CoordinatorControlResponse>>,
    {
        loop {
            self.serve_once_with(&handler).await?;
        }
    }
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
            let request = AtomicRequest::decode_body(body)?;
            let space = runtime.only_space_name()?;
            let key = request.key.clone();
            let exists = legacy_record_exists(runtime, &space, &key).await?;
            let checks = legacy_checks_from_request(request.checks);

            let status = if request.flags & LEGACY_ATOMIC_FLAG_FAIL_IF_FOUND != 0 && exists {
                LegacyReturnCode::CompareFailed
            } else if request.flags & LEGACY_ATOMIC_FLAG_FAIL_IF_NOT_FOUND != 0 && !exists {
                LegacyReturnCode::NotFound
            } else if request.flags & LEGACY_ATOMIC_FLAG_WRITE != 0 {
                let mutations = legacy_mutations_from_funcalls(request.funcalls)?;
                let response = if checks.is_empty() {
                    HyperdexClientService::handle(
                        runtime,
                        ClientRequest::Put {
                            space,
                            key: request.key.into(),
                            mutations,
                        },
                    )
                    .await?
                } else {
                    HyperdexClientService::handle(
                        runtime,
                        ClientRequest::ConditionalPut {
                            space,
                            key: request.key.into(),
                            checks,
                            mutations,
                        },
                    )
                    .await?
                };
                legacy_atomic_status(response)?
            } else {
                if !checks.is_empty() || !request.funcalls.is_empty() {
                    anyhow::bail!("legacy delete path does not yet support checks or funcalls");
                }
                let response = HyperdexClientService::handle(
                    runtime,
                    ClientRequest::Delete {
                        space,
                        key: request.key.into(),
                    },
                )
                .await?;
                legacy_atomic_status(response)?
            };

            Ok((
                ResponseHeader {
                    message_type: LegacyMessageType::RespAtomic,
                    target_virtual_server: header.target_virtual_server,
                    nonce: header.nonce,
                },
                AtomicResponse { status }.encode_body().to_vec(),
            ))
        }
        LegacyMessageType::ReqSearchStart => {
            let request = SearchStartRequest::decode_body(body)?;
            let response = HyperdexClientService::handle(
                runtime,
                ClientRequest::Search {
                    space: request.space,
                    checks: legacy_checks_from_request(request.checks),
                },
            )
            .await?;

            let ClientResponse::SearchResult(records) = response else {
                anyhow::bail!("unexpected runtime response to search request");
            };

            runtime
                .legacy_searches
                .lock()
                .expect("legacy search state poisoned")
                .insert(request.search_id, VecDeque::from(records));

            legacy_search_response(runtime, header, request.search_id)
        }
        LegacyMessageType::ReqSearchNext => {
            let request = SearchContinueRequest::decode_body(body)?;
            legacy_search_response(runtime, header, request.search_id)
        }
        LegacyMessageType::ReqSearchStop => {
            let request = SearchContinueRequest::decode_body(body)?;
            runtime
                .legacy_searches
                .lock()
                .expect("legacy search state poisoned")
                .remove(&request.search_id);

            Ok((
                ResponseHeader {
                    message_type: LegacyMessageType::RespSearchDone,
                    target_virtual_server: header.target_virtual_server,
                    nonce: header.nonce,
                },
                SearchDoneResponse {
                    search_id: request.search_id,
                }
                .encode_body()
                .to_vec(),
            ))
        }
        LegacyMessageType::ReqCount => {
            let request = CountRequest::decode_body(body)?;
            let response = HyperdexClientService::handle(
                runtime,
                ClientRequest::Count {
                    space: request.space,
                    checks: Vec::new(),
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
                    nonce: header.nonce,
                },
                CountResponse { count }.encode_body().to_vec(),
            ))
        }
        LegacyMessageType::ReqGet => {
            let request = GetRequest::decode_body(body)?;
            let response = HyperdexClientService::handle(
                runtime,
                ClientRequest::Get {
                    space: runtime.only_space_name()?,
                    key: request.key.into(),
                },
            )
            .await?;

            let ClientResponse::Record(record) = response else {
                anyhow::bail!("unexpected runtime response to get request");
            };

            let get = match record {
                Some(record) => GetResponse {
                    status: LegacyReturnCode::Success,
                    attributes: record
                        .attributes
                        .into_iter()
                        .map(|(name, value)| GetAttribute {
                            name,
                            value: legacy_value_from_model(value),
                        })
                        .collect(),
                },
                None => GetResponse {
                    status: LegacyReturnCode::NotFound,
                    attributes: Vec::new(),
                },
            };

            Ok((
                ResponseHeader {
                    message_type: LegacyMessageType::RespGet,
                    target_virtual_server: header.target_virtual_server,
                    nonce: header.nonce,
                },
                get.encode_body(),
            ))
        }
        _ => Ok((config_mismatch_response(header), Vec::new())),
    }
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

fn legacy_search_response(
    runtime: &ClusterRuntime,
    header: RequestHeader,
    search_id: u64,
) -> Result<(ResponseHeader, Vec<u8>)> {
    let mut searches = runtime
        .legacy_searches
        .lock()
        .expect("legacy search state poisoned");
    let Some(records) = searches.get_mut(&search_id) else {
        return Ok((
            ResponseHeader {
                message_type: LegacyMessageType::RespSearchDone,
                target_virtual_server: header.target_virtual_server,
                nonce: header.nonce,
            },
            SearchDoneResponse { search_id }.encode_body().to_vec(),
        ));
    };

    let response = if let Some(record) = records.pop_front() {
        (
            ResponseHeader {
                message_type: LegacyMessageType::RespSearchItem,
                target_virtual_server: header.target_virtual_server,
                nonce: header.nonce,
            },
            SearchItemResponse {
                search_id,
                key: record.key.to_vec(),
                attributes: record
                    .attributes
                    .into_iter()
                    .map(|(name, value)| GetAttribute {
                        name,
                        value: legacy_value_from_model(value),
                    })
                    .collect(),
            }
            .encode_body(),
        )
    } else {
        (
            ResponseHeader {
                message_type: LegacyMessageType::RespSearchDone,
                target_virtual_server: header.target_virtual_server,
                nonce: header.nonce,
            },
            SearchDoneResponse { search_id }.encode_body().to_vec(),
        )
    };

    if records.is_empty() {
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

fn legacy_checks_from_request(checks: Vec<LegacyCheck>) -> Vec<Check> {
    checks
        .into_iter()
        .map(|check| Check {
            attribute: check.attribute,
            predicate: model_predicate_from_legacy(check.predicate),
            value: model_value_from_legacy(check.value),
        })
        .collect()
}

fn legacy_mutations_from_funcalls(funcalls: Vec<LegacyFuncall>) -> Result<Vec<Mutation>> {
    funcalls
        .into_iter()
        .map(|funcall| match funcall.name {
            LegacyFuncallName::Set => Ok(Mutation::Set(Attribute {
                name: funcall.attribute,
                value: model_value_from_legacy(funcall.arg1),
            })),
            LegacyFuncallName::NumAdd
            | LegacyFuncallName::NumSub
            | LegacyFuncallName::NumMul
            | LegacyFuncallName::NumDiv
            | LegacyFuncallName::NumMod
            | LegacyFuncallName::NumAnd
            | LegacyFuncallName::NumOr
            | LegacyFuncallName::NumXor => {
                let GetValue::Int(operand) = funcall.arg1 else {
                    anyhow::bail!("numeric legacy funcalls require integer operands");
                };
                if funcall.arg2.is_some() {
                    anyhow::bail!("scalar numeric legacy funcalls do not use arg2");
                }
                Ok(Mutation::Numeric {
                    attribute: funcall.attribute,
                    op: model_numeric_op_from_legacy(funcall.name),
                    operand,
                })
            }
            other => anyhow::bail!("legacy funcall {other:?} is not implemented yet"),
        })
        .collect()
}

fn legacy_value_from_model(value: Value) -> GetValue {
    match value {
        Value::Null => GetValue::Null,
        Value::Bool(v) => GetValue::Bool(v),
        Value::Int(v) => GetValue::Int(v),
        Value::Bytes(v) => GetValue::Bytes(v.to_vec()),
        Value::String(v) => GetValue::String(v),
        Value::Float(v) => GetValue::String(v.to_string()),
        Value::List(v) => GetValue::String(format!("{v:?}")),
        Value::Set(v) => GetValue::String(format!("{v:?}")),
        Value::Map(v) => GetValue::String(format!("{v:?}")),
    }
}

fn model_value_from_legacy(value: GetValue) -> Value {
    match value {
        GetValue::Null => Value::Null,
        GetValue::Bool(v) => Value::Bool(v),
        GetValue::Int(v) => Value::Int(v),
        GetValue::Bytes(v) => Value::Bytes(v.into()),
        GetValue::String(v) => Value::String(v),
    }
}

fn model_predicate_from_legacy(predicate: LegacyPredicate) -> Predicate {
    match predicate {
        LegacyPredicate::Equal => Predicate::Equal,
        LegacyPredicate::LessThan => Predicate::LessThan,
        LegacyPredicate::LessThanOrEqual => Predicate::LessThanOrEqual,
        LegacyPredicate::GreaterThanOrEqual => Predicate::GreaterThanOrEqual,
        LegacyPredicate::GreaterThan => Predicate::GreaterThan,
    }
}

fn model_numeric_op_from_legacy(name: LegacyFuncallName) -> NumericOp {
    match name {
        LegacyFuncallName::NumAdd => NumericOp::Add,
        LegacyFuncallName::NumSub => NumericOp::Sub,
        LegacyFuncallName::NumMul => NumericOp::Mul,
        LegacyFuncallName::NumDiv => NumericOp::Div,
        LegacyFuncallName::NumMod => NumericOp::Mod,
        LegacyFuncallName::NumAnd => NumericOp::And,
        LegacyFuncallName::NumOr => NumericOp::Or,
        LegacyFuncallName::NumXor => NumericOp::Xor,
        other => unreachable!("not a scalar numeric funcall: {other:?}"),
    }
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
            serde_json::to_vec(&runtime.stable_version())?,
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

#[async_trait]
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
                version: self.stable_version(),
            }),
        }
    }
}

#[async_trait]
impl HyperdexClientService for ClusterRuntime {
    async fn handle(&self, request: ClientRequest) -> Result<ClientResponse> {
        match request {
            ClientRequest::Put {
                space,
                key,
                mutations,
            } => {
                let primary = self.route_primary(&key)?;
                if primary == self.local_node_id {
                    Ok(match self.data_plane.put(&space, key, &mutations)? {
                        WriteResult::Written => ClientResponse::Unit,
                        WriteResult::ConditionFailed => ClientResponse::ConditionFailed,
                        WriteResult::Missing => ClientResponse::Unit,
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
                            DataPlaneResponse::Record(_) => {
                                anyhow::bail!("unexpected record response to remote put")
                            }
                        },
                    )
                }
            }
            ClientRequest::Get { space, key } => {
                let primary = self.route_primary(&key)?;
                if primary == self.local_node_id {
                    Ok(ClientResponse::Record(self.data_plane.get(&space, &key)?))
                } else {
                    Ok(
                        match self
                            .forward_data_request(primary, DataPlaneRequest::Get { space, key })
                            .await?
                        {
                            DataPlaneResponse::Record(record) => ClientResponse::Record(record),
                            DataPlaneResponse::Unit | DataPlaneResponse::ConditionFailed => {
                                anyhow::bail!("unexpected write response to remote get")
                            }
                        },
                    )
                }
            }
            ClientRequest::Delete { space, key } => {
                Ok(match self.data_plane.delete(&space, &key)? {
                    WriteResult::Written | WriteResult::Missing => ClientResponse::Unit,
                    WriteResult::ConditionFailed => ClientResponse::ConditionFailed,
                })
            }
            ClientRequest::ConditionalPut {
                space,
                key,
                checks,
                mutations,
            } => Ok(
                match self
                    .data_plane
                    .conditional_put(&space, key, &checks, &mutations)?
                {
                    WriteResult::Written => ClientResponse::Unit,
                    WriteResult::ConditionFailed => ClientResponse::ConditionFailed,
                    WriteResult::Missing => ClientResponse::Unit,
                },
            ),
            ClientRequest::Search { space, checks } => Ok(ClientResponse::SearchResult(
                self.data_plane.search(&space, &checks)?,
            )),
            ClientRequest::Count { space, checks } => Ok(ClientResponse::Count(
                self.data_plane.count(&space, &checks)?,
            )),
            ClientRequest::DeleteGroup { space, checks } => Ok(ClientResponse::Deleted(
                self.data_plane.delete_matching(&space, &checks)?,
            )),
        }
    }
}

pub fn bootstrap_runtime() -> ClusterRuntime {
    ClusterRuntime::single_node(ClusterConfig::default()).expect("default cluster config is valid")
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
        consensus,
        placement,
        storage,
        internode_transport,
        ..
    } = mode
    {
        config.nodes = vec![daemon_registration_node(mode).expect("daemon mode has a node")];
        config.consensus = consensus.clone();
        config.placement = placement.clone();
        config.storage = storage.clone();
        config.internode_transport = internode_transport.clone();
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use cluster_config::{
        ClusterNode, ConsensusBackend, PlacementBackend, StorageBackend, TransportBackend,
    };
    use data_model::{Attribute, Check, Mutation, Predicate, Value};
    use hyperdex_admin_protocol::{
        AdminRequest, AdminResponse, ConfigView, CoordinatorAdminRequest, CoordinatorReturnCode,
        HyperdexAdminService, LegacyAdminRequest, LegacyAdminReturnCode,
    };
    use hyperdex_client_protocol::HyperdexClientService;
    use legacy_protocol::{
        AtomicRequest, AtomicResponse, CountRequest, CountResponse, GetRequest, GetResponse,
        GetValue, LegacyCheck, LegacyFuncall, LegacyFuncallName, LegacyMessageType,
        LegacyPredicate, LegacyReturnCode, RequestHeader, SearchContinueRequest,
        SearchDoneResponse, SearchItemResponse, SearchStartRequest,
        LEGACY_ATOMIC_FLAG_FAIL_IF_FOUND, LEGACY_ATOMIC_FLAG_WRITE,
    };

    #[test]
    fn runtime_uses_single_node_consensus_by_default() {
        let runtime = bootstrap_runtime();

        assert_eq!(runtime.consensus_backend_name(), "single-node");
        assert_eq!(runtime.placement_backend_name(), "hyperspace");
        assert_eq!(runtime.storage_backend_name(), "memory");
        assert_eq!(runtime.internode_transport_name(), "in-process");
    }

    #[cfg(not(feature = "omnipaxos"))]
    #[test]
    fn runtime_rejects_omnipaxos_when_feature_is_disabled() {
        let mut config = ClusterConfig::default();
        config.consensus = ConsensusBackend::OmniPaxos;
        let err = ClusterRuntime::single_node(config)
            .err()
            .expect("omnipaxos should be rejected without the feature")
            .to_string();
        assert!(err.contains("server feature `omnipaxos`"));
    }

    #[cfg(not(feature = "openraft"))]
    #[test]
    fn runtime_rejects_openraft_when_feature_is_disabled() {
        let mut config = ClusterConfig::default();
        config.consensus = ConsensusBackend::OpenRaft;
        let err = ClusterRuntime::single_node(config)
            .err()
            .expect("openraft should be rejected without the feature")
            .to_string();
        assert!(err.contains("server feature `openraft`"));
    }

    #[test]
    fn runtime_selects_mirror_consensus_from_config() {
        let mut config = ClusterConfig::default();
        config.consensus = ConsensusBackend::Mirror;

        let runtime = ClusterRuntime::single_node(config).unwrap();

        assert_eq!(runtime.consensus_backend_name(), "mirror");
    }

    #[test]
    fn runtime_selects_rendezvous_placement_from_config() {
        let mut config = ClusterConfig::default();
        config.placement = PlacementBackend::Rendezvous;

        let runtime = ClusterRuntime::single_node(config).unwrap();

        assert_eq!(runtime.placement_backend_name(), "rendezvous");
    }

    #[tokio::test]
    async fn runtime_selects_rocksdb_storage_from_config() {
        let mut config = ClusterConfig::default();
        config.storage = StorageBackend::RocksDb;

        let runtime = ClusterRuntime::single_node(config).unwrap();
        assert_eq!(runtime.storage_backend_name(), "rocksdb");

        HyperdexAdminService::handle(
            &runtime,
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();

        HyperdexClientService::handle(
            &runtime,
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from_static(b"ada"),
                mutations: vec![
                    Mutation::Set(Attribute {
                        name: "username".to_owned(),
                        value: Value::Bytes(Bytes::from_static(b"ada")),
                    }),
                    Mutation::Set(Attribute {
                        name: "first".to_owned(),
                        value: Value::String("Ada".to_owned()),
                    }),
                ],
            },
        )
        .await
        .unwrap();

        let record = HyperdexClientService::handle(
            &runtime,
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from_static(b"ada"),
            },
        )
        .await
        .unwrap();

        assert!(matches!(record, ClientResponse::Record(Some(_))));
    }

    #[test]
    fn runtime_selects_grpc_internode_transport_from_config() {
        let mut config = ClusterConfig::default();
        config.internode_transport = TransportBackend::Grpc;

        let runtime = ClusterRuntime::single_node(config).unwrap();

        assert_eq!(runtime.internode_transport_name(), "grpc");
    }

    #[test]
    fn runtime_rejects_missing_local_node() {
        let mut config = ClusterConfig::default();
        config.nodes = vec![
            ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: 1982,
                data_port: 2012,
            },
            ClusterNode {
                id: 2,
                host: "127.0.0.1".to_owned(),
                control_port: 1983,
                data_port: 2013,
            },
        ];

        let err = ClusterRuntime::for_node(config, 9)
            .err()
            .expect("missing local node should be rejected")
            .to_string();

        assert!(err.contains("local node 9"));
    }

    #[cfg(feature = "omnipaxos")]
    #[test]
    fn runtime_accepts_omnipaxos_when_feature_is_enabled() {
        let mut config = ClusterConfig::default();
        config.consensus = ConsensusBackend::OmniPaxos;

        let runtime = ClusterRuntime::single_node(config).unwrap();

        assert_eq!(runtime.consensus_backend_name(), "omnipaxos");
    }

    #[cfg(feature = "openraft")]
    #[test]
    fn runtime_accepts_openraft_when_feature_is_enabled() {
        let mut config = ClusterConfig::default();
        config.consensus = ConsensusBackend::OpenRaft;

        let runtime = ClusterRuntime::single_node(config).unwrap();

        assert_eq!(runtime.consensus_backend_name(), "openraft");
    }

    #[tokio::test]
    async fn runtime_accepts_hyperdex_dsl_schema() {
        let runtime = bootstrap_runtime();

        let response = HyperdexAdminService::handle(
            &runtime,
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();

        assert_eq!(response, AdminResponse::Unit);
        assert_eq!(
            HyperdexAdminService::handle(&runtime, AdminRequest::ListSpaces)
                .await
                .unwrap(),
            AdminResponse::Spaces(vec!["profiles".to_owned()])
        );
    }

    #[tokio::test]
    async fn runtime_dump_config_tracks_space_lifecycle_and_stability() {
        let mut config = ClusterConfig::default();
        config.consensus = ConsensusBackend::Mirror;
        config.placement = PlacementBackend::Rendezvous;
        config.storage = StorageBackend::Memory;
        config.internode_transport = TransportBackend::Grpc;
        let runtime = ClusterRuntime::single_node(config.clone()).unwrap();

        let response = HyperdexAdminService::handle(&runtime, AdminRequest::DumpConfig)
            .await
            .unwrap();
        assert_eq!(
            response,
            AdminResponse::Config(ConfigView {
                version: 0,
                stable_through: 0,
                cluster: config.clone(),
                spaces: Vec::new(),
            })
        );

        HyperdexAdminService::handle(
            &runtime,
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();

        let response = HyperdexAdminService::handle(&runtime, AdminRequest::DumpConfig)
            .await
            .unwrap();
        let AdminResponse::Config(config_view) = response else {
            panic!("expected config response after create");
        };
        assert_eq!(config_view.version, 1);
        assert_eq!(config_view.stable_through, 1);
        assert_eq!(config_view.cluster, config);
        assert_eq!(config_view.spaces.len(), 1);
        assert_eq!(config_view.spaces[0].name, "profiles");
        assert_eq!(
            HyperdexAdminService::handle(&runtime, AdminRequest::WaitUntilStable)
                .await
                .unwrap(),
            AdminResponse::Stable { version: 1 }
        );

        HyperdexAdminService::handle(&runtime, AdminRequest::DropSpace("profiles".to_owned()))
            .await
            .unwrap();

        let response = HyperdexAdminService::handle(&runtime, AdminRequest::DumpConfig)
            .await
            .unwrap();
        assert_eq!(
            response,
            AdminResponse::Config(ConfigView {
                version: 2,
                stable_through: 2,
                cluster: config,
                spaces: Vec::new(),
            })
        );
        assert_eq!(
            HyperdexAdminService::handle(&runtime, AdminRequest::WaitUntilStable)
                .await
                .unwrap(),
            AdminResponse::Stable { version: 2 }
        );
    }

    #[tokio::test]
    async fn runtime_register_daemon_updates_config_and_layout() {
        let runtime = ClusterRuntime::single_node(coordinator_cluster_config()).unwrap();

        assert_eq!(runtime.catalog.layout().unwrap().nodes, Vec::<u64>::new());

        HyperdexAdminService::handle(
            &runtime,
            AdminRequest::RegisterDaemon(ClusterNode {
                id: 4,
                host: "10.0.0.4".to_owned(),
                control_port: 2982,
                data_port: 3012,
            }),
        )
        .await
        .unwrap();
        HyperdexAdminService::handle(
            &runtime,
            AdminRequest::RegisterDaemon(ClusterNode {
                id: 9,
                host: "10.0.0.9".to_owned(),
                control_port: 3982,
                data_port: 4012,
            }),
        )
        .await
        .unwrap();

        assert_eq!(runtime.catalog.layout().unwrap().nodes, vec![4, 9]);

        let AdminResponse::Config(config_view) =
            HyperdexAdminService::handle(&runtime, AdminRequest::DumpConfig)
                .await
                .unwrap()
        else {
            panic!("expected config response after daemon registration");
        };
        assert_eq!(config_view.version, 2);
        assert_eq!(config_view.stable_through, 2);
        assert_eq!(
            config_view.cluster.nodes,
            vec![
                ClusterNode {
                    id: 4,
                    host: "10.0.0.4".to_owned(),
                    control_port: 2982,
                    data_port: 3012,
                },
                ClusterNode {
                    id: 9,
                    host: "10.0.0.9".to_owned(),
                    control_port: 3982,
                    data_port: 4012,
                },
            ]
        );
    }

    #[tokio::test]
    async fn legacy_admin_space_add_success_maps_to_hyperdex_status() {
        let runtime = bootstrap_runtime();

        let status = handle_legacy_admin_request(
            &runtime,
            LegacyAdminRequest::SpaceAddDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();

        assert_eq!(status, LegacyAdminReturnCode::Success);
    }

    #[tokio::test]
    async fn legacy_admin_space_add_duplicate_maps_to_hyperdex_status() {
        let runtime = bootstrap_runtime();
        let request = LegacyAdminRequest::SpaceAddDsl(
            "space profiles\n\
             key username\n\
             attributes\n\
                string first,\n\
                int profile_views\n\
             tolerate 0 failures\n"
                .to_owned(),
        );

        assert_eq!(
            handle_legacy_admin_request(&runtime, request.clone())
                .await
                .unwrap(),
            LegacyAdminReturnCode::Success
        );
        assert_eq!(
            handle_legacy_admin_request(&runtime, request)
                .await
                .unwrap(),
            LegacyAdminReturnCode::Duplicate
        );
    }

    #[tokio::test]
    async fn legacy_admin_space_add_bad_schema_maps_to_badspace() {
        let runtime = bootstrap_runtime();

        let status = handle_legacy_admin_request(
            &runtime,
            LegacyAdminRequest::SpaceAddDsl("space broken".to_owned()),
        )
        .await
        .unwrap();

        assert_eq!(status, LegacyAdminReturnCode::BadSpace);
    }

    #[tokio::test]
    async fn legacy_admin_space_rm_missing_maps_to_notfound() {
        let runtime = bootstrap_runtime();

        let status = handle_legacy_admin_request(
            &runtime,
            LegacyAdminRequest::SpaceRm("profiles".to_owned()),
        )
        .await
        .unwrap();

        assert_eq!(status, LegacyAdminReturnCode::NotFound);
    }

    #[tokio::test]
    async fn coordinator_admin_space_rm_maps_to_exact_coordinator_code() {
        let runtime = bootstrap_runtime();

        let status = handle_coordinator_admin_request(
            &runtime,
            CoordinatorAdminRequest::SpaceRm("profiles".to_owned()),
        )
        .await
        .unwrap();

        assert_eq!(status, CoordinatorReturnCode::NotFound);
        assert_eq!(
            CoordinatorReturnCode::decode(&status.encode()).unwrap(),
            CoordinatorReturnCode::NotFound
        );
    }

    #[tokio::test]
    async fn coordinator_admin_method_dispatch_returns_wire_bytes() {
        let runtime = bootstrap_runtime();

        let bytes = handle_coordinator_admin_method(
            &runtime,
            "space_rm",
            CoordinatorAdminRequest::SpaceRm("profiles".to_owned()),
        )
        .await
        .unwrap();
        assert_eq!(
            CoordinatorReturnCode::decode(&bytes).unwrap(),
            CoordinatorReturnCode::NotFound
        );

        let malformed = handle_coordinator_admin_method(
            &runtime,
            "space_rm",
            CoordinatorAdminRequest::SpaceAdd(
                parse_hyperdex_space(
                    "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first\n\
                 tolerate 0 failures\n",
                )
                .unwrap(),
            ),
        )
        .await
        .unwrap();
        assert_eq!(
            CoordinatorReturnCode::decode(&malformed).unwrap(),
            CoordinatorReturnCode::Malformed
        );
    }

    #[tokio::test]
    async fn coordinator_control_service_routes_space_add_over_tcp() {
        let runtime = Arc::new(bootstrap_runtime());
        let service = CoordinatorControlService::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let address = service.local_addr().unwrap();

        let server = tokio::spawn(async move {
            service
                .serve_once_with(move |method, request| {
                    let runtime = runtime.clone();
                    async move {
                        handle_coordinator_control_method(runtime.as_ref(), &method, request).await
                    }
                })
                .await
                .unwrap()
        });

        let response = request_coordinator_control_once(
            address,
            "space_add",
            &CoordinatorAdminRequest::SpaceAdd(
                parse_hyperdex_space(
                    "space profiles\n\
                     key username\n\
                     attributes\n\
                        string first,\n\
                        int profile_views\n\
                     tolerate 0 failures\n",
                )
                .unwrap(),
            ),
        )
        .await
        .unwrap();

        server.await.unwrap();
        assert_eq!(
            CoordinatorReturnCode::decode(&response).unwrap(),
            CoordinatorReturnCode::Success
        );
    }

    #[tokio::test]
    async fn coordinator_control_service_registers_multiple_daemons_over_tcp() {
        let runtime = Arc::new(ClusterRuntime::single_node(coordinator_cluster_config()).unwrap());

        for node in [
            ClusterNode {
                id: 2,
                host: "10.0.0.2".to_owned(),
                control_port: 2982,
                data_port: 3012,
            },
            ClusterNode {
                id: 8,
                host: "10.0.0.8".to_owned(),
                control_port: 3982,
                data_port: 4012,
            },
        ] {
            let service = CoordinatorControlService::bind("127.0.0.1:0".parse().unwrap())
                .await
                .unwrap();
            let address = service.local_addr().unwrap();
            let runtime_for_server = runtime.clone();

            let server = tokio::spawn(async move {
                service
                    .serve_once_with(move |method, request| {
                        let runtime = runtime_for_server.clone();
                        async move {
                            handle_coordinator_control_method(runtime.as_ref(), &method, request)
                                .await
                        }
                    })
                    .await
                    .unwrap()
            });

            let response = request_coordinator_control_once(
                address,
                "daemon_register",
                &CoordinatorAdminRequest::DaemonRegister(node),
            )
            .await
            .unwrap();

            server.await.unwrap();
            assert_eq!(
                CoordinatorReturnCode::decode(&response).unwrap(),
                CoordinatorReturnCode::Success
            );
        }

        let AdminResponse::Config(config_view) =
            HyperdexAdminService::handle(runtime.as_ref(), AdminRequest::DumpConfig)
                .await
                .unwrap()
        else {
            panic!("expected config response after daemon registration");
        };
        assert_eq!(config_view.version, 2);
        assert_eq!(
            config_view
                .cluster
                .nodes
                .iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![2, 8]
        );
        assert_eq!(runtime.catalog.layout().unwrap().nodes, vec![2, 8]);
    }

    #[tokio::test]
    async fn coordinator_control_service_returns_malformed_for_method_request_mismatch() {
        let runtime = Arc::new(bootstrap_runtime());
        let service = CoordinatorControlService::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let address = service.local_addr().unwrap();

        let server = tokio::spawn(async move {
            service
                .serve_once_with(move |method, request| {
                    let runtime = runtime.clone();
                    async move {
                        handle_coordinator_control_method(runtime.as_ref(), &method, request).await
                    }
                })
                .await
                .unwrap()
        });

        let response = request_coordinator_control_once(
            address,
            "space_rm",
            &CoordinatorAdminRequest::SpaceAdd(
                parse_hyperdex_space(
                    "space profiles\n\
                     key username\n\
                     attributes\n\
                        string first,\n\
                        int profile_views\n\
                     tolerate 0 failures\n",
                )
                .unwrap(),
            ),
        )
        .await
        .unwrap();

        server.await.unwrap();
        assert_eq!(
            CoordinatorReturnCode::decode(&response).unwrap(),
            CoordinatorReturnCode::Malformed
        );
    }

    #[tokio::test]
    async fn coordinator_control_service_wait_until_stable_returns_version_body() {
        let runtime = Arc::new(bootstrap_runtime());
        HyperdexAdminService::handle(
            runtime.as_ref(),
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();
        let service = CoordinatorControlService::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let address = service.local_addr().unwrap();

        let server = tokio::spawn(async move {
            service
                .serve_once_with(move |method, request| {
                    let runtime = runtime.clone();
                    async move {
                        handle_coordinator_control_method(runtime.as_ref(), &method, request).await
                    }
                })
                .await
                .unwrap()
        });

        let response = request_coordinator_control_with_body_once(
            address,
            "wait_until_stable",
            &CoordinatorAdminRequest::WaitUntilStable,
        )
        .await
        .unwrap();

        server.await.unwrap();
        assert_eq!(
            CoordinatorReturnCode::decode(&response.status).unwrap(),
            CoordinatorReturnCode::Success
        );
        let version: u64 = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(version, 1);
    }

    #[tokio::test]
    async fn coordinator_control_service_config_get_returns_config_snapshot() {
        let runtime = Arc::new(bootstrap_runtime());
        HyperdexAdminService::handle(
            runtime.as_ref(),
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();
        let service = CoordinatorControlService::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let address = service.local_addr().unwrap();

        let server = tokio::spawn(async move {
            service
                .serve_once_with(move |method, request| {
                    let runtime = runtime.clone();
                    async move {
                        handle_coordinator_control_method(runtime.as_ref(), &method, request).await
                    }
                })
                .await
                .unwrap()
        });

        let response = request_coordinator_control_with_body_once(
            address,
            "config_get",
            &CoordinatorAdminRequest::ConfigGet,
        )
        .await
        .unwrap();

        server.await.unwrap();
        assert_eq!(
            CoordinatorReturnCode::decode(&response.status).unwrap(),
            CoordinatorReturnCode::Success
        );
        let config_view: ConfigView = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(config_view.version, 1);
        assert_eq!(config_view.stable_through, 1);
        assert_eq!(config_view.spaces.len(), 1);
        assert_eq!(config_view.spaces[0].name, "profiles");
    }

    #[tokio::test]
    async fn runtime_supports_put_get_count_and_delete_group() {
        let runtime = bootstrap_runtime();
        HyperdexAdminService::handle(
            &runtime,
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();

        HyperdexClientService::handle(
            &runtime,
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from_static(b"ada"),
                mutations: vec![
                    Mutation::Set(Attribute {
                        name: "username".to_owned(),
                        value: Value::Bytes(Bytes::from_static(b"ada")),
                    }),
                    Mutation::Set(Attribute {
                        name: "first".to_owned(),
                        value: Value::String("Ada".to_owned()),
                    }),
                    Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(5),
                    }),
                ],
            },
        )
        .await
        .unwrap();

        let record = HyperdexClientService::handle(
            &runtime,
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from_static(b"ada"),
            },
        )
        .await
        .unwrap();
        assert!(matches!(record, ClientResponse::Record(Some(_))));

        let count = HyperdexClientService::handle(
            &runtime,
            ClientRequest::Count {
                space: "profiles".to_owned(),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::GreaterThanOrEqual,
                    value: Value::Int(5),
                }],
            },
        )
        .await
        .unwrap();
        assert_eq!(count, ClientResponse::Count(1));

        let deleted = HyperdexClientService::handle(
            &runtime,
            ClientRequest::DeleteGroup {
                space: "profiles".to_owned(),
                checks: vec![Check {
                    attribute: "first".to_owned(),
                    predicate: Predicate::Equal,
                    value: Value::String("Ada".to_owned()),
                }],
            },
        )
        .await
        .unwrap();
        assert_eq!(deleted, ClientResponse::Deleted(1));
    }

    #[tokio::test]
    async fn legacy_get_returns_record_attributes() {
        let runtime = bootstrap_runtime();
        HyperdexAdminService::handle(
            &runtime,
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();

        HyperdexClientService::handle(
            &runtime,
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from_static(b"ada"),
                mutations: vec![
                    Mutation::Set(Attribute {
                        name: "username".to_owned(),
                        value: Value::Bytes(Bytes::from_static(b"ada")),
                    }),
                    Mutation::Set(Attribute {
                        name: "first".to_owned(),
                        value: Value::String("Ada".to_owned()),
                    }),
                    Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(5),
                    }),
                ],
            },
        )
        .await
        .unwrap();

        let (header, body) = handle_legacy_request(
            &runtime,
            RequestHeader {
                message_type: LegacyMessageType::ReqGet,
                flags: 0,
                version: 1,
                target_virtual_server: 11,
                nonce: 19,
            },
            &GetRequest {
                key: b"ada".to_vec(),
            }
            .encode_body(),
        )
        .await
        .unwrap();

        assert_eq!(header.message_type, LegacyMessageType::RespGet);
        let response = GetResponse::decode_body(&body).unwrap();
        assert_eq!(response.status, LegacyReturnCode::Success);
        assert!(response.attributes.iter().any(|attr| {
            attr.name == "first" && attr.value == GetValue::String("Ada".to_owned())
        }));
    }

    #[tokio::test]
    async fn legacy_atomic_put_stores_record_attributes() {
        let runtime = bootstrap_runtime();
        HyperdexAdminService::handle(
            &runtime,
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();

        let (header, body) = handle_legacy_request(
            &runtime,
            RequestHeader {
                message_type: LegacyMessageType::ReqAtomic,
                flags: 0,
                version: 1,
                target_virtual_server: 11,
                nonce: 19,
            },
            &AtomicRequest {
                flags: LEGACY_ATOMIC_FLAG_WRITE,
                key: b"ada".to_vec(),
                checks: Vec::new(),
                funcalls: vec![
                    LegacyFuncall {
                        attribute: "first".to_owned(),
                        name: LegacyFuncallName::Set,
                        arg1: GetValue::String("Ada".to_owned()),
                        arg2: None,
                    },
                    LegacyFuncall {
                        attribute: "profile_views".to_owned(),
                        name: LegacyFuncallName::Set,
                        arg1: GetValue::Int(5),
                        arg2: None,
                    },
                ],
            }
            .encode_body(),
        )
        .await
        .unwrap();

        assert_eq!(header.message_type, LegacyMessageType::RespAtomic);
        assert_eq!(
            AtomicResponse::decode_body(&body).unwrap().status,
            LegacyReturnCode::Success
        );

        let response = HyperdexClientService::handle(
            &runtime,
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from_static(b"ada"),
            },
        )
        .await
        .unwrap();

        let ClientResponse::Record(Some(record)) = response else {
            panic!("expected stored record");
        };

        assert_eq!(
            record.attributes.get("first"),
            Some(&Value::String("Ada".to_owned()))
        );
        assert_eq!(record.attributes.get("profile_views"), Some(&Value::Int(5)));
    }

    #[tokio::test]
    async fn legacy_atomic_respects_fail_if_found() {
        let runtime = bootstrap_runtime();
        HyperdexAdminService::handle(
            &runtime,
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();

        HyperdexClientService::handle(
            &runtime,
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from_static(b"ada"),
                mutations: vec![Mutation::Set(Attribute {
                    name: "first".to_owned(),
                    value: Value::String("Ada".to_owned()),
                })],
            },
        )
        .await
        .unwrap();

        let (_, body) = handle_legacy_request(
            &runtime,
            RequestHeader {
                message_type: LegacyMessageType::ReqAtomic,
                flags: 0,
                version: 1,
                target_virtual_server: 11,
                nonce: 19,
            },
            &AtomicRequest {
                flags: LEGACY_ATOMIC_FLAG_WRITE | LEGACY_ATOMIC_FLAG_FAIL_IF_FOUND,
                key: b"ada".to_vec(),
                checks: Vec::new(),
                funcalls: vec![LegacyFuncall {
                    attribute: "first".to_owned(),
                    name: LegacyFuncallName::Set,
                    arg1: GetValue::String("Grace".to_owned()),
                    arg2: None,
                }],
            }
            .encode_body(),
        )
        .await
        .unwrap();

        assert_eq!(
            AtomicResponse::decode_body(&body).unwrap().status,
            LegacyReturnCode::CompareFailed
        );
    }

    #[tokio::test]
    async fn legacy_atomic_checks_map_to_conditional_put() {
        let runtime = bootstrap_runtime();
        HyperdexAdminService::handle(
            &runtime,
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();

        HyperdexClientService::handle(
            &runtime,
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from_static(b"ada"),
                mutations: vec![
                    Mutation::Set(Attribute {
                        name: "first".to_owned(),
                        value: Value::String("Ada".to_owned()),
                    }),
                    Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(2),
                    }),
                ],
            },
        )
        .await
        .unwrap();

        let (_, body) = handle_legacy_request(
            &runtime,
            RequestHeader {
                message_type: LegacyMessageType::ReqAtomic,
                flags: 0,
                version: 1,
                target_virtual_server: 11,
                nonce: 19,
            },
            &AtomicRequest {
                flags: LEGACY_ATOMIC_FLAG_WRITE,
                key: b"ada".to_vec(),
                checks: vec![LegacyCheck {
                    attribute: "profile_views".to_owned(),
                    predicate: LegacyPredicate::GreaterThanOrEqual,
                    value: GetValue::Int(5),
                }],
                funcalls: vec![LegacyFuncall {
                    attribute: "first".to_owned(),
                    name: LegacyFuncallName::Set,
                    arg1: GetValue::String("Grace".to_owned()),
                    arg2: None,
                }],
            }
            .encode_body(),
        )
        .await
        .unwrap();

        assert_eq!(
            AtomicResponse::decode_body(&body).unwrap().status,
            LegacyReturnCode::CompareFailed
        );

        let response = HyperdexClientService::handle(
            &runtime,
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from_static(b"ada"),
            },
        )
        .await
        .unwrap();

        let ClientResponse::Record(Some(record)) = response else {
            panic!("expected stored record");
        };

        assert_eq!(
            record.attributes.get("first"),
            Some(&Value::String("Ada".to_owned()))
        );
    }

    #[tokio::test]
    async fn legacy_atomic_numeric_funcall_updates_record() {
        let runtime = bootstrap_runtime();
        HyperdexAdminService::handle(
            &runtime,
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    int profile_views\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();

        HyperdexClientService::handle(
            &runtime,
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from_static(b"ada"),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(2),
                })],
            },
        )
        .await
        .unwrap();

        let (_, body) = handle_legacy_request(
            &runtime,
            RequestHeader {
                message_type: LegacyMessageType::ReqAtomic,
                flags: 0,
                version: 1,
                target_virtual_server: 11,
                nonce: 19,
            },
            &AtomicRequest {
                flags: LEGACY_ATOMIC_FLAG_WRITE,
                key: b"ada".to_vec(),
                checks: Vec::new(),
                funcalls: vec![LegacyFuncall {
                    attribute: "profile_views".to_owned(),
                    name: LegacyFuncallName::NumAdd,
                    arg1: GetValue::Int(3),
                    arg2: None,
                }],
            }
            .encode_body(),
        )
        .await
        .unwrap();

        assert_eq!(
            AtomicResponse::decode_body(&body).unwrap().status,
            LegacyReturnCode::Success
        );

        let response = HyperdexClientService::handle(
            &runtime,
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from_static(b"ada"),
            },
        )
        .await
        .unwrap();

        let ClientResponse::Record(Some(record)) = response else {
            panic!("expected stored record");
        };

        assert_eq!(record.attributes.get("profile_views"), Some(&Value::Int(5)));
    }

    #[tokio::test]
    async fn legacy_count_returns_runtime_count() {
        let runtime = bootstrap_runtime();
        HyperdexAdminService::handle(
            &runtime,
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();

        let (header, body) = handle_legacy_request(
            &runtime,
            RequestHeader {
                message_type: LegacyMessageType::ReqCount,
                flags: 0,
                version: 1,
                target_virtual_server: 11,
                nonce: 19,
            },
            &CountRequest {
                space: "profiles".to_owned(),
            }
            .encode_body(),
        )
        .await
        .unwrap();

        assert_eq!(header.message_type, LegacyMessageType::RespCount);
        assert_eq!(CountResponse::decode_body(&body).unwrap().count, 0);
    }

    #[tokio::test]
    async fn legacy_search_start_returns_first_matching_record() {
        let runtime = bootstrap_runtime();
        HyperdexAdminService::handle(
            &runtime,
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();

        for (key, first, views) in [("ada", "Ada", 5), ("grace", "Grace", 3), ("eve", "Eve", 1)] {
            HyperdexClientService::handle(
                &runtime,
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::copy_from_slice(key.as_bytes()),
                    mutations: vec![
                        Mutation::Set(Attribute {
                            name: "first".to_owned(),
                            value: Value::String(first.to_owned()),
                        }),
                        Mutation::Set(Attribute {
                            name: "profile_views".to_owned(),
                            value: Value::Int(views),
                        }),
                    ],
                },
            )
            .await
            .unwrap();
        }

        let (header, body) = handle_legacy_request(
            &runtime,
            RequestHeader {
                message_type: LegacyMessageType::ReqSearchStart,
                flags: 0,
                version: 1,
                target_virtual_server: 11,
                nonce: 19,
            },
            &SearchStartRequest {
                space: "profiles".to_owned(),
                search_id: 41,
                checks: vec![LegacyCheck {
                    attribute: "profile_views".to_owned(),
                    predicate: LegacyPredicate::GreaterThanOrEqual,
                    value: GetValue::Int(3),
                }],
            }
            .encode_body(),
        )
        .await
        .unwrap();

        assert_eq!(header.message_type, LegacyMessageType::RespSearchItem);
        let item = SearchItemResponse::decode_body(&body).unwrap();
        assert_eq!(item.search_id, 41);
        assert_eq!(item.key, b"ada".to_vec());
    }

    #[tokio::test]
    async fn legacy_search_next_drains_cursor_then_returns_done() {
        let runtime = bootstrap_runtime();
        HyperdexAdminService::handle(
            &runtime,
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();

        for (key, first) in [("ada", "Ada"), ("grace", "Grace")] {
            HyperdexClientService::handle(
                &runtime,
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::copy_from_slice(key.as_bytes()),
                    mutations: vec![Mutation::Set(Attribute {
                        name: "first".to_owned(),
                        value: Value::String(first.to_owned()),
                    })],
                },
            )
            .await
            .unwrap();
        }

        let _ = handle_legacy_request(
            &runtime,
            RequestHeader {
                message_type: LegacyMessageType::ReqSearchStart,
                flags: 0,
                version: 1,
                target_virtual_server: 11,
                nonce: 19,
            },
            &SearchStartRequest {
                space: "profiles".to_owned(),
                search_id: 99,
                checks: Vec::new(),
            }
            .encode_body(),
        )
        .await
        .unwrap();

        let (header, body) = handle_legacy_request(
            &runtime,
            RequestHeader {
                message_type: LegacyMessageType::ReqSearchNext,
                flags: 0,
                version: 1,
                target_virtual_server: 11,
                nonce: 20,
            },
            &SearchContinueRequest { search_id: 99 }.encode_body(),
        )
        .await
        .unwrap();
        assert_eq!(header.message_type, LegacyMessageType::RespSearchItem);
        assert_eq!(
            SearchItemResponse::decode_body(&body).unwrap().key,
            b"grace".to_vec()
        );

        let (header, body) = handle_legacy_request(
            &runtime,
            RequestHeader {
                message_type: LegacyMessageType::ReqSearchNext,
                flags: 0,
                version: 1,
                target_virtual_server: 11,
                nonce: 21,
            },
            &SearchContinueRequest { search_id: 99 }.encode_body(),
        )
        .await
        .unwrap();
        assert_eq!(header.message_type, LegacyMessageType::RespSearchDone);
        assert_eq!(
            SearchDoneResponse::decode_body(&body).unwrap().search_id,
            99
        );
    }

    #[test]
    fn parse_coordinator_cli() {
        let args = vec![
            "coordinator".to_owned(),
            "--foreground".to_owned(),
            "--data=/tmp/coordinator".to_owned(),
            "--listen=127.0.0.1".to_owned(),
            "--listen-port=1982".to_owned(),
        ];

        assert_eq!(
            parse_process_mode(&args).unwrap(),
            ProcessMode::Coordinator {
                data_dir: "/tmp/coordinator".to_owned(),
                listen_host: "127.0.0.1".to_owned(),
                listen_port: 1982,
            }
        );
    }

    #[test]
    fn parse_daemon_cli() {
        let args = vec![
            "daemon".to_owned(),
            "--foreground".to_owned(),
            "--node-id=7".to_owned(),
            "--threads=1".to_owned(),
            "--data=/tmp/daemon".to_owned(),
            "--listen=127.0.0.1".to_owned(),
            "--listen-port=2012".to_owned(),
            "--coordinator=127.0.0.1".to_owned(),
            "--coordinator-port=1982".to_owned(),
        ];

        assert_eq!(
            parse_process_mode(&args).unwrap(),
            ProcessMode::Daemon {
                node_id: 7,
                threads: 1,
                data_dir: "/tmp/daemon".to_owned(),
                listen_host: "127.0.0.1".to_owned(),
                listen_port: 2012,
                control_port: 2012,
                coordinator_host: "127.0.0.1".to_owned(),
                coordinator_port: 1982,
                consensus: ConsensusBackend::SingleNode,
                placement: PlacementBackend::Hyperspace,
                storage: StorageBackend::Memory,
                internode_transport: TransportBackend::InProcess,
            }
        );
    }

    #[test]
    fn parse_daemon_cli_with_runtime_shape() {
        let args = vec![
            "daemon".to_owned(),
            "--node-id=7".to_owned(),
            "--threads=1".to_owned(),
            "--data=/tmp/daemon".to_owned(),
            "--listen=127.0.0.1".to_owned(),
            "--listen-port=2012".to_owned(),
            "--control-port=3012".to_owned(),
            "--coordinator=127.0.0.1".to_owned(),
            "--coordinator-port=1982".to_owned(),
            "--consensus=mirror".to_owned(),
            "--placement=rendezvous".to_owned(),
            "--storage=rocksdb".to_owned(),
            "--transport=grpc".to_owned(),
        ];

        assert_eq!(
            parse_process_mode(&args).unwrap(),
            ProcessMode::Daemon {
                node_id: 7,
                threads: 1,
                data_dir: "/tmp/daemon".to_owned(),
                listen_host: "127.0.0.1".to_owned(),
                listen_port: 2012,
                control_port: 3012,
                coordinator_host: "127.0.0.1".to_owned(),
                coordinator_port: 1982,
                consensus: ConsensusBackend::Mirror,
                placement: PlacementBackend::Rendezvous,
                storage: StorageBackend::RocksDb,
                internode_transport: TransportBackend::Grpc,
            }
        );
    }

    #[test]
    fn daemon_cluster_config_uses_daemon_identity() {
        let mode = ProcessMode::Daemon {
            node_id: 11,
            threads: 1,
            data_dir: "/tmp/daemon".to_owned(),
            listen_host: "10.0.0.11".to_owned(),
            listen_port: 2012,
            control_port: 3012,
            coordinator_host: "127.0.0.1".to_owned(),
            coordinator_port: 1982,
            consensus: ConsensusBackend::Mirror,
            placement: PlacementBackend::Rendezvous,
            storage: StorageBackend::Memory,
            internode_transport: TransportBackend::Grpc,
        };

        assert_eq!(
            daemon_cluster_config(&mode).nodes,
            vec![ClusterNode {
                id: 11,
                host: "10.0.0.11".to_owned(),
                control_port: 3012,
                data_port: 2012,
            }]
        );
    }
}
