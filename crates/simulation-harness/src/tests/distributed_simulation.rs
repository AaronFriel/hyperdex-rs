use super::*;

fn direct_profile_mutations(key: &str, views: i64) -> Vec<Mutation> {
    vec![
        Mutation::Set(Attribute {
            name: "username".to_owned(),
            value: Value::Bytes(Bytes::from(key.as_bytes().to_vec())),
        }),
        Mutation::Set(Attribute {
            name: "profile_views".to_owned(),
            value: Value::Int(views),
        }),
    ]
}

async fn direct_replicated_put(runtime: &ClusterRuntime, key: &str, views: i64) {
    let response = runtime
        .handle_internode_request(
            InternodeRequest::encode(
                DATA_PLANE_METHOD,
                &DataPlaneRequest::ReplicatedPut {
                    space: "profiles".to_owned(),
                    key: Bytes::from(key.as_bytes().to_vec()),
                    mutations: direct_profile_mutations(key, views),
                },
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.decode::<DataPlaneResponse>().unwrap(),
        DataPlaneResponse::Unit
    );
}

async fn direct_replicated_delete(runtime: &ClusterRuntime, key: &str) {
    let response = runtime
        .handle_internode_request(
            InternodeRequest::encode(
                DATA_PLANE_METHOD,
                &DataPlaneRequest::ReplicatedDelete {
                    space: "profiles".to_owned(),
                    key: Bytes::from(key.as_bytes().to_vec()),
                },
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.decode::<DataPlaneResponse>().unwrap(),
        DataPlaneResponse::Unit
    );
}

async fn direct_local_profile_views(runtime: &ClusterRuntime, key: &str) -> Option<i64> {
    let response = runtime
        .handle_internode_request(
            InternodeRequest::encode(
                DATA_PLANE_METHOD,
                &DataPlaneRequest::Get {
                    space: "profiles".to_owned(),
                    key: Bytes::from(key.as_bytes().to_vec()),
                },
            )
            .unwrap(),
        )
        .await
        .unwrap();

    match response.decode::<DataPlaneResponse>().unwrap() {
        DataPlaneResponse::Record(Some(record)) => match record.attributes.get("profile_views") {
            Some(Value::Int(views)) => Some(*views),
            other => panic!("unexpected local profile_views attribute: {other:?}"),
        },
        DataPlaneResponse::Record(None) => None,
        other => panic!("unexpected local get response: {other:?}"),
    }
}

#[test]
fn turmoil_recovery_preserves_delete_group_retry_then_put_visibility_after_replica_outage() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (transport, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;

        let delete_key = (0..65536)
            .map(|i| format!("recovery-delete-group-{i}"))
            .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 1)
            .expect("expected a key routed to node 1");
        let survivor_key = format!("{delete_key}-survivor");

        for (key, views) in [(&delete_key, 61), (&survivor_key, 12)] {
            let put = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::from(key.as_bytes().to_vec()),
                    mutations: vec![Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(views),
                    })],
                },
            )
            .await
            .unwrap();
            assert_eq!(put, ClientResponse::Unit);
        }

        transport.set_unavailable(2, true).await;

        let failed_delete_group = HyperdexClientService::handle(
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
            failed_delete_group.is_err(),
            "expected replica outage to abort delete-group"
        );

        let during_outage = BTreeMap::from([(delete_key.clone(), 61_i64)]);
        assert_search_and_count_match_model(runtime1.as_ref(), 61, &during_outage).await;

        transport.set_unavailable(2, false).await;

        let retried_delete_group = HyperdexClientService::handle(
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
        .await
        .unwrap();
        assert_eq!(retried_delete_group, ClientResponse::Deleted(1));

        let replacement_put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(delete_key.as_bytes().to_vec()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(88),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(replacement_put, ClientResponse::Unit);

        let expected_new = BTreeMap::from([(delete_key.clone(), 88_i64)]);
        assert_search_and_count_match_model(runtime1.as_ref(), 61, &expected_new).await;
        assert_search_and_count_match_model(runtime2.as_ref(), 61, &expected_new).await;

        let recovered_record = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(delete_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        match recovered_record {
            ClientResponse::Record(Some(record)) => {
                assert_eq!(record.key, Bytes::from(delete_key.as_bytes().to_vec()));
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(88))
                );
            }
            other => panic!("unexpected recovered record after retry/put: {other:?}"),
        }

        let retained_record = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(survivor_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        match retained_record {
            ClientResponse::Record(Some(record)) => {
                assert_eq!(record.key, Bytes::from(survivor_key.as_bytes().to_vec()));
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(12))
                );
            }
            other => panic!("unexpected retained record after recovery: {other:?}"),
        }

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_recovery_preserves_delete_retry_then_put_visibility_after_replica_outage() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (transport, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;

        let delete_key = (0..65536)
            .map(|i| format!("recovery-delete-{i}"))
            .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 1)
            .expect("expected a key routed to node 1");
        let survivor_key = format!("{delete_key}-survivor");

        for (key, views) in [(&delete_key, 61), (&survivor_key, 12)] {
            let put = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::from(key.as_bytes().to_vec()),
                    mutations: vec![Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(views),
                    })],
                },
            )
            .await
            .unwrap();
            assert_eq!(put, ClientResponse::Unit);
        }

        transport.set_unavailable(2, true).await;

        let failed_delete = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Delete {
                space: "profiles".to_owned(),
                key: Bytes::from(delete_key.as_bytes().to_vec()),
            },
        )
        .await;
        assert!(
            failed_delete.is_err(),
            "expected replica outage to abort delete"
        );

        for runtime in [&runtime1, &runtime2] {
            assert_eq!(
                direct_local_profile_views(runtime.as_ref(), &delete_key).await,
                Some(61)
            );
            assert_eq!(
                direct_local_profile_views(runtime.as_ref(), &survivor_key).await,
                Some(12)
            );
        }

        let during_outage =
            BTreeMap::from([(delete_key.clone(), 61_i64), (survivor_key.clone(), 12_i64)]);
        assert_search_and_count_match_model(runtime1.as_ref(), 1, &during_outage).await;

        transport.set_unavailable(2, false).await;

        let retried_delete = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Delete {
                space: "profiles".to_owned(),
                key: Bytes::from(delete_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert_eq!(retried_delete, ClientResponse::Unit);

        for runtime in [&runtime1, &runtime2] {
            assert_eq!(
                direct_local_profile_views(runtime.as_ref(), &delete_key).await,
                None
            );
        }

        let after_delete = BTreeMap::from([(survivor_key.clone(), 12_i64)]);
        assert_search_and_count_match_model(runtime1.as_ref(), 1, &after_delete).await;
        assert_search_and_count_match_model(runtime2.as_ref(), 1, &after_delete).await;

        let replacement_put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(delete_key.as_bytes().to_vec()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(88),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(replacement_put, ClientResponse::Unit);

        let recovered_state =
            BTreeMap::from([(delete_key.clone(), 88_i64), (survivor_key.clone(), 12_i64)]);
        assert_search_and_count_match_model(runtime1.as_ref(), 1, &recovered_state).await;
        assert_search_and_count_match_model(runtime2.as_ref(), 1, &recovered_state).await;

        let recovered_record = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(delete_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        match recovered_record {
            ClientResponse::Record(Some(record)) => {
                assert_eq!(record.key, Bytes::from(delete_key.as_bytes().to_vec()));
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(88))
                );
            }
            other => panic!("unexpected recovered record after delete retry/put: {other:?}"),
        }

        let retained_record = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(survivor_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        match retained_record {
            ClientResponse::Record(Some(record)) => {
                assert_eq!(record.key, Bytes::from(survivor_key.as_bytes().to_vec()));
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(12))
                );
            }
            other => panic!("unexpected retained record after delete recovery: {other:?}"),
        }

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_delete_group_rejects_divergent_replica_snapshots() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (_, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;

        let mut divergent_keys = (0..65536)
            .map(|i| format!("divergent-delete-group-{i}"))
            .filter(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 1);
        let left_key = divergent_keys.next().expect("expected first divergent key");
        let right_key = divergent_keys
            .next()
            .expect("expected second divergent key");

        for key in [&left_key, &right_key] {
            let put = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::from(key.as_bytes().to_vec()),
                    mutations: vec![Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(61),
                    })],
                },
            )
            .await
            .unwrap();
            assert_eq!(put, ClientResponse::Unit);
        }

        direct_replicated_delete(runtime2.as_ref(), &left_key).await;
        direct_replicated_delete(runtime1.as_ref(), &right_key).await;

        assert_eq!(
            direct_local_profile_views(runtime1.as_ref(), &left_key).await,
            Some(61)
        );
        assert_eq!(
            direct_local_profile_views(runtime1.as_ref(), &right_key).await,
            None
        );
        assert_eq!(
            direct_local_profile_views(runtime2.as_ref(), &left_key).await,
            None
        );
        assert_eq!(
            direct_local_profile_views(runtime2.as_ref(), &right_key).await,
            Some(61)
        );

        let before_delete =
            BTreeMap::from([(left_key.clone(), 61_i64), (right_key.clone(), 61_i64)]);
        assert_search_and_count_match_model(runtime1.as_ref(), 61, &before_delete).await;

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
        let err = delete_group.expect_err("expected divergent snapshots to abort delete-group");
        assert!(
            err.to_string().contains("snapshot mismatch"),
            "unexpected delete-group error: {err:#}"
        );

        assert_eq!(
            direct_local_profile_views(runtime1.as_ref(), &left_key).await,
            Some(61)
        );
        assert_eq!(
            direct_local_profile_views(runtime1.as_ref(), &right_key).await,
            None
        );
        assert_eq!(
            direct_local_profile_views(runtime2.as_ref(), &left_key).await,
            None
        );
        assert_eq!(
            direct_local_profile_views(runtime2.as_ref(), &right_key).await,
            Some(61)
        );
        assert_search_and_count_match_model(runtime2.as_ref(), 61, &before_delete).await;

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_search_and_count_reject_divergent_replica_values_for_same_key() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (_, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;

        let key = (0..65536)
            .map(|i| format!("divergent-search-{i}"))
            .find(|candidate| runtime1.route_primary(candidate.as_bytes()).unwrap() == 1)
            .expect("expected a key routed to node 1");

        direct_replicated_put(runtime1.as_ref(), &key, 61).await;
        direct_replicated_put(runtime2.as_ref(), &key, 88).await;

        let search = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Search {
                space: "profiles".to_owned(),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::GreaterThanOrEqual,
                    value: Value::Int(0),
                }],
            },
        )
        .await;
        let search_err = search.expect_err("expected divergent replica values to abort search");
        assert!(
            search_err.to_string().contains("replica divergence"),
            "unexpected search error: {search_err:#}"
        );

        let count = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Count {
                space: "profiles".to_owned(),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::GreaterThanOrEqual,
                    value: Value::Int(0),
                }],
            },
        )
        .await;
        let count_err = count.expect_err("expected divergent replica values to abort count");
        assert!(
            count_err.to_string().contains("replica divergence"),
            "unexpected count error: {count_err:#}"
        );

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_delete_group_rolls_back_when_replica_snapshot_coverage_is_incomplete() {
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

        let mut runtime2 = ClusterRuntime::for_node(config, 2).unwrap();
        runtime2.install_cluster_transport(transport.clone(), TransportRuntime::Grpc);
        let runtime2 = Arc::new(runtime2);

        transport.register(1, runtime1.clone()).await;
        transport.register(2, runtime2.clone()).await;

        HyperdexAdminService::handle(
            runtime1.as_ref(),
            AdminRequest::CreateSpaceDsl(replicated_profiles_schema()),
        )
        .await
        .unwrap();

        let key = (0..65536)
            .map(|i| format!("partial-delete-group-{i}"))
            .find(|candidate| runtime1.route_primary(candidate.as_bytes()).unwrap() == 1)
            .expect("expected a key routed to node 1");

        direct_replicated_put(runtime1.as_ref(), &key, 61).await;
        assert_eq!(
            direct_local_profile_views(runtime1.as_ref(), &key).await,
            Some(61)
        );

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
        let err =
            delete_group.expect_err("expected incomplete replica coverage to abort delete-group");
        assert!(
            err.to_string()
                .contains("physical records across replica factor"),
            "unexpected delete-group error: {err:#}"
        );

        assert_eq!(
            direct_local_profile_views(runtime1.as_ref(), &key).await,
            Some(61)
        );
        let visible_after_failure = BTreeMap::from([(key.clone(), 61_i64)]);
        assert_search_and_count_match_model(runtime1.as_ref(), 61, &visible_after_failure).await;

        drop(runtime2);

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[cfg(madsim)]
#[test]
fn madsim_recovery_preserves_delete_group_retry_then_put_visibility_after_replica_outage() {
    let runtime = madsim::runtime::Runtime::with_seed_and_config(19, madsim::Config::default());

    runtime.block_on(async move {
        let (transport, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;

        let delete_key = (0..65536)
            .map(|i| format!("madsim-recovery-delete-group-{i}"))
            .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 1)
            .expect("expected a key routed to node 1");
        let survivor_key = format!("{delete_key}-survivor");

        for (key, views) in [(&delete_key, 61), (&survivor_key, 12)] {
            let put = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::from(key.as_bytes().to_vec()),
                    mutations: vec![Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(views),
                    })],
                },
            )
            .await
            .unwrap();
            assert_eq!(put, ClientResponse::Unit);
        }

        madsim::time::sleep(Duration::from_millis(5)).await;
        transport.set_unavailable(2, true).await;
        madsim::time::sleep(Duration::from_millis(5)).await;

        let failed_delete_group = HyperdexClientService::handle(
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
            failed_delete_group.is_err(),
            "expected replica outage to abort delete-group"
        );

        let during_outage = BTreeMap::from([(delete_key.clone(), 61_i64)]);
        assert_search_and_count_match_model(runtime1.as_ref(), 61, &during_outage).await;

        transport.set_unavailable(2, false).await;
        madsim::time::sleep(Duration::from_millis(5)).await;

        let retried_delete_group = HyperdexClientService::handle(
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
        .await
        .unwrap();
        assert_eq!(retried_delete_group, ClientResponse::Deleted(1));

        let replacement_put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(delete_key.as_bytes().to_vec()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(88),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(replacement_put, ClientResponse::Unit);

        let expected_new = BTreeMap::from([(delete_key.clone(), 88_i64)]);
        assert_search_and_count_match_model(runtime1.as_ref(), 61, &expected_new).await;
        assert_search_and_count_match_model(runtime2.as_ref(), 61, &expected_new).await;

        let recovered_record = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(delete_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        match recovered_record {
            ClientResponse::Record(Some(record)) => {
                assert_eq!(record.key, Bytes::from(delete_key.as_bytes().to_vec()));
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(88))
                );
            }
            other => panic!("unexpected recovered record after retry/put: {other:?}"),
        }

        let retained_record = HyperdexClientService::handle(
            runtime2.as_ref(),
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from(survivor_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        match retained_record {
            ClientResponse::Record(Some(record)) => {
                assert_eq!(record.key, Bytes::from(survivor_key.as_bytes().to_vec()));
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(12))
                );
            }
            other => panic!("unexpected retained record after recovery: {other:?}"),
        }
    });
}

