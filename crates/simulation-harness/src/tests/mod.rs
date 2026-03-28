use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bytes::Bytes;
use cluster_config::{ClusterConfig, ClusterNode, TransportBackend};
use control_plane::{Catalog, InMemoryCatalog};
use data_model::{Attribute, Check, Mutation, Predicate, Space, SpaceOptions, Subspace, Value};
use data_plane::DataPlane;
use engine_memory::MemoryEngine;
use hyperdex_admin_protocol::{AdminRequest, HyperdexAdminService};
use hyperdex_client_protocol::{ClientRequest, ClientResponse, HyperdexClientService};
use placement_core::HyperSpacePlacement;
use proptest::prelude::*;
use server::{ClusterRuntime, TransportRuntime};
use storage_core::{StorageEngine, WriteResult};
use tokio::sync::Mutex;
use transport_core::{ClusterTransport, InternodeRequest, InternodeResponse, RemoteNode};

static HEGEL_SERVER_COMMAND: std::sync::OnceLock<String> = std::sync::OnceLock::new();
static HEGEL_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct TestHarness {
    data_plane: DataPlane,
}

#[derive(Default)]
struct SimTransport {
    runtimes: Mutex<BTreeMap<u64, Arc<ClusterRuntime>>>,
    unavailable: Mutex<BTreeSet<u64>>,
}

#[derive(Clone, Debug)]
enum ModelOp {
    Put { key: String, value: String },
    Delete { key: String },
    Get { key: String },
}

impl TestHarness {
    fn new() -> Self {
        let config = ClusterConfig::default();
        let catalog: Arc<dyn Catalog> =
            Arc::new(InMemoryCatalog::new(config.nodes.clone(), config.replicas));
        let storage: Arc<dyn StorageEngine> = Arc::new(MemoryEngine::new());

        storage.create_space("profiles".to_owned()).unwrap();
        catalog
            .create_space(Space {
                name: "profiles".to_owned(),
                key_attribute: "id".to_owned(),
                attributes: Vec::new(),
                subspaces: vec![Subspace {
                    dimensions: vec!["id".to_owned()],
                }],
                options: SpaceOptions::default(),
            })
            .unwrap();

        let data_plane = DataPlane::new(catalog, storage, Arc::new(HyperSpacePlacement::default()));

        Self { data_plane }
    }

    fn put_name(&self, key: &str, value: &str) -> WriteResult {
        self.data_plane
            .put(
                "profiles",
                Bytes::copy_from_slice(key.as_bytes()),
                &[Mutation::Set(Attribute {
                    name: "name".to_owned(),
                    value: Value::String(value.to_owned()),
                })],
            )
            .unwrap()
    }

    fn delete(&self, key: &str) -> WriteResult {
        self.data_plane.delete("profiles", key.as_bytes()).unwrap()
    }

    fn get_name(&self, key: &str) -> Option<String> {
        self.data_plane
            .get("profiles", key.as_bytes())
            .unwrap()
            .and_then(|record| match record.attributes.get("name") {
                Some(Value::String(value)) => Some(value.clone()),
                _ => None,
            })
    }

    fn snapshot(&self) -> BTreeMap<String, String> {
        self.data_plane
            .search("profiles", &[])
            .unwrap()
            .into_iter()
            .map(|record| {
                let key = String::from_utf8(record.key.to_vec()).unwrap();
                let value = match record.attributes.get("name") {
                    Some(Value::String(value)) => value.clone(),
                    other => panic!("unexpected name attribute: {other:?}"),
                };
                (key, value)
            })
            .collect()
    }

    fn count(&self) -> u64 {
        self.data_plane.count("profiles", &[]).unwrap()
    }
}

async fn single_node_runtime() -> Arc<ClusterRuntime> {
    let config = ClusterConfig {
        nodes: vec![ClusterNode {
            id: 1,
            host: "node1".to_owned(),
            control_port: 1001,
            data_port: 2001,
        }],
        replicas: 1,
        ..ClusterConfig::default()
    };

    let runtime = Arc::new(ClusterRuntime::for_node(config, 1).unwrap());
    HyperdexAdminService::handle(
        runtime.as_ref(),
        AdminRequest::CreateSpaceDsl(profiles_schema()),
    )
    .await
    .unwrap();
    runtime
}

