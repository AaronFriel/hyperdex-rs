use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use cluster_config::{
    ClusterConfig, ConsensusBackend, PlacementBackend, StorageBackend, TransportBackend,
};
use control_plane::{Catalog, InMemoryCatalog};
use data_model::{
    parse_hyperdex_space, Attribute, Check, Mutation, NumericOp, Predicate, Space, Value,
};
use data_plane::DataPlane;
use engine_memory::MemoryEngine;
use engine_rocks::RocksEngine;
use hyperdex_admin_protocol::{AdminRequest, AdminResponse, HyperdexAdminService};
use hyperdex_client_protocol::{ClientRequest, ClientResponse, HyperdexClientService};
use legacy_protocol::{
    config_mismatch_response, AtomicRequest, AtomicResponse, CountRequest, CountResponse,
    GetAttribute, GetRequest, GetResponse, GetValue, LegacyCheck, LegacyFuncall,
    LegacyFuncallName, LegacyMessageType, LegacyPredicate, LegacyReturnCode, RequestHeader,
    ResponseHeader, LEGACY_ATOMIC_FLAG_FAIL_IF_FOUND, LEGACY_ATOMIC_FLAG_FAIL_IF_NOT_FOUND,
    LEGACY_ATOMIC_FLAG_WRITE,
};
use placement_core::{HyperSpacePlacement, PlacementStrategy, RendezvousPlacement};
use storage_core::{StorageEngine, WriteResult};
use tempfile::TempDir;
use transport_core::InProcessTransport;

pub struct ClusterRuntime {
    catalog: Arc<dyn Catalog>,
    storage: Arc<dyn StorageEngine>,
    data_plane: DataPlane,
    consensus: ConsensusRuntime,
    placement: PlacementRuntime,
    storage_backend: StorageRuntime,
    internode_transport: TransportRuntime,
    _ephemeral_storage_dir: Option<TempDir>,
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

impl ClusterRuntime {
    pub fn new(
        catalog: Arc<dyn Catalog>,
        storage: Arc<dyn StorageEngine>,
        placement: Arc<dyn PlacementStrategy>,
        consensus: ConsensusRuntime,
        placement_runtime: PlacementRuntime,
        storage_backend: StorageRuntime,
        internode_transport: TransportRuntime,
        ephemeral_storage_dir: Option<TempDir>,
    ) -> Self {
        let data_plane = DataPlane::new(catalog.clone(), storage.clone(), placement);
        Self {
            catalog,
            storage,
            data_plane,
            consensus,
            placement: placement_runtime,
            storage_backend,
            internode_transport,
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
        let catalog: Arc<dyn Catalog> =
            Arc::new(InMemoryCatalog::new(config.nodes.clone(), config.replicas));
        let consensus = select_consensus_backend(&config)?;
        let (placement, placement_runtime) = select_placement_backend(&config);
        let (storage, storage_backend, ephemeral_storage_dir) =
            select_storage_backend(&config, data_dir)?;
        let internode_transport = select_internode_transport(&config);

        Ok(Self::new(
            catalog,
            storage,
            placement,
            consensus,
            placement_runtime,
            storage_backend,
            internode_transport,
            ephemeral_storage_dir,
        ))
    }

    fn create_space(&self, space: Space) -> Result<()> {
        self.storage.create_space(space.name.clone())?;
        self.catalog.create_space(space)?;
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

    fn only_space_name(&self) -> Result<String> {
        let spaces = self.catalog.list_spaces()?;
        match spaces.as_slice() {
            [space] => Ok(space.clone()),
            [] => Err(anyhow!("legacy request handling requires one created space")),
            _ => Err(anyhow!(
                "legacy request handling is ambiguous with multiple spaces"
            )),
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
        PlacementBackend::Rendezvous => (
            Arc::new(RendezvousPlacement),
            PlacementRuntime::Rendezvous,
        ),
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

#[async_trait]
impl HyperdexAdminService for ClusterRuntime {
    async fn handle(&self, request: AdminRequest) -> Result<AdminResponse> {
        match request {
            AdminRequest::CreateSpace(space) => {
                self.create_space(space)?;
                Ok(AdminResponse::Unit)
            }
            AdminRequest::CreateSpaceDsl(schema) => {
                self.create_space(parse_hyperdex_space(&schema)?)?;
                Ok(AdminResponse::Unit)
            }
            AdminRequest::DropSpace(space) => {
                self.catalog.drop_space(&space)?;
                self.storage.drop_space(&space)?;
                Ok(AdminResponse::Unit)
            }
            AdminRequest::ListSpaces => Ok(AdminResponse::Spaces(self.catalog.list_spaces()?)),
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
            } => Ok(match self.data_plane.put(&space, key, &mutations)? {
                WriteResult::Written => ClientResponse::Unit,
                WriteResult::ConditionFailed => ClientResponse::ConditionFailed,
                WriteResult::Missing => ClientResponse::Unit,
            }),
            ClientRequest::Get { space, key } => {
                Ok(ClientResponse::Record(self.data_plane.get(&space, &key)?))
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessMode {
    Coordinator {
        data_dir: String,
        listen_host: String,
        listen_port: u16,
    },
    Daemon {
        threads: usize,
        data_dir: String,
        listen_host: String,
        listen_port: u16,
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
        "daemon" => Ok(ProcessMode::Daemon {
            threads: required_option(args, "--threads")?.parse()?,
            data_dir: required_option(args, "--data")?,
            listen_host: required_option(args, "--listen")?,
            listen_port: required_option(args, "--listen-port")?.parse()?,
            coordinator_host: required_option(args, "--coordinator")?,
            coordinator_port: required_option(args, "--coordinator-port")?.parse()?,
            consensus: optional_consensus_backend(args)?,
            placement: optional_placement_backend(args)?,
            storage: optional_storage_backend(args)?,
            internode_transport: optional_transport_backend(args)?,
        }),
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
    use cluster_config::{ConsensusBackend, PlacementBackend, StorageBackend, TransportBackend};
    use data_model::{Attribute, Check, Mutation, Predicate, Value};
    use hyperdex_admin_protocol::HyperdexAdminService;
    use hyperdex_client_protocol::HyperdexClientService;
    use legacy_protocol::{
        AtomicRequest, AtomicResponse, CountRequest, CountResponse, GetRequest, GetResponse,
        GetValue, LegacyCheck, LegacyFuncall, LegacyFuncallName, LegacyMessageType,
        LegacyPredicate, LegacyReturnCode, RequestHeader, LEGACY_ATOMIC_FLAG_FAIL_IF_FOUND,
        LEGACY_ATOMIC_FLAG_WRITE,
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
                threads: 1,
                data_dir: "/tmp/daemon".to_owned(),
                listen_host: "127.0.0.1".to_owned(),
                listen_port: 2012,
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
            "--threads=1".to_owned(),
            "--data=/tmp/daemon".to_owned(),
            "--listen=127.0.0.1".to_owned(),
            "--listen-port=2012".to_owned(),
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
                threads: 1,
                data_dir: "/tmp/daemon".to_owned(),
                listen_host: "127.0.0.1".to_owned(),
                listen_port: 2012,
                coordinator_host: "127.0.0.1".to_owned(),
                coordinator_port: 1982,
                consensus: ConsensusBackend::Mirror,
                placement: PlacementBackend::Rendezvous,
                storage: StorageBackend::RocksDb,
                internode_transport: TransportBackend::Grpc,
            }
        );
    }
}