#[cfg(madsim)]
#[test]
fn madsim_recovery_preserves_conditional_put_retry_then_delete_visibility_after_replica_outage() {
    let runtime = madsim::runtime::Runtime::with_seed_and_config(23, madsim::Config::default());

    runtime.block_on(async move {
        let (transport, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;

        let conditional_key = (0..65536)
            .map(|i| format!("madsim-recovery-conditional-put-{i}"))
            .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 1)
            .expect("expected a key routed to node 1");
        let survivor_key = format!("{conditional_key}-survivor");

        for (key, views) in [(&conditional_key, 19), (&survivor_key, 12)] {
            let put = HyperdexClientService::handle(
                runtime1.as_ref(),
                ClientRequest::Put {
                    space: "profiles".to_owned(),
                    key: Bytes::from(key.as_bytes().to_vec()),
                    mutations: vec![Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(views),
                    })],
                },
            )
            .await
            .unwrap();
            assert_eq!(put, ClientResponse::Unit);
        }

        madsim::time::sleep(Duration::from_millis(5)).await;
        transport.set_unavailable(2, true).await;
        madsim::time::sleep(Duration::from_millis(5)).await;

        let failed_conditional_put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::ConditionalPut {
                space: "profiles".to_owned(),
                key: Bytes::from(conditional_key.as_bytes().to_vec()),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::Equal,
                    value: Value::Int(19),
                }],
                mutations: vec![
                    Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(30),
                    }),
                    Mutation::Numeric {
                        attribute: "profile_views".to_owned(),
                        op: data_model::NumericOp::Add,
                        operand: 5,
                    },
                ],
            },
        )
        .await;
        assert!(
            failed_conditional_put.is_err(),
            "expected replica outage to abort conditional put"
        );

        for runtime in [&runtime1, &runtime2] {
            assert_eq!(
                direct_local_profile_views(runtime.as_ref(), &conditional_key).await,
                Some(19)
            );
            assert_eq!(
                direct_local_profile_views(runtime.as_ref(), &survivor_key).await,
                Some(12)
            );
        }

        let during_outage = BTreeMap::from([
            (conditional_key.clone(), 19_i64),
            (survivor_key.clone(), 12_i64),
        ]);
        assert_search_and_count_match_model(runtime1.as_ref(), 1, &during_outage).await;
        assert_search_and_count_match_model(runtime2.as_ref(), 1, &during_outage).await;

        transport.set_unavailable(2, false).await;
        madsim::time::sleep(Duration::from_millis(5)).await;

        let retried_conditional_put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::ConditionalPut {
                space: "profiles".to_owned(),
                key: Bytes::from(conditional_key.as_bytes().to_vec()),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::Equal,
                    value: Value::Int(19),
                }],
                mutations: vec![
                    Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(30),
                    }),
                    Mutation::Numeric {
                        attribute: "profile_views".to_owned(),
                        op: data_model::NumericOp::Add,
                        operand: 5,
                    },
                ],
            },
        )
        .await
        .unwrap();
        assert_eq!(retried_conditional_put, ClientResponse::Unit);

        for runtime in [&runtime1, &runtime2] {
            assert_eq!(
                direct_local_profile_views(runtime.as_ref(), &conditional_key).await,
                Some(35)
            );
        }

        let after_retry = BTreeMap::from([
            (conditional_key.clone(), 35_i64),
            (survivor_key.clone(), 12_i64),
        ]);
        assert_search_and_count_match_model(runtime1.as_ref(), 1, &after_retry).await;
        assert_search_and_count_match_model(runtime2.as_ref(), 1, &after_retry).await;

        let delete = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Delete {
                space: "profiles".to_owned(),
                key: Bytes::from(conditional_key.as_bytes().to_vec()),
            },
        )
        .await
        .unwrap();
        assert_eq!(delete, ClientResponse::Unit);

        for runtime in [&runtime1, &runtime2] {
            assert_eq!(
                direct_local_profile_views(runtime.as_ref(), &conditional_key).await,
                None
            );
            assert_eq!(
                direct_local_profile_views(runtime.as_ref(), &survivor_key).await,
                Some(12)
            );
        }

        let after_delete = BTreeMap::from([(survivor_key.clone(), 12_i64)]);
        assert_search_and_count_match_model(runtime1.as_ref(), 1, &after_delete).await;
        assert_search_and_count_match_model(runtime2.as_ref(), 1, &after_delete).await;
    });
}