async fn distributed_runtime_fixture_with_schema(
    schema: String,
) -> (Arc<SimTransport>, Arc<ClusterRuntime>, Arc<ClusterRuntime>) {
    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "node1".to_owned(),
                control_port: 1001,
                data_port: 2001,
            },
            ClusterNode {
                id: 2,
                host: "node2".to_owned(),
                control_port: 1002,
                data_port: 2002,
            },
        ],
        replicas: 2,
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    let transport = Arc::new(SimTransport::default());

    let mut runtime1 = ClusterRuntime::for_node(config.clone(), 1).unwrap();
    runtime1.install_cluster_transport(transport.clone(), TransportRuntime::Grpc);
    let runtime1 = Arc::new(runtime1);

    let mut runtime2 = ClusterRuntime::for_node(config, 2).unwrap();
    runtime2.install_cluster_transport(transport.clone(), TransportRuntime::Grpc);
    let runtime2 = Arc::new(runtime2);

    transport.register(1, runtime1.clone()).await;
    transport.register(2, runtime2.clone()).await;

    HyperdexAdminService::handle(
        runtime1.as_ref(),
        AdminRequest::CreateSpaceDsl(schema.clone()),
    )
    .await
    .unwrap();
    HyperdexAdminService::handle(runtime2.as_ref(), AdminRequest::CreateSpaceDsl(schema))
        .await
        .unwrap();

    (transport, runtime1, runtime2)
}

async fn distributed_runtime_fixture(
) -> (Arc<SimTransport>, Arc<ClusterRuntime>, Arc<ClusterRuntime>) {
    distributed_runtime_fixture_with_schema(profiles_schema()).await
}

async fn distributed_runtime_fixture_with_local_schema_only(
    schema: String,
    local_schema_node: u64,
) -> (Arc<SimTransport>, Arc<ClusterRuntime>, Arc<ClusterRuntime>) {
    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "node1".to_owned(),
                control_port: 1001,
                data_port: 2001,
            },
            ClusterNode {
                id: 2,
                host: "node2".to_owned(),
                control_port: 1002,
                data_port: 2002,
            },
        ],
        replicas: 1,
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    let transport = Arc::new(SimTransport::default());

    let mut runtime1 = ClusterRuntime::for_node(config.clone(), 1).unwrap();
    runtime1.install_cluster_transport(transport.clone(), TransportRuntime::Grpc);
    let runtime1 = Arc::new(runtime1);

    let mut runtime2 = ClusterRuntime::for_node(config, 2).unwrap();
    runtime2.install_cluster_transport(transport.clone(), TransportRuntime::Grpc);
    let runtime2 = Arc::new(runtime2);

    transport.register(1, runtime1.clone()).await;
    transport.register(2, runtime2.clone()).await;

    let schema_owner = match local_schema_node {
        1 => runtime1.clone(),
        2 => runtime2.clone(),
        other => panic!("unsupported schema owner node {other}"),
    };

    HyperdexAdminService::handle(
        schema_owner.as_ref(),
        AdminRequest::CreateSpaceDsl(schema),
    )
    .await
    .unwrap();

    (transport, runtime1, runtime2)
}

async fn distributed_runtime_pair() -> (Arc<ClusterRuntime>, Arc<ClusterRuntime>) {
    let (_, runtime1, runtime2) = distributed_runtime_fixture().await;
    (runtime1, runtime2)
}

impl SimTransport {
    async fn register(&self, node_id: u64, runtime: Arc<ClusterRuntime>) {
        self.runtimes.lock().await.insert(node_id, runtime);
    }

    async fn set_unavailable(&self, node_id: u64, unavailable: bool) {
        let mut guard = self.unavailable.lock().await;
        if unavailable {
            guard.insert(node_id);
        } else {
            guard.remove(&node_id);
        }
    }
}

fn degraded_read_target(
    runtime1: &Arc<ClusterRuntime>,
    runtime2: &Arc<ClusterRuntime>,
    prefix: &str,
) -> (Arc<ClusterRuntime>, u64, String) {
    (0..65536)
        .map(|i| format!("{prefix}-{i}"))
        .find_map(
            |key| match runtime1.route_primary(key.as_bytes()).unwrap() {
                1 => Some((runtime2.clone(), 1, key)),
                2 => Some((runtime1.clone(), 2, key)),
                _ => None,
            },
        )
        .expect("expected a key routed to either cluster node")
}

