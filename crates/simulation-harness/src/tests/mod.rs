mod failure_testing;
mod distributed_simulation;

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow};
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
use transport_core::{
    ClusterTransport, DATA_PLANE_METHOD, DataPlaneRequest, DataPlaneResponse, InternodeRequest,
    InternodeResponse, RemoteNode,
};

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

async fn distributed_runtime_fixture()
-> (Arc<SimTransport>, Arc<ClusterRuntime>, Arc<ClusterRuntime>) {
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

    HyperdexAdminService::handle(schema_owner.as_ref(), AdminRequest::CreateSpaceDsl(schema))
        .await
        .unwrap();

    (transport, runtime1, runtime2)
}

async fn distributed_runtime_fixture_with_diverged_cluster_views(
    schema: String,
) -> (Arc<SimTransport>, Arc<ClusterRuntime>, Arc<ClusterRuntime>) {
    let config1 = ClusterConfig {
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
    let config2 = ClusterConfig {
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
            ClusterNode {
                id: 3,
                host: "node3".to_owned(),
                control_port: 1003,
                data_port: 2003,
            },
        ],
        replicas: 1,
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    let transport = Arc::new(SimTransport::default());

    let mut runtime1 = ClusterRuntime::for_node(config1, 1).unwrap();
    runtime1.install_cluster_transport(transport.clone(), TransportRuntime::Grpc);
    let runtime1 = Arc::new(runtime1);

    let mut runtime2 = ClusterRuntime::for_node(config2, 2).unwrap();
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

fn converged_two_node_config() -> ClusterConfig {
    ClusterConfig {
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
    }
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

fn stale_placement_mutation_target(
    runtime1: &Arc<ClusterRuntime>,
    runtime2: &Arc<ClusterRuntime>,
    prefix: &str,
) -> (u64, u64, String) {
    (0..65536)
        .map(|i| format!("{prefix}-{i}"))
        .find_map(|key| {
            let primary1 = runtime1.route_primary(key.as_bytes()).ok()?;
            let primary2 = runtime2.route_primary(key.as_bytes()).ok()?;
            (primary1 == 2 && primary2 != 2).then_some((primary1, primary2, key))
        })
        .expect("expected a key whose primary diverges across cluster views")
}

fn stale_local_primary_target(
    runtime1: &Arc<ClusterRuntime>,
    runtime2: &Arc<ClusterRuntime>,
    prefix: &str,
) -> (u64, u64, String) {
    (0..65536)
        .map(|i| format!("{prefix}-{i}"))
        .find_map(|key| {
            let primary1 = runtime1.route_primary(key.as_bytes()).ok()?;
            let primary2 = runtime2.route_primary(key.as_bytes()).ok()?;
            (primary1 == runtime1.local_node_id() && primary1 != primary2)
                .then_some((primary1, primary2, key))
        })
        .expect("expected a key whose local primary ownership diverges")
}

fn stale_local_primary_target_for_authoritative_node(
    runtime1: &Arc<ClusterRuntime>,
    runtime2: &Arc<ClusterRuntime>,
    prefix: &str,
    authoritative_primary: u64,
) -> (u64, u64, String) {
    (0..65536)
        .map(|i| format!("{prefix}-{i}"))
        .find_map(|key| {
            let primary1 = runtime1.route_primary(key.as_bytes()).ok()?;
            let primary2 = runtime2.route_primary(key.as_bytes()).ok()?;
            (primary1 == runtime1.local_node_id() && primary2 == authoritative_primary)
                .then_some((primary1, primary2, key))
        })
        .expect(
            "expected a key whose stale local primary diverges to the requested authoritative node",
        )
}

impl ClusterTransport for SimTransport {
    fn send<'a>(
        &'a self,
        node: &'a RemoteNode,
        request: InternodeRequest,
    ) -> Pin<Box<dyn Future<Output = Result<InternodeResponse>> + Send + 'a>> {
        Box::pin(async move {
            if self.unavailable.lock().await.contains(&node.id) {
                return Err(anyhow!("connection refused for simulated node {}", node.id));
            }

            let runtime = self
                .runtimes
                .lock()
                .await
                .get(&node.id)
                .cloned()
                .ok_or_else(|| anyhow!("connection refused for simulated node {}", node.id))?;
            runtime.handle_internode_request(request).await
        })
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

fn profile_views_ge_checks(threshold: i64) -> Vec<Check> {
    vec![Check {
        attribute: "profile_views".to_owned(),
        predicate: Predicate::GreaterThanOrEqual,
        value: Value::Int(threshold),
    }]
}

fn expected_profile_views_at_or_above(
    model: &BTreeMap<String, i64>,
    threshold: i64,
) -> BTreeMap<String, i64> {
    model
        .iter()
        .filter(|(_, views)| **views >= threshold)
        .map(|(key, views)| (key.clone(), *views))
        .collect()
}

fn search_result_profile_views(response: ClientResponse) -> BTreeMap<String, i64> {
    let ClientResponse::SearchResult(records) = response else {
        panic!("expected search response");
    };

    let mut logical = BTreeMap::new();
    for record in records {
        let key = String::from_utf8(record.key.to_vec()).expect("search key must be utf-8");
        let views = match record.attributes.get("profile_views") {
            Some(Value::Int(value)) => *value,
            other => panic!("unexpected record attribute: {other:?}"),
        };
        assert!(
            logical.insert(key, views).is_none(),
            "distributed search returned duplicate logical keys"
        );
    }
    logical
}

async fn assert_search_and_count_match_model(
    runtime: &ClusterRuntime,
    threshold: i64,
    expected: &BTreeMap<String, i64>,
) {
    let search = HyperdexClientService::handle(
        runtime,
        ClientRequest::Search {
            space: "profiles".to_owned(),
            checks: profile_views_ge_checks(threshold),
        },
    )
    .await
    .unwrap();
    assert_eq!(search_result_profile_views(search), *expected);

    let count = HyperdexClientService::handle(
        runtime,
        ClientRequest::Count {
            space: "profiles".to_owned(),
            checks: profile_views_ge_checks(threshold),
        },
    )
    .await
    .unwrap();
    assert_eq!(count, ClientResponse::Count(expected.len() as u64));
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

        assert!(
            runtime2
                .route_primary_for_space("profiles", local_key.as_bytes())
                .is_err()
        );

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

#[test]
fn turmoil_search_and_count_work_from_node_missing_local_space_definition() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (_, runtime1, runtime2) =
            distributed_runtime_fixture_with_local_schema_only(profiles_schema(), 2).await;

        let remote_key = (0..65536)
            .map(|i| format!("remote-schema-{i}"))
            .find(|key| runtime2.route_primary(key.as_bytes()).unwrap() == 2)
            .expect("expected a key routed to node 2");

        let put = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(remote_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(17),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(put, ClientResponse::Unit);

        assert!(
            runtime1
                .route_primary_for_space("profiles", remote_key.as_bytes())
                .is_err()
        );

        let search = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Search {
                space: "profiles".to_owned(),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::GreaterThanOrEqual,
                    value: Value::Int(10),
                }],
            },
        )
        .await
        .unwrap();
        let ClientResponse::SearchResult(records) = search else {
            panic!("expected remote search results while local node lacks the space");
        };
        let keys = records
            .iter()
            .map(|record| record.key.to_vec())
            .collect::<Vec<_>>();
        assert_eq!(keys, vec![remote_key.as_bytes().to_vec()]);

        let count = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Count {
                space: "profiles".to_owned(),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::GreaterThanOrEqual,
                    value: Value::Int(10),
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

#[test]
fn turmoil_delete_group_works_from_node_missing_local_space_definition() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (_, runtime1, runtime2) =
            distributed_runtime_fixture_with_local_schema_only(profiles_schema(), 2).await;

        let remote_key = (0..65536)
            .map(|i| format!("remote-delete-group-{i}"))
            .find(|key| runtime2.route_primary(key.as_bytes()).unwrap() == 2)
            .expect("expected a key routed to node 2");

        let put = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(remote_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(29),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(put, ClientResponse::Unit);

        let deleted = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::DeleteGroup {
                space: "profiles".to_owned(),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::Equal,
                    value: Value::Int(29),
                }],
            },
        )
        .await
        .unwrap();
        assert_eq!(deleted, ClientResponse::Deleted(1));

        let record = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(remote_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert_eq!(record, ClientResponse::Record(None));

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_reverts_primary_put_when_replica_transport_fails() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (transport, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;

        let failing_key = (0..65536)
            .map(|i| format!("replica-failure-put-{i}"))
            .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 1)
            .expect("expected a key routed to node 1");

        transport.set_unavailable(2, true).await;

        let put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(23),
                })],
            },
        )
        .await;
        assert!(
            put.is_err(),
            "expected replica transport failure to surface"
        );

        let local_record = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert_eq!(local_record, ClientResponse::Record(None));

        transport.set_unavailable(2, false).await;

        let recovered_record = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert_eq!(recovered_record, ClientResponse::Record(None));

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_reverts_primary_delete_when_replica_transport_fails() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (transport, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;

        let failing_key = (0..65536)
            .map(|i| format!("replica-failure-delete-{i}"))
            .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 1)
            .expect("expected a key routed to node 1");

        let put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(31),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(put, ClientResponse::Unit);

        transport.set_unavailable(2, true).await;

        let delete = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Delete {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.as_bytes().to_vec()),
            },
        )
        .await;
        assert!(
            delete.is_err(),
            "expected replica transport failure to surface"
        );

        let local_record = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert!(matches!(local_record, ClientResponse::Record(Some(_))));

        transport.set_unavailable(2, false).await;

        let recovered_record = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert!(matches!(recovered_record, ClientResponse::Record(Some(_))));

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_reverts_primary_conditional_put_when_replica_transport_fails() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (transport, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;

        let failing_key = (0..65536)
            .map(|i| format!("replica-failure-conditional-put-{i}"))
            .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 1)
            .expect("expected a key routed to node 1");

        let put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(31),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(put, ClientResponse::Unit);

        transport.set_unavailable(2, true).await;

        let conditional_put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::ConditionalPut {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.as_bytes().to_vec()),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::Equal,
                    value: Value::Int(31),
                }],
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(47),
                })],
            },
        )
        .await;
        assert!(
            conditional_put.is_err(),
            "expected replica transport failure to surface"
        );

        let local_record = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        match local_record {
            ClientResponse::Record(Some(record)) => {
                assert_eq!(record.key, Bytes::from(failing_key.as_bytes().to_vec()));
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(31))
                );
            }
            other => panic!("unexpected local conditional-put record result: {other:?}"),
        }

        transport.set_unavailable(2, false).await;

        let recovered_record = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        match recovered_record {
            ClientResponse::Record(Some(record)) => {
                assert_eq!(record.key, Bytes::from(failing_key.as_bytes().to_vec()));
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(31))
                );
            }
            other => panic!("unexpected recovered conditional-put record result: {other:?}"),
        }

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_reverts_delete_group_when_replica_transport_fails() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (transport, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;

        let failing_key = (0..65536)
            .map(|i| format!("replica-failure-delete-group-{i}"))
            .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 1)
            .expect("expected a key routed to node 1");

        let put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(61),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(put, ClientResponse::Unit);

        transport.set_unavailable(2, true).await;

        let delete_group = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::DeleteGroup {
                space: "profiles".to_owned(),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::Equal,
                    value: Value::Int(61),
                }],
            },
        )
        .await;
        assert!(
            delete_group.is_err(),
            "expected replica transport failure to surface"
        );

        let local_record = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert!(matches!(local_record, ClientResponse::Record(Some(_))));

        transport.set_unavailable(2, false).await;

        let recovered_record = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert!(matches!(recovered_record, ClientResponse::Record(Some(_))));

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_rejects_or_recovers_routed_mutation_under_stale_placement() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (_, runtime1, runtime2) =
            distributed_runtime_fixture_with_diverged_cluster_views(profiles_schema()).await;

        let (runtime1_primary, runtime2_primary, stale_key) =
            stale_placement_mutation_target(&runtime1, &runtime2, "stale-placement-put");
        assert_eq!(runtime1_primary, 2);
        assert_ne!(runtime2_primary, 2);

        let put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(stale_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(19),
                })],
            },
        )
        .await;
        assert!(
            put.is_err(),
            "expected stale-placement routed mutation to fail instead of silently succeeding"
        );

        let response = runtime2
            .handle_internode_request(
                InternodeRequest::encode(
                    DATA_PLANE_METHOD,
                    &DataPlaneRequest::Get {
                        space: "profiles".to_owned(),
                        key: Bytes::from(stale_key.as_bytes().to_vec()),
                    },
                )
                .unwrap(),
            )
            .await
            .unwrap();
        let local_record = response.decode::<DataPlaneResponse>().unwrap();
        assert_eq!(local_record, DataPlaneResponse::Record(None));

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_preserves_correctness_when_stale_node_rejoins_cluster() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (transport, runtime1, runtime2_stale) =
            distributed_runtime_fixture_with_diverged_cluster_views(profiles_schema()).await;

        let (runtime1_primary, runtime2_primary, rejoin_key) =
            stale_placement_mutation_target(&runtime1, &runtime2_stale, "stale-rejoin-put");
        assert_eq!(runtime1_primary, 2);
        assert_ne!(runtime2_primary, 2);

        let stale_put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(rejoin_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(41),
                })],
            },
        )
        .await;
        assert!(
            stale_put.is_err(),
            "expected routed mutation to fail while node 2 still has stale placement"
        );

        let mut recovered_runtime =
            ClusterRuntime::for_node(converged_two_node_config(), 2).unwrap();
        recovered_runtime.install_cluster_transport(transport.clone(), TransportRuntime::Grpc);
        let recovered_runtime = Arc::new(recovered_runtime);
        HyperdexAdminService::handle(
            recovered_runtime.as_ref(),
            AdminRequest::CreateSpaceDsl(profiles_schema()),
        )
        .await
        .unwrap();
        transport.register(2, recovered_runtime.clone()).await;

        let recovered_put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(rejoin_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(41),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(recovered_put, ClientResponse::Unit);

        let recovered_record = HyperdexClientService::handle(
            recovered_runtime.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(rejoin_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        match recovered_record {
            ClientResponse::Record(Some(record)) => {
                assert_eq!(record.key, Bytes::from(rejoin_key.as_bytes().to_vec()));
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(41))
                );
            }
            other => panic!("unexpected recovered record after rejoin: {other:?}"),
        }

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_recovery_preserves_operation_order_after_stale_local_primary_rejoin() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (transport, current_runtime, stale_runtime) =
            distributed_runtime_fixture_with_diverged_cluster_views(profiles_schema()).await;

        let (current_primary, stale_primary, recovery_key) = stale_placement_mutation_target(
            &current_runtime,
            &stale_runtime,
            "stale-recovery-ordering",
        );
        assert_eq!(current_primary, 2);
        assert_ne!(stale_primary, current_primary);

        let rejected_put = HyperdexClientService::handle(
            current_runtime.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(recovery_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(7),
                })],
            },
        )
        .await;
        assert!(
            rejected_put.is_err(),
            "expected the stale local-primary write to fail before recovery"
        );

        let mut recovered_runtime =
            ClusterRuntime::for_node(converged_two_node_config(), current_primary).unwrap();
        recovered_runtime.install_cluster_transport(transport.clone(), TransportRuntime::Grpc);
        let recovered_runtime = Arc::new(recovered_runtime);
        HyperdexAdminService::handle(
            recovered_runtime.as_ref(),
            AdminRequest::CreateSpaceDsl(profiles_schema()),
        )
        .await
        .unwrap();
        transport
            .register(current_primary, recovered_runtime.clone())
            .await;

        let first_write = HyperdexClientService::handle(
            current_runtime.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(recovery_key.as_bytes().to_vec()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(11),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(first_write, ClientResponse::Unit);

        let first_view = HyperdexClientService::handle(
            recovered_runtime.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(recovery_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        match first_view {
            ClientResponse::Record(Some(record)) => {
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(11))
                );
            }
            other => panic!("unexpected recovered-node view after first write: {other:?}"),
        }

        let second_write = HyperdexClientService::handle(
            current_runtime.as_ref(),
            ClientRequest::ConditionalPut {
                space: "profiles".to_owned(),
                key: Bytes::from(recovery_key.as_bytes().to_vec()),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::Equal,
                    value: Value::Int(11),
                }],
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(29),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(second_write, ClientResponse::Unit);

        let recovered_view = HyperdexClientService::handle(
            recovered_runtime.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(recovery_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        match &recovered_view {
            ClientResponse::Record(Some(record)) => {
                assert_eq!(record.key, Bytes::from(recovery_key.as_bytes().to_vec()));
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(29))
                );
            }
            other => panic!("unexpected recovered-node view after ordered writes: {other:?}"),
        }

        let authoritative_view = HyperdexClientService::handle(
            current_runtime.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(recovery_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert_eq!(authoritative_view, recovered_view);

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_recovery_preserves_delete_then_rewrite_visibility_after_stale_local_primary_rejoin() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (transport, current_runtime, stale_runtime) =
            distributed_runtime_fixture_with_diverged_cluster_views(profiles_schema()).await;

        let (current_primary, stale_primary, recovery_key) = stale_placement_mutation_target(
            &current_runtime,
            &stale_runtime,
            "stale-recovery-delete-rewrite",
        );
        assert_eq!(current_primary, 2);
        assert_ne!(stale_primary, current_primary);

        let rejected_put = HyperdexClientService::handle(
            current_runtime.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(recovery_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(5),
                })],
            },
        )
        .await;
        assert!(
            rejected_put.is_err(),
            "expected the stale local-primary write to fail before recovery"
        );

        let mut recovered_runtime =
            ClusterRuntime::for_node(converged_two_node_config(), current_primary).unwrap();
        recovered_runtime.install_cluster_transport(transport.clone(), TransportRuntime::Grpc);
        let recovered_runtime = Arc::new(recovered_runtime);
        HyperdexAdminService::handle(
            recovered_runtime.as_ref(),
            AdminRequest::CreateSpaceDsl(profiles_schema()),
        )
        .await
        .unwrap();
        transport
            .register(current_primary, recovered_runtime.clone())
            .await;

        let first_write = HyperdexClientService::handle(
            current_runtime.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(recovery_key.as_bytes().to_vec()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(11),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(first_write, ClientResponse::Unit);

        let first_visibility = HyperdexClientService::handle(
            recovered_runtime.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(recovery_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        match first_visibility {
            ClientResponse::Record(Some(record)) => {
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(11))
                );
            }
            other => panic!("unexpected recovered-node view after first write: {other:?}"),
        }

        let delete = HyperdexClientService::handle(
            current_runtime.as_ref(),
            ClientRequest::Delete {
                space: "profiles".to_owned(),
                key: Bytes::from(recovery_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert_eq!(delete, ClientResponse::Unit);

        let deleted_view = HyperdexClientService::handle(
            recovered_runtime.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(recovery_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert_eq!(deleted_view, ClientResponse::Record(None));

        let deleted_count = HyperdexClientService::handle(
            recovered_runtime.as_ref(),
            ClientRequest::Count {
                space: "profiles".to_owned(),
                checks: Vec::new(),
            },
        )
        .await
        .unwrap();
        assert_eq!(deleted_count, ClientResponse::Count(0));

        let rewrite = HyperdexClientService::handle(
            current_runtime.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(recovery_key.as_bytes().to_vec()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(29),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(rewrite, ClientResponse::Unit);

        let recovered_view = HyperdexClientService::handle(
            recovered_runtime.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(recovery_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        match &recovered_view {
            ClientResponse::Record(Some(record)) => {
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(29))
                );
            }
            other => panic!("unexpected recovered-node view after rewrite: {other:?}"),
        }

        let recovered_count = HyperdexClientService::handle(
            recovered_runtime.as_ref(),
            ClientRequest::Count {
                space: "profiles".to_owned(),
                checks: Vec::new(),
            },
        )
        .await
        .unwrap();
        assert_eq!(recovered_count, ClientResponse::Count(1));

        let authoritative_view = HyperdexClientService::handle(
            current_runtime.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(recovery_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert_eq!(authoritative_view, recovered_view);

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_rejects_local_mutation_when_peer_has_newer_primary_view() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (_, runtime1, runtime2) =
            distributed_runtime_fixture_with_diverged_cluster_views(profiles_schema()).await;

        let (runtime1_primary, runtime2_primary, stale_key) =
            stale_local_primary_target(&runtime1, &runtime2, "stale-local-primary-put");
        assert_eq!(runtime1_primary, runtime1.local_node_id());
        assert_ne!(runtime2_primary, runtime1_primary);

        let put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(stale_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(73),
                })],
            },
        )
        .await;
        assert!(
            put.is_err(),
            "expected stale local primary mutation to fail when a peer has a newer primary view"
        );

        let local_record = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(stale_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert_eq!(local_record, ClientResponse::Record(None));

        let remote_record = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(stale_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert_eq!(remote_record, ClientResponse::Record(None));

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_rejects_stale_local_mutation_across_peer_outage_and_recovery() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (transport, runtime1, runtime2) =
            distributed_runtime_fixture_with_diverged_cluster_views(profiles_schema()).await;

        let (runtime1_primary, runtime2_primary, stale_key) =
            stale_local_primary_target(&runtime1, &runtime2, "stale-local-primary-recovery");
        assert_eq!(runtime1_primary, runtime1.local_node_id());
        assert_ne!(runtime2_primary, runtime1_primary);

        transport.set_unavailable(runtime2.local_node_id(), true).await;

        let put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(stale_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(91),
                })],
            },
        )
        .await;
        assert!(
            put.is_err(),
            "expected stale local primary mutation to fail while peer with newer view is temporarily unavailable"
        );

        transport.set_unavailable(runtime2.local_node_id(), false).await;

        let local_record = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(stale_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert_eq!(local_record, ClientResponse::Record(None));

        let remote_record = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(stale_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert_eq!(remote_record, ClientResponse::Record(None));

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_rejects_stale_local_delete_across_peer_outage_and_recovery() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (transport, runtime1, runtime2) =
            distributed_runtime_fixture_with_diverged_cluster_views(profiles_schema()).await;

        let (runtime1_primary, runtime2_primary, stale_key) =
            stale_local_primary_target_for_authoritative_node(
                &runtime1,
                &runtime2,
                "stale-local-primary-delete",
                runtime2.local_node_id(),
            );
        assert_eq!(runtime1_primary, runtime1.local_node_id());
        assert_eq!(runtime2_primary, runtime2.local_node_id());

        let initial_put = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(stale_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(113),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(initial_put, ClientResponse::Unit);

        transport.set_unavailable(runtime2.local_node_id(), true).await;

        let delete = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Delete {
                space: "profiles".to_owned(),
                key: Bytes::from(stale_key.as_bytes().to_vec()),
            },
        )
        .await;
        assert!(
            delete.is_err(),
            "expected stale local primary delete to fail while the authoritative peer is temporarily unavailable"
        );

        transport.set_unavailable(runtime2.local_node_id(), false).await;

        let remote_record = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(stale_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        match remote_record {
            ClientResponse::Record(Some(record)) => {
                assert_eq!(record.key, Bytes::from(stale_key.as_bytes().to_vec()));
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(113))
                );
            }
            other => panic!("unexpected authoritative record after stale delete attempt: {other:?}"),
        }

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

#[hegel::test(test_cases = 25)]
fn hegel_memory_engine_matches_stateful_sequence_model(tc: hegel::TestCase) {
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
}

#[hegel::test(test_cases = 20)]
fn hegel_single_node_runtime_matches_sequence_model(tc: hegel::TestCase) {
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
}

#[hegel::test(test_cases = 15)]
fn hegel_distributed_runtime_routes_put_and_get(tc: hegel::TestCase) {
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
}

#[hegel::test(test_cases = 15)]
fn hegel_distributed_runtime_preserves_degraded_get_and_count(tc: hegel::TestCase) {
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
}

#[hegel::test(test_cases = 15)]
fn hegel_distributed_runtime_routes_delete(tc: hegel::TestCase) {
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
}

#[hegel::test(test_cases = 15)]
fn hegel_distributed_runtime_routes_conditional_put(tc: hegel::TestCase) {
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
}

#[hegel::test(test_cases = 15)]
fn hegel_distributed_runtime_routes_numeric_mutation(tc: hegel::TestCase) {
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
}

#[hegel::test(test_cases = 15)]
fn hegel_distributed_runtime_preserves_logical_delete_group_search_and_count(tc: hegel::TestCase) {
    // Generated routed writes and delete-group operations must still present one
    // logical query result set from either runtime, even though records are
    // physically replicated on both nodes.
    let ops: Vec<(u8, u8, u8, u16, u16)> = tc.draw(
        hegel::generators::vecs(hegel::generators::tuples5(
            hegel::generators::integers::<u8>().max_value(2),
            hegel::generators::integers::<u8>().max_value(1),
            hegel::generators::integers::<u8>().max_value(7),
            hegel::generators::integers::<u16>().max_value(119),
            hegel::generators::integers::<u16>().max_value(119),
        ))
        .min_size(1)
        .max_size(20),
    );

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async move {
        let (_, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;
        let runtimes = [runtime1, runtime2];
        let mut model = BTreeMap::<String, i64>::new();

        for (kind, runtime_id, key_id, value_id, threshold_id) in ops {
            let runtime = &runtimes[usize::from(runtime_id)];
            let key = format!("hegel-delete-group-k{key_id}");
            let key_bytes = Bytes::from(key.clone().into_bytes());
            let value = i64::from(value_id);
            let threshold = i64::from(threshold_id);

            match kind {
                0 => {
                    let response = HyperdexClientService::handle(
                        runtime.as_ref(),
                        ClientRequest::Put {
                            space: "profiles".to_owned(),
                            key: key_bytes,
                            mutations: vec![Mutation::Set(Attribute {
                                name: "profile_views".to_owned(),
                                value: Value::Int(value),
                            })],
                        },
                    )
                    .await
                    .unwrap();
                    assert_eq!(response, ClientResponse::Unit);
                    model.insert(key, value);
                }
                1 => {
                    let expected_deleted = model
                        .iter()
                        .filter(|(_, views)| **views >= threshold)
                        .count() as u64;
                    let response = HyperdexClientService::handle(
                        runtime.as_ref(),
                        ClientRequest::DeleteGroup {
                            space: "profiles".to_owned(),
                            checks: profile_views_ge_checks(threshold),
                        },
                    )
                    .await
                    .unwrap();
                    assert_eq!(response, ClientResponse::Deleted(expected_deleted));
                    model.retain(|_, views| *views < threshold);
                }
                2 => {
                    let response = HyperdexClientService::handle(
                        runtime.as_ref(),
                        ClientRequest::Get {
                            space: "profiles".to_owned(),
                            key: key_bytes,
                        },
                    )
                    .await
                    .unwrap();
                    match response {
                        ClientResponse::Record(Some(record)) => {
                            let actual = match record.attributes.get("profile_views") {
                                Some(Value::Int(value)) => Some(*value),
                                _ => None,
                            };
                            assert_eq!(actual, model.get(&key).copied());
                        }
                        ClientResponse::Record(None) => {
                            assert_eq!(model.get(&key), None);
                        }
                        other => panic!("unexpected get response: {other:?}"),
                    }
                }
                _ => unreachable!("operation kind is bounded to 0..=2"),
            }

            let expected = expected_profile_views_at_or_above(&model, threshold);
            assert_search_and_count_match_model(runtimes[0].as_ref(), threshold, &expected).await;
            assert_search_and_count_match_model(runtimes[1].as_ref(), threshold, &expected).await;
        }
    });
}

#[hegel::test(test_cases = 15)]
fn hegel_distributed_runtime_preserves_mixed_mutation_query_model(tc: hegel::TestCase) {
    // This property exercises a broader distributed state machine than the
    // delete-group proof: routed puts, compare-and-write updates, deletes, gets,
    // and threshold queries must all agree with one logical model from either
    // runtime, even though writes are routed and replicated under the hood.
    let ops: Vec<(u8, u8, u8, u16, u16, u16)> = tc.draw(
        hegel::generators::vecs(hegel::generators::tuples6(
            hegel::generators::integers::<u8>().max_value(5),
            hegel::generators::integers::<u8>().max_value(1),
            hegel::generators::integers::<u8>().max_value(7),
            hegel::generators::integers::<u16>().max_value(119),
            hegel::generators::integers::<u16>().max_value(119),
            hegel::generators::integers::<u16>().max_value(119),
        ))
        .min_size(1)
        .max_size(24),
    );

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async move {
        let (_, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;
        let runtimes = [runtime1, runtime2];
        let mut model = BTreeMap::<String, i64>::new();

        for (kind, runtime_id, key_id, value_id, compare_id, threshold_id) in ops {
            let runtime = &runtimes[usize::from(runtime_id)];
            let key = format!("hegel-mixed-k{key_id}");
            let key_bytes = Bytes::from(key.clone().into_bytes());
            let value = i64::from(value_id);
            let compare = i64::from(compare_id);
            let threshold = i64::from(threshold_id);

            match kind {
                0 => {
                    let response = HyperdexClientService::handle(
                        runtime.as_ref(),
                        ClientRequest::Put {
                            space: "profiles".to_owned(),
                            key: key_bytes,
                            mutations: vec![Mutation::Set(Attribute {
                                name: "profile_views".to_owned(),
                                value: Value::Int(value),
                            })],
                        },
                    )
                    .await
                    .unwrap();
                    assert_eq!(response, ClientResponse::Unit);
                    model.insert(key, value);
                }
                1 => {
                    let expected = model.get(&key).copied();
                    let response = HyperdexClientService::handle(
                        runtime.as_ref(),
                        ClientRequest::ConditionalPut {
                            space: "profiles".to_owned(),
                            key: key_bytes,
                            checks: vec![Check {
                                attribute: "profile_views".to_owned(),
                                predicate: Predicate::Equal,
                                value: Value::Int(compare),
                            }],
                            mutations: vec![Mutation::Set(Attribute {
                                name: "profile_views".to_owned(),
                                value: Value::Int(value),
                            })],
                        },
                    )
                    .await
                    .unwrap();
                    if expected == Some(compare) {
                        assert_eq!(response, ClientResponse::Unit);
                        model.insert(key, value);
                    } else {
                        assert_eq!(response, ClientResponse::ConditionFailed);
                    }
                }
                2 => {
                    let response = HyperdexClientService::handle(
                        runtime.as_ref(),
                        ClientRequest::Delete {
                            space: "profiles".to_owned(),
                            key: key_bytes,
                        },
                    )
                    .await
                    .unwrap();
                    assert_eq!(response, ClientResponse::Unit);
                    model.remove(&key);
                }
                3 => {
                    let response = HyperdexClientService::handle(
                        runtime.as_ref(),
                        ClientRequest::Get {
                            space: "profiles".to_owned(),
                            key: key_bytes,
                        },
                    )
                    .await
                    .unwrap();
                    match response {
                        ClientResponse::Record(Some(record)) => {
                            let actual = match record.attributes.get("profile_views") {
                                Some(Value::Int(current)) => Some(*current),
                                _ => None,
                            };
                            assert_eq!(actual, model.get(&key).copied());
                        }
                        ClientResponse::Record(None) => {
                            assert_eq!(model.get(&key), None);
                        }
                        other => panic!("unexpected get response: {other:?}"),
                    }
                }
                4 => {
                    let expected = expected_profile_views_at_or_above(&model, threshold);
                    assert_search_and_count_match_model(runtime.as_ref(), threshold, &expected)
                        .await;
                }
                5 => {
                    let count = HyperdexClientService::handle(
                        runtime.as_ref(),
                        ClientRequest::Count {
                            space: "profiles".to_owned(),
                            checks: profile_views_ge_checks(threshold),
                        },
                    )
                    .await
                    .unwrap();
                    let expected = expected_profile_views_at_or_above(&model, threshold);
                    assert_eq!(count, ClientResponse::Count(expected.len() as u64));
                }
                _ => unreachable!("operation kind is bounded to 0..=5"),
            }

            let expected = expected_profile_views_at_or_above(&model, threshold);
            assert_search_and_count_match_model(runtimes[0].as_ref(), threshold, &expected).await;
            assert_search_and_count_match_model(runtimes[1].as_ref(), threshold, &expected).await;
        }
    });
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
