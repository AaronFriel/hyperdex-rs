#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::time::Duration;

    use bytes::Bytes;
    use cluster_config::ClusterConfig;
    use control_plane::{Catalog, InMemoryCatalog};
    use data_model::{Attribute, Mutation, Space, SpaceOptions, Subspace, Value};
    use data_plane::DataPlane;
    use engine_memory::MemoryEngine;
    use placement_core::HyperSpacePlacement;
    use proptest::prelude::*;
    use storage_core::{StorageEngine, WriteResult};

    struct TestHarness {
        data_plane: DataPlane,
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
