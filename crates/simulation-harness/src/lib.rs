#[cfg(test)]
mod tests {
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

            let data_plane =
                DataPlane::new(catalog, storage, Arc::new(HyperSpacePlacement::default()));

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
            .find_map(|key| match runtime1.route_primary(key.as_bytes()).unwrap() {
                1 => Some((runtime2.clone(), 1, key)),
                2 => Some((runtime1.clone(), 2, key)),
                _ => None,
            })
            .expect("expected a key routed to either cluster node")
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

            let mut runtime2 = ClusterRuntime::for_node(config.clone(), 2).unwrap();
            runtime2.install_cluster_transport(transport.clone(), TransportRuntime::Grpc);
            let runtime2 = Arc::new(runtime2);

            transport.register(1, runtime1.clone()).await;
            transport.register(2, runtime2.clone()).await;

            HyperdexAdminService::handle(
                runtime1.as_ref(),
                AdminRequest::CreateSpaceDsl(profiles_schema()),
            )
            .await
            .unwrap();
            HyperdexAdminService::handle(
                runtime2.as_ref(),
                AdminRequest::CreateSpaceDsl(profiles_schema()),
            )
            .await
            .unwrap();

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

    #[cfg(madsim)]
    #[test]
    fn madsim_preserves_degraded_read_correctness_after_one_node_loss() {
        let runtime = madsim::runtime::Runtime::with_seed_and_config(7, madsim::Config::default());

        runtime.block_on(async move {
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

            let mut runtime2 = ClusterRuntime::for_node(config.clone(), 2).unwrap();
            runtime2.install_cluster_transport(transport.clone(), TransportRuntime::Grpc);
            let runtime2 = Arc::new(runtime2);

            transport.register(1, runtime1.clone()).await;
            transport.register(2, runtime2.clone()).await;

            HyperdexAdminService::handle(
                runtime1.as_ref(),
                AdminRequest::CreateSpaceDsl(profiles_schema()),
            )
            .await
            .unwrap();
            HyperdexAdminService::handle(
                runtime2.as_ref(),
                AdminRequest::CreateSpaceDsl(profiles_schema()),
            )
            .await
            .unwrap();

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
}