fn ensure_hegel_server_command() -> String {
    HEGEL_SERVER_COMMAND
        .get_or_init(|| {
            let root = std::env::temp_dir().join(format!(
                "hyperdex-rs-hegel-core-0.2.3-{}",
                std::process::id()
            ));
            let venv_dir = root.join("venv");
            let hegel = venv_dir.join("bin/hegel");
            let pyvenv_cfg = venv_dir.join("pyvenv.cfg");

            if hegel.is_file() && pyvenv_cfg.is_file() {
                return hegel.to_str().expect("hegel path must be utf-8").to_owned();
            }

            if venv_dir.exists() && !pyvenv_cfg.is_file() {
                std::fs::remove_dir_all(&venv_dir).expect("remove invalid hegel venv dir");
            }

            std::fs::create_dir_all(&root).expect("create hegel temp dir");

            let status = std::process::Command::new("uv")
                .args(["venv", "--clear"])
                .arg(&venv_dir)
                .status()
                .expect("run uv venv");
            assert!(status.success(), "uv venv failed for {:?}", venv_dir);

            let python = venv_dir.join("bin/python");
            let status = std::process::Command::new("uv")
                .args(["pip", "install", "--python"])
                .arg(&python)
                .arg("hegel-core==0.2.3")
                .status()
                .expect("run uv pip install");
            assert!(status.success(), "uv pip install failed for {:?}", python);

            assert!(hegel.is_file(), "missing hegel binary at {:?}", hegel);
            hegel.to_str().expect("hegel path must be utf-8").to_owned()
        })
        .clone()
}

#[async_trait]
impl ClusterTransport for SimTransport {
    async fn send(
        &self,
        node: &RemoteNode,
        request: InternodeRequest,
    ) -> Result<InternodeResponse> {
        if self.unavailable.lock().await.contains(&node.id) {
            return Err(anyhow!("connection refused for simulated node {}", node.id));
        }

        let runtime = self
            .runtimes
            .lock()
            .await
            .get(&node.id)
            .cloned()
            .ok_or_else(|| anyhow!("missing simulated node {}", node.id))?;
        runtime.handle_internode_request(request).await
    }

    fn name(&self) -> &'static str {
        "simulation"
    }
}

fn profiles_schema() -> String {
    "space profiles\n\
     key username\n\
     attributes\n\
        int profile_views\n\
     tolerate 0 failures\n"
        .to_owned()
}

fn replicated_profiles_schema() -> String {
    "space profiles\n\
     key username\n\
     attributes\n\
        int profile_views\n\
     tolerate 1 failures\n"
        .to_owned()
}

fn key_strategy() -> impl Strategy<Value = String> {
    "[a-z]{1,4}".prop_map(|value| value)
}

fn value_strategy() -> impl Strategy<Value = String> {
    "[a-z]{1,8}".prop_map(|value| value)
}

fn operation_strategy() -> impl Strategy<Value = ModelOp> {
    prop_oneof![
        (key_strategy(), value_strategy()).prop_map(|(key, value)| ModelOp::Put { key, value }),
        key_strategy().prop_map(|key| ModelOp::Delete { key }),
        key_strategy().prop_map(|key| ModelOp::Get { key }),
    ]
}

#[test]
fn data_plane_round_trip_with_memory_engine() {
    let harness = TestHarness::new();

    assert_eq!(harness.put_name("ada", "Ada"), WriteResult::Written);
    assert_eq!(harness.get_name("ada"), Some("Ada".to_owned()));
    assert_eq!(harness.count(), 1);
}

