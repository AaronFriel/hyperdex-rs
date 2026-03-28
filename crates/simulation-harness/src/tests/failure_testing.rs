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
