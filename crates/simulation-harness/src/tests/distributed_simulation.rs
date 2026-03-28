use super::*;

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