#[test]
fn turmoil_runs_a_deterministic_data_plane_session() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("test", async {
        let harness = TestHarness::new();
        assert_eq!(harness.put_name("ada", "Ada"), WriteResult::Written);

        tokio::time::sleep(Duration::from_millis(5)).await;

        assert_eq!(harness.get_name("ada"), Some("Ada".to_owned()));
        assert_eq!(harness.count(), 1);
        assert_eq!(harness.snapshot().get("ada"), Some(&"Ada".to_owned()));
        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_preserves_degraded_read_correctness_after_one_node_loss() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (transport, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;

        let (survivor_runtime, unavailable_node, degraded_key) =
            degraded_read_target(&runtime1, &runtime2, "sim-degraded");

        for (key, views) in [
            (degraded_key.clone(), 11_i64),
            ("sim-search-a".to_owned(), 7_i64),
            ("sim-search-b".to_owned(), 9_i64),
            ("sim-search-survivor".to_owned(), 1_i64),
        ] {
            let response = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::from(key.into_bytes()),
                    mutations: vec![Mutation::Numeric {
                        attribute: "profile_views".to_owned(),
                        op: data_model::NumericOp::Add,
                        operand: views,
                    }],
                },
            )
            .await
            .unwrap();
            assert_eq!(response, ClientResponse::Unit);
        }

        tokio::time::sleep(Duration::from_millis(5)).await;
        transport.set_unavailable(unavailable_node, true).await;
        tokio::time::sleep(Duration::from_millis(5)).await;

        let get = HyperdexClientService::handle(
            survivor_runtime.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(degraded_key.clone().into_bytes()),
            },
        )
        .await
        .unwrap();
        assert!(matches!(get, ClientResponse::Record(Some(_))));

        let search = HyperdexClientService::handle(
            survivor_runtime.as_ref(),
            ClientRequest::Search {
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
        let ClientResponse::SearchResult(records) = search else {
            panic!("expected search results");
        };

        let mut keys: Vec<Vec<u8>> = records
            .into_iter()
            .map(|record| record.key.to_vec())
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                degraded_key.as_bytes().to_vec(),
                b"sim-search-a".to_vec(),
                b"sim-search-b".to_vec()
            ]
        );

        let count = HyperdexClientService::handle(
            survivor_runtime.as_ref(),
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
        assert_eq!(count, ClientResponse::Count(3));

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_preserves_search_and_count_during_schema_convergence_gap() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (_, runtime1, runtime2) =
            distributed_runtime_fixture_with_local_schema_only(profiles_schema(), 1).await;

        let local_key = (0..65536)
            .map(|i| format!("stale-schema-{i}"))
            .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 1)
            .expect("expected a key routed to node 1");

        let put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(local_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(11),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(put, ClientResponse::Unit);

        assert!(runtime2.route_primary_for_space("profiles", local_key.as_bytes()).is_err());

        let search = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Search {
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
        let ClientResponse::SearchResult(records) = search else {
            panic!("expected search results during schema convergence gap");
        };
        let keys = records
            .iter()
            .map(|record| record.key.to_vec())
            .collect::<Vec<_>>();
        assert_eq!(keys, vec![local_key.as_bytes().to_vec()]);

        let count = HyperdexClientService::handle(
            runtime1.as_ref(),
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

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[cfg(madsim)]
#[test]
fn madsim_preserves_degraded_read_correctness_after_one_node_loss() {
    let runtime = madsim::runtime::Runtime::with_seed_and_config(7, madsim::Config::default());

    runtime.block_on(async move {
        let (transport, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;

        let (survivor_runtime, unavailable_node, degraded_key) =
            degraded_read_target(&runtime1, &runtime2, "madsim-degraded");

        for (key, views) in [
            (degraded_key.clone(), 11_i64),
            ("madsim-search-a".to_owned(), 7_i64),
            ("madsim-search-b".to_owned(), 9_i64),
            ("madsim-search-survivor".to_owned(), 1_i64),
        ] {
            let response = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::from(key.into_bytes()),
                    mutations: vec![Mutation::Numeric {
                        attribute: "profile_views".to_owned(),
                        op: data_model::NumericOp::Add,
                        operand: views,
                    }],
                },
            )
            .await
            .unwrap();
            assert_eq!(response, ClientResponse::Unit);
        }

        madsim::time::sleep(Duration::from_millis(5)).await;
        transport.set_unavailable(unavailable_node, true).await;
        madsim::time::sleep(Duration::from_millis(5)).await;

        let get = HyperdexClientService::handle(
            survivor_runtime.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(degraded_key.clone().into_bytes()),
            },
        )
        .await
        .unwrap();
        assert!(matches!(get, ClientResponse::Record(Some(_))));

        let search = HyperdexClientService::handle(
            survivor_runtime.as_ref(),
            ClientRequest::Search {
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
        let ClientResponse::SearchResult(records) = search else {
            panic!("expected search results");
        };

        let mut keys: Vec<Vec<u8>> = records
            .into_iter()
            .map(|record| record.key.to_vec())
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                degraded_key.as_bytes().to_vec(),
                b"madsim-search-a".to_vec(),
                b"madsim-search-b".to_vec()
            ]
        );

        let count = HyperdexClientService::handle(
            survivor_runtime.as_ref(),
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
        assert_eq!(count, ClientResponse::Count(3));
    });
}

#[test]
fn hegel_memory_engine_matches_stateful_sequence_model() {
    let _guard = HEGEL_ENV_LOCK.lock().unwrap();
    let hegel_server_command = ensure_hegel_server_command();
    unsafe {
        std::env::set_var("HEGEL_SERVER_COMMAND", &hegel_server_command);
    }

    hegel::Hegel::new(|tc: hegel::TestCase| {
        let ops: Vec<(u8, u8, u16)> = tc.draw(
            hegel::generators::vecs(hegel::generators::tuples3(
                hegel::generators::integers::<u8>().max_value(2),
                hegel::generators::integers::<u8>().max_value(12),
                hegel::generators::integers::<u16>().max_value(99),
            ))
            .max_size(30),
        );

        let harness = TestHarness::new();
        let mut model = BTreeMap::<String, String>::new();

        for (kind, key_id, value_id) in ops {
            let key = format!("k{key_id}");

            match kind {
                0 => {
                    let value = format!("v{value_id}");
                    assert_eq!(harness.put_name(&key, &value), WriteResult::Written);
                    model.insert(key.clone(), value);
                }
                1 => {
                    let expected = if model.remove(&key).is_some() {
                        WriteResult::Written
                    } else {
                        WriteResult::Missing
                    };
                    assert_eq!(harness.delete(&key), expected);
                }
                2 => {
                    assert_eq!(harness.get_name(&key), model.get(&key).cloned());
                }
                _ => unreachable!("operation kind is bounded to 0..=2"),
            }

            assert_eq!(harness.snapshot(), model);
            assert_eq!(harness.count(), model.len() as u64);
        }
    })
    .settings(hegel::Settings::new().test_cases(25))
    .run();
}

#[test]
fn hegel_single_node_runtime_matches_sequence_model() {
    let _guard = HEGEL_ENV_LOCK.lock().unwrap();
    let hegel_server_command = ensure_hegel_server_command();
    unsafe {
        std::env::set_var("HEGEL_SERVER_COMMAND", &hegel_server_command);
    }

    hegel::Hegel::new(|tc: hegel::TestCase| {
        let ops: Vec<(u8, u8, u16)> = tc.draw(
            hegel::generators::vecs(hegel::generators::tuples3(
                hegel::generators::integers::<u8>().max_value(3),
                hegel::generators::integers::<u8>().max_value(12),
                hegel::generators::integers::<u16>().max_value(99),
            ))
            .max_size(25),
        );

        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let runtime = single_node_runtime().await;
            let mut model = BTreeMap::<String, i64>::new();

            for (kind, key_id, value_id) in ops {
                let key = format!("k{key_id}");
                let key_bytes = Bytes::from(key.clone().into_bytes());

                match kind {
                    0 => {
                        let value = i64::from(value_id);
                        let response = HyperdexClientService::handle(
                            runtime.as_ref(),
                            ClientRequest::Put {
                                space: "profiles".to_owned(),
                                key: key_bytes.clone(),
                                mutations: vec![Mutation::Set(Attribute {
                                    name: "profile_views".to_owned(),
                                    value: Value::Int(value),
                                })],
                            },
                        )
                        .await
                        .unwrap();
                        assert_eq!(response, ClientResponse::Unit);
                        model.insert(key.clone(), value);
                    }
                    1 => {
                        let response = HyperdexClientService::handle(
                            runtime.as_ref(),
                            ClientRequest::Delete {
                                space: "profiles".to_owned(),
                                key: key_bytes.clone(),
                            },
                        )
                        .await
                        .unwrap();
                        assert_eq!(response, ClientResponse::Unit);
                        model.remove(&key);
                    }
                    2 => {
                        let response = HyperdexClientService::handle(
                            runtime.as_ref(),
                            ClientRequest::Get {
                                space: "profiles".to_owned(),
                                key: key_bytes.clone(),
                            },
                        )
                        .await
                        .unwrap();
                        let expected = model.get(&key).cloned();
                        match response {
                            ClientResponse::Record(Some(record)) => {
                                let actual = match record.attributes.get("profile_views") {
                                    Some(Value::Int(value)) => Some(*value),
                                    _ => None,
                                };
                                assert_eq!(actual, expected);
                            }
                            ClientResponse::Record(None) => assert_eq!(expected, None),
                            other => panic!("unexpected get response: {other:?}"),
                        }
                    }
                    3 => {
                        let response = HyperdexClientService::handle(
                            runtime.as_ref(),
                            ClientRequest::Count {
                                space: "profiles".to_owned(),
                                checks: Vec::new(),
                            },
                        )
                        .await
                        .unwrap();
                        assert_eq!(response, ClientResponse::Count(model.len() as u64));
                    }
                    _ => unreachable!("operation kind is bounded to 0..=3"),
                }

                let count = HyperdexClientService::handle(
                    runtime.as_ref(),
                    ClientRequest::Count {
                        space: "profiles".to_owned(),
                        checks: Vec::new(),
                    },
                )
                .await
                .unwrap();
                assert_eq!(count, ClientResponse::Count(model.len() as u64));
            }
        });
    })
    .settings(hegel::Settings::new().test_cases(20))
    .run();
}