#[cfg(madsim)]
#[test]
fn madsim_recovery_preserves_operation_order_after_stale_local_primary_rejoin() {
    let runtime = madsim::runtime::Runtime::with_seed_and_config(29, madsim::Config::default());

    runtime.block_on(async move {
        let (transport, current_runtime, stale_runtime) =
            distributed_runtime_fixture_with_diverged_cluster_views(profiles_schema()).await;

        let (current_primary, stale_primary, recovery_key) = stale_placement_mutation_target(
            &current_runtime,
            &stale_runtime,
            "madsim-stale-recovery-ordering",
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
    });
}

#[cfg(madsim)]
#[test]
fn madsim_recovery_preserves_delete_then_rewrite_visibility_after_stale_local_primary_rejoin() {
    let runtime = madsim::runtime::Runtime::with_seed_and_config(31, madsim::Config::default());

    runtime.block_on(async move {
        let (transport, current_runtime, stale_runtime) =
            distributed_runtime_fixture_with_diverged_cluster_views(profiles_schema()).await;

        let (current_primary, stale_primary, recovery_key) = stale_placement_mutation_target(
            &current_runtime,
            &stale_runtime,
            "madsim-stale-recovery-delete-rewrite",
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
        madsim::time::sleep(Duration::from_millis(5)).await;

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
    });
}
