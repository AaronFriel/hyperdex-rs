use super::*;

#[test]
fn turmoil_rejects_stale_local_conditional_put_across_peer_outage_and_recovery() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (transport, runtime1, runtime2) =
            distributed_runtime_fixture_with_diverged_cluster_views(profiles_schema()).await;

        let (runtime1_primary, runtime2_primary, stale_key) =
            stale_local_primary_target_for_authoritative_node(
                &runtime1,
                &runtime2,
                "stale-local-primary-conditional-put",
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
                    value: Value::Int(127),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(initial_put, ClientResponse::Unit);

        transport.set_unavailable(runtime2.local_node_id(), true).await;

        let conditional_put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::ConditionalPut {
                space: "profiles".to_owned(),
                key: Bytes::from(stale_key.as_bytes().to_vec()),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::Equal,
                    value: Value::Int(127),
                }],
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(211),
                })],
            },
        )
        .await;
        assert!(
            conditional_put.is_err(),
            "expected stale local primary conditional put to fail while the authoritative peer is temporarily unavailable"
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
                    Some(&Value::Int(127))
                );
            }
            other => {
                panic!("unexpected authoritative record after stale conditional put attempt: {other:?}")
            }
        }

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_rejects_divergent_replica_conditional_put() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (_, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;

        let divergent_key = (0..65536)
            .map(|i| format!("divergent-conditional-put-{i}"))
            .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 2)
            .expect("expected a key routed to node 2");

        let put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(divergent_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(17),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(put, ClientResponse::Unit);

        runtime1
            .handle_internode_request(
                InternodeRequest::encode(
                    DATA_PLANE_METHOD,
                    &DataPlaneRequest::ReplicatedPut {
                        space: "profiles".to_owned(),
                        key: Bytes::from(divergent_key.as_bytes().to_vec()),
                        mutations: vec![
                            Mutation::Set(Attribute {
                                name: "username".to_owned(),
                                value: Value::String(divergent_key.clone()),
                            }),
                            Mutation::Set(Attribute {
                                name: "profile_views".to_owned(),
                                value: Value::Int(29),
                            }),
                        ],
                    },
                )
                .unwrap(),
            )
            .await
            .unwrap();

        let conditional_put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::ConditionalPut {
                space: "profiles".to_owned(),
                key: Bytes::from(divergent_key.as_bytes().to_vec()),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::Equal,
                    value: Value::Int(17),
                }],
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(41),
                })],
            },
        )
        .await;
        let err = conditional_put.expect_err(
            "expected conditional put to fail closed when replicas disagree on the current record",
        );
        assert!(
            err.to_string().contains("divergent replica state"),
            "expected divergence error, got {err:?}"
        );

        let node1_after = runtime1
            .handle_internode_request(
                InternodeRequest::encode(
                    DATA_PLANE_METHOD,
                    &DataPlaneRequest::Get {
                        space: "profiles".to_owned(),
                        key: Bytes::from(divergent_key.as_bytes().to_vec()),
                    },
                )
                .unwrap(),
            )
            .await
            .unwrap()
            .decode::<DataPlaneResponse>()
            .unwrap();
        match node1_after {
            DataPlaneResponse::Record(Some(record)) => {
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(29))
                );
            }
            other => {
                panic!("unexpected divergent node1 record after failed conditional put: {other:?}")
            }
        }

        let node2_after = runtime2
            .handle_internode_request(
                InternodeRequest::encode(
                    DATA_PLANE_METHOD,
                    &DataPlaneRequest::Get {
                        space: "profiles".to_owned(),
                        key: Bytes::from(divergent_key.as_bytes().to_vec()),
                    },
                )
                .unwrap(),
            )
            .await
            .unwrap()
            .decode::<DataPlaneResponse>()
            .unwrap();
        match node2_after {
            DataPlaneResponse::Record(Some(record)) => {
                assert_eq!(
                    record.attributes.get("profile_views"),
                    Some(&Value::Int(17))
                );
            }
            other => panic!(
                "unexpected authoritative node2 record after failed conditional put: {other:?}"
            ),
        }

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}

#[test]
fn turmoil_recovers_mixed_conditional_put_after_replica_outage_without_partial_visibility() {
    let mut sim = turmoil::Builder::new().build();

    sim.client("cluster", async move {
        let (transport, runtime1, runtime2) =
            distributed_runtime_fixture_with_schema(replicated_profiles_schema()).await;

        let failing_key = (0..65536)
            .map(|i| format!("replica-failure-mixed-conditional-put-{i}"))
            .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 1)
            .expect("expected a key routed to node 1");

        let initial_put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.clone().into_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "profile_views".to_owned(),
                    value: Value::Int(19),
                })],
            },
        )
        .await
        .unwrap();
        assert_eq!(initial_put, ClientResponse::Unit);

        transport.set_unavailable(2, true).await;

        let failed_conditional_put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::ConditionalPut {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.as_bytes().to_vec()),
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
            "expected replica outage to abort mixed conditional put"
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
                    Some(&Value::Int(19))
                );
            }
            other => panic!("unexpected local mixed conditional-put record result: {other:?}"),
        }

        let expected_during_outage = BTreeMap::from([(failing_key.clone(), 19_i64)]);
        assert_search_and_count_match_model(runtime1.as_ref(), 19, &expected_during_outage).await;
        assert_search_and_count_match_model(runtime1.as_ref(), 20, &BTreeMap::new()).await;

        transport.set_unavailable(2, false).await;

        let retried_conditional_put = HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::ConditionalPut {
                space: "profiles".to_owned(),
                key: Bytes::from(failing_key.as_bytes().to_vec()),
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
            let recovered_record = HyperdexClientService::handle(
                runtime.as_ref(),
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
                        Some(&Value::Int(35))
                    );
                }
                other => panic!("unexpected recovered mixed conditional-put record: {other:?}"),
            }
        }

        let expected_after_recovery = BTreeMap::from([(failing_key.clone(), 35_i64)]);
        assert_search_and_count_match_model(runtime1.as_ref(), 30, &expected_after_recovery).await;
        assert_search_and_count_match_model(runtime2.as_ref(), 30, &expected_after_recovery).await;

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    sim.run().unwrap();
}