#[test]
fn hegel_distributed_runtime_routes_put_and_get() {
    let _guard = HEGEL_ENV_LOCK.lock().unwrap();
    let hegel_server_command = ensure_hegel_server_command();
    unsafe {
        std::env::set_var("HEGEL_SERVER_COMMAND", &hegel_server_command);
    }

    hegel::Hegel::new(|tc: hegel::TestCase| {
        let key_suffix: u16 = tc.draw(hegel::generators::integers::<u16>().max_value(4095));
        let value_id: u16 = tc.draw(hegel::generators::integers::<u16>().max_value(99));

        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let (runtime1, runtime2) = distributed_runtime_pair().await;
            let routed_key = (0..65536)
                .map(|i| format!("dist-{key_suffix}-{i}"))
                .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 2)
                .expect("expected a key routed to node 2");
            let expected_value = i64::from(value_id);

            let put = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.clone().into_bytes()),
                    mutations: vec![Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(expected_value),
                    })],
                },
            )
            .await
            .unwrap();
            assert_eq!(put, ClientResponse::Unit);

            let get = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Get {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.clone().into_bytes()),
                },
            )
            .await
            .unwrap();
            match get {
                ClientResponse::Record(Some(record)) => {
                    let actual = match record.attributes.get("profile_views") {
                        Some(Value::Int(value)) => *value,
                        other => panic!("unexpected record attribute: {other:?}"),
                    };
                    assert_eq!(actual, expected_value);
                }
                other => panic!("unexpected get response: {other:?}"),
            }

            let primary_view = HyperdexClientService::handle(
                runtime2.as_ref(),
                ClientRequest::Get {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.into_bytes()),
                },
            )
            .await
            .unwrap();
            match primary_view {
                ClientResponse::Record(Some(record)) => {
                    let actual = match record.attributes.get("profile_views") {
                        Some(Value::Int(value)) => *value,
                        other => panic!("unexpected record attribute: {other:?}"),
                    };
                    assert_eq!(actual, expected_value);
                }
                other => panic!("unexpected primary get response: {other:?}"),
            }
        });
    })
    .settings(hegel::Settings::new().test_cases(15))
    .run();
}

