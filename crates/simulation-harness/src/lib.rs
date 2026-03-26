#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bytes::Bytes;
    use cluster_config::ClusterConfig;
    use control_plane::{Catalog, InMemoryCatalog};
    use data_model::{Attribute, Mutation, Space, Subspace, Value};
    use data_plane::DataPlane;
    use engine_memory::MemoryEngine;
    use placement_core::HyperSpacePlacement;
    use storage_core::StorageEngine;

    #[test]
    fn data_plane_round_trip_with_memory_engine() {
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
            })
            .unwrap();

        let data_plane = DataPlane::new(catalog, storage, Arc::new(HyperSpacePlacement));
        data_plane
            .put(
                "profiles",
                Bytes::from_static(b"ada"),
                &[Mutation::Set(Attribute {
                    name: "name".to_owned(),
                    value: Value::String("Ada".to_owned()),
                })],
            )
            .unwrap();

        let record = data_plane.get("profiles", b"ada").unwrap().unwrap();

        assert_eq!(record.attributes.get("name"), Some(&Value::String("Ada".to_owned())));
    }
}