#[test]
fn hegel_distributed_runtime_preserves_degraded_get_and_count() {
    let _guard = HEGEL_ENV_LOCK.lock().unwrap();
    let hegel_server_command = ensure_hegel_server_command();
    unsafe {
        std::env::set_var("HEGEL_SERVER_COMMAND", &hegel_server_command);
    }

    hegel::Hegel::new(|tc: hegel::TestCase| {
        let degraded_suffix: u16 = tc.draw(hegel::generators::integers::<u16>().max_value(4095));
        let degraded_value_id: u16 = tc.draw(hegel::generators::integers::<u16>().max_value(99));
        let survivor_value_id: u16 = tc.draw(hegel::generators::integers::<u16>().max_value(99));

        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let (transport, runtime1, runtime2) =
                distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;
            let (survivor_runtime, unavailable_node, degraded_key) = degraded_read_target(
                &runtime1,
                &runtime2,
                &format!("hegel-degraded-{degraded_suffix}"),
            );
            let degraded_value = i64::from(degraded_value_id);
            let survivor_value = i64::from(survivor_value_id);
            let survivor_key = format!("survivor-{degraded_suffix}");

            let put_degraded = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::from(degraded_key.clone().into_bytes()),
                    mutations: vec![Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(degraded_value),
                    })],
                },
            )
            .await
            .unwrap();
            assert_eq!(put_degraded, ClientResponse::Unit);

            let put_survivor = HyperdexClientService::handle(
                survivor_runtime.as_ref(),
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::from(survivor_key.clone().into_bytes()),
                    mutations: vec![Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(survivor_value),
                    })],
                },
            )
            .await
            .unwrap();
            assert_eq!(put_survivor, ClientResponse::Unit);

            transport.set_unavailable(unavailable_node, true).await;

            let degraded_get = HyperdexClientService::handle(
                survivor_runtime.as_ref(),
                ClientRequest::Get {
                    space: "profiles".to_owned(),
                    key: Bytes::from(degraded_key.clone().into_bytes()),
                },
            )
            .await
            .unwrap();
            match degraded_get {
                ClientResponse::Record(Some(record)) => {
                    let actual = match record.attributes.get("profile_views") {
                        Some(Value::Int(value)) => *value,
                        other => panic!("unexpected record attribute: {other:?}"),
                    };
                    assert_eq!(actual, degraded_value);
                }
                other => panic!("unexpected degraded get response: {other:?}"),
            }

            let degraded_count = HyperdexClientService::handle(
                survivor_runtime.as_ref(),
                ClientRequest::Count {
                    space: "profiles".to_owned(),
                    checks: Vec::new(),
                },
            )
            .await
            .unwrap();
            assert_eq!(degraded_count, ClientResponse::Count(2));
        });
    })
    .settings(hegel::Settings::new().test_cases(15))
    .run();
}

#[test]
fn hegel_distributed_runtime_routes_delete() {
    let _guard = HEGEL_ENV_LOCK.lock().unwrap();
    let hegel_server_command = ensure_hegel_server_command();
    unsafe {
        std::env::set_var("HEGEL_SERVER_COMMAND", &hegel_server_command);
    }

    hegel::Hegel::new(|tc: hegel::TestCase| {
        let key_suffix: u16 = tc.draw(hegel::generators::integers::<u16>().max_value(4095));
        let value_id: u16 = tc.draw(hegel::generators::integers::<u16>().max_value(99));

        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let (runtime1, runtime2) = distributed_runtime_pair().await;
            let routed_key = (0..65536)
                .map(|i| format!("delete-{key_suffix}-{i}"))
                .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 2)
                .expect("expected a key routed to node 2");
            let expected_value = i64::from(value_id);

            let put = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.clone().into_bytes()),
                    mutations: vec![Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(expected_value),
                    })],
                },
            )
            .await
            .unwrap();
            assert_eq!(put, ClientResponse::Unit);

            let delete = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Delete {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.clone().into_bytes()),
                },
            )
            .await
            .unwrap();
            assert_eq!(delete, ClientResponse::Unit);

            let routed_get = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Get {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.clone().into_bytes()),
                },
            )
            .await
            .unwrap();
            assert_eq!(routed_get, ClientResponse::Record(None));

            let primary_get = HyperdexClientService::handle(
                runtime2.as_ref(),
                ClientRequest::Get {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.into_bytes()),
                },
            )
            .await
            .unwrap();
            assert_eq!(primary_get, ClientResponse::Record(None));
        });
    })
    .settings(hegel::Settings::new().test_cases(15))
    .run();
}

#[test]
fn hegel_distributed_runtime_routes_conditional_put() {
    let _guard = HEGEL_ENV_LOCK.lock().unwrap();
    let hegel_server_command = ensure_hegel_server_command();
    unsafe {
        std::env::set_var("HEGEL_SERVER_COMMAND", &hegel_server_command);
    }

    hegel::Hegel::new(|tc: hegel::TestCase| {
        let key_suffix: u16 = tc.draw(hegel::generators::integers::<u16>().max_value(4095));
        let initial_value_id: u16 = tc.draw(hegel::generators::integers::<u16>().max_value(99));
        let updated_value_id: u16 = tc.draw(hegel::generators::integers::<u16>().max_value(99));
        let failed_value_id: u16 = tc.draw(hegel::generators::integers::<u16>().max_value(99));

        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let (runtime1, runtime2) = distributed_runtime_pair().await;
            let routed_key = (0..65536)
                .map(|i| format!("conditional-{key_suffix}-{i}"))
                .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 2)
                .expect("expected a key routed to node 2");
            let initial_value = i64::from(initial_value_id);
            let updated_value = i64::from(updated_value_id) + 100;
            let failed_value = i64::from(failed_value_id) + 200;

            let put = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.clone().into_bytes()),
                    mutations: vec![Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(initial_value),
                    })],
                },
            )
            .await
            .unwrap();
            assert_eq!(put, ClientResponse::Unit);

            let success = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::ConditionalPut {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.clone().into_bytes()),
                    checks: vec![Check {
                        attribute: "profile_views".to_owned(),
                        predicate: Predicate::Equal,
                        value: Value::Int(initial_value),
                    }],
                    mutations: vec![Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(updated_value),
                    })],
                },
            )
            .await
            .unwrap();
            assert_eq!(success, ClientResponse::Unit);

            let compare_failed = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::ConditionalPut {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.clone().into_bytes()),
                    checks: vec![Check {
                        attribute: "profile_views".to_owned(),
                        predicate: Predicate::Equal,
                        value: Value::Int(initial_value),
                    }],
                    mutations: vec![Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(failed_value),
                    })],
                },
            )
            .await
            .unwrap();
            assert_eq!(compare_failed, ClientResponse::ConditionFailed);

            let routed_get = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Get {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.clone().into_bytes()),
                },
            )
            .await
            .unwrap();
            match routed_get {
                ClientResponse::Record(Some(record)) => {
                    let actual = match record.attributes.get("profile_views") {
                        Some(Value::Int(value)) => *value,
                        other => panic!("unexpected record attribute: {other:?}"),
                    };
                    assert_eq!(actual, updated_value);
                }
                other => panic!("unexpected routed get response: {other:?}"),
            }

            let primary_get = HyperdexClientService::handle(
                runtime2.as_ref(),
                ClientRequest::Get {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.into_bytes()),
                },
            )
            .await
            .unwrap();
            match primary_get {
                ClientResponse::Record(Some(record)) => {
                    let actual = match record.attributes.get("profile_views") {
                        Some(Value::Int(value)) => *value,
                        other => panic!("unexpected record attribute: {other:?}"),
                    };
                    assert_eq!(actual, updated_value);
                }
                other => panic!("unexpected primary get response: {other:?}"),
            }
        });
    })
    .settings(hegel::Settings::new().test_cases(15))
    .run();
}

#[test]
fn hegel_distributed_runtime_routes_numeric_mutation() {
    let _guard = HEGEL_ENV_LOCK.lock().unwrap();
    let hegel_server_command = ensure_hegel_server_command();
    unsafe {
        std::env::set_var("HEGEL_SERVER_COMMAND", &hegel_server_command);
    }

    hegel::Hegel::new(|tc: hegel::TestCase| {
        let key_suffix: u16 = tc.draw(hegel::generators::integers::<u16>().max_value(4095));
        let initial_value_id: u16 = tc.draw(hegel::generators::integers::<u16>().max_value(99));
        let operand_id: u16 = tc.draw(hegel::generators::integers::<u16>().max_value(99));

        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let (runtime1, runtime2) = distributed_runtime_pair().await;
            let routed_key = (0..65536)
                .map(|i| format!("numeric-{key_suffix}-{i}"))
                .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 2)
                .expect("expected a key routed to node 2");
            let initial_value = i64::from(initial_value_id);
            let operand = i64::from(operand_id) + 1;
            let expected_value = initial_value + operand;

            let put = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.clone().into_bytes()),
                    mutations: vec![Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(initial_value),
                    })],
                },
            )
            .await
            .unwrap();
            assert_eq!(put, ClientResponse::Unit);

            let numeric = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.clone().into_bytes()),
                    mutations: vec![Mutation::Numeric {
                        attribute: "profile_views".to_owned(),
                        op: data_model::NumericOp::Add,
                        operand,
                    }],
                },
            )
            .await
            .unwrap();
            assert_eq!(numeric, ClientResponse::Unit);

            let routed_get = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Get {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.clone().into_bytes()),
                },
            )
            .await
            .unwrap();
            match routed_get {
                ClientResponse::Record(Some(record)) => {
                    let actual = match record.attributes.get("profile_views") {
                        Some(Value::Int(value)) => *value,
                        other => panic!("unexpected record attribute: {other:?}"),
                    };
                    assert_eq!(actual, expected_value);
                }
                other => panic!("unexpected routed get response: {other:?}"),
            }

            let primary_get = HyperdexClientService::handle(
                runtime2.as_ref(),
                ClientRequest::Get {
                    space: "profiles".to_owned(),
                    key: Bytes::from(routed_key.into_bytes()),
                },
            )
            .await
            .unwrap();
            match primary_get {
                ClientResponse::Record(Some(record)) => {
                    let actual = match record.attributes.get("profile_views") {
                        Some(Value::Int(value)) => *value,
                        other => panic!("unexpected record attribute: {other:?}"),
                    };
                    assert_eq!(actual, expected_value);
                }
                other => panic!("unexpected primary get response: {other:?}"),
            }
        });
    })
    .settings(hegel::Settings::new().test_cases(15))
    .run();
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        .. ProptestConfig::default()
    })]

    #[test]
    fn memory_engine_matches_a_simple_model(ops in proptest::collection::vec(operation_strategy(), 1..40)) {
        let harness = TestHarness::new();
        let mut model = BTreeMap::<String, String>::new();

        for op in ops {
            match op {
                ModelOp::Put { key, value } => {
                    let result = harness.put_name(&key, &value);
                    prop_assert_eq!(result, WriteResult::Written);
                    model.insert(key, value);
                }
                ModelOp::Delete { key } => {
                    let result = harness.delete(&key);
                    let expected = if model.remove(&key).is_some() {
                        WriteResult::Written
                    } else {
                        WriteResult::Missing
                    };
                    prop_assert_eq!(result, expected);
                }
                ModelOp::Get { key } => {
                    prop_assert_eq!(harness.get_name(&key), model.get(&key).cloned());
                }
            }

            prop_assert_eq!(harness.snapshot(), model.clone());
            prop_assert_eq!(harness.count(), model.len() as u64);
        }
    }
}
