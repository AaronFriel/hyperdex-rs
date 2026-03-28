use super::*;
use bytes::Bytes;
use cluster_config::{
    ClusterNode, ConsensusBackend, PlacementBackend, StorageBackend, TransportBackend,
};
use data_model::{Attribute, Check, Mutation, Predicate, Value};
use hyperdex_admin_protocol::{
    AdminRequest, AdminResponse, BusyBeeFrame, ConfigView, CoordinatorAdminRequest,
    CoordinatorReturnCode, HyperdexAdminService, LegacyAdminRequest, LegacyAdminReturnCode,
    ReplicantAdminRequestMessage, ReplicantCallCompletion, ReplicantConditionCompletion,
    ReplicantReturnCode,
};
use hyperdex_client_protocol::HyperdexClientService;
use legacy_protocol::{
    decode_protocol_atomic_response, decode_protocol_count_response, decode_protocol_get_response,
    decode_protocol_search_item, encode_protocol_atomic_request, encode_protocol_count_request,
    encode_protocol_get_request, encode_protocol_search_continue, encode_protocol_search_start,
    LegacyMessageType, LegacyReturnCode, ProtocolAttributeCheck, ProtocolFuncall,
    ProtocolKeyChange, ProtocolSearchStart, RequestHeader, FUNC_NUM_ADD, FUNC_SET,
    HYPERDATATYPE_INT64, HYPERDATATYPE_STRING, HYPERPREDICATE_GREATER_EQUAL,
};
use std::sync::Arc;
use std::time::Duration;

fn bootstrap_runtime() -> ClusterRuntime {
    super::bootstrap_runtime().unwrap()
}

fn legacy_request_body(nonce: u64, body: Vec<u8>) -> Vec<u8> {
    let mut request = nonce.to_be_bytes().to_vec();
    request.extend_from_slice(&body);
    request
}

#[test]
fn legacy_decode_request_nonce_rejects_truncated_nonce() {
    let err = legacy_decode_request_nonce(&[1, 2, 3]).expect_err("truncated nonce should fail");
    assert!(err.to_string().contains("missing nonce"));
}

#[test]
fn legacy_decode_container_value_rejects_truncated_string_length() {
    let err = legacy_decode_container_value(&ValueKind::Bytes, &[1, 2, 3])
        .expect_err("truncated container length should fail");
    assert!(err.to_string().contains("truncated"));
}

fn dsl_space_add_decoder(bytes: &[u8]) -> Result<Space> {
    Ok(parse_hyperdex_space(std::str::from_utf8(bytes)?)?)
}

async fn read_admin_response_frame(stream: &mut tokio::net::TcpStream) -> BusyBeeFrame {
    read_busybee_frame_from_stream(stream)
        .await
        .unwrap()
        .expect("expected admin response frame")
}

#[derive(Debug, PartialEq, Eq)]
struct DecodedLegacyConfig {
    version: u64,
    server_ids: Vec<u64>,
    spaces: Vec<DecodedLegacySpace>,
}

#[derive(Debug, PartialEq, Eq)]
struct DecodedLegacySpace {
    name: String,
    attributes: Vec<(String, u16)>,
}

fn decode_legacy_config(bytes: &[u8]) -> DecodedLegacyConfig {
    let mut cursor = 0;
    let _cluster = decode_u64(bytes, &mut cursor);
    let version = decode_u64(bytes, &mut cursor);
    let _flags = decode_u64(bytes, &mut cursor);
    let servers_len = decode_u64(bytes, &mut cursor) as usize;
    let spaces_len = decode_u64(bytes, &mut cursor) as usize;
    let transfers_len = decode_u64(bytes, &mut cursor) as usize;
    let mut server_ids = Vec::with_capacity(servers_len);
    let mut spaces = Vec::with_capacity(spaces_len);

    for _ in 0..servers_len {
        let _state = decode_u8(bytes, &mut cursor);
        server_ids.push(decode_u64(bytes, &mut cursor));
        skip_legacy_location(bytes, &mut cursor);
    }

    for _ in 0..spaces_len {
        spaces.push(decode_legacy_space(bytes, &mut cursor));
    }

    for _ in 0..transfers_len {
        cursor += 6 * std::mem::size_of::<u64>();
    }

    assert_eq!(
        cursor,
        bytes.len(),
        "expected legacy config payload to be fully consumed"
    );

    DecodedLegacyConfig {
        version,
        server_ids,
        spaces,
    }
}

fn decode_legacy_space(bytes: &[u8], cursor: &mut usize) -> DecodedLegacySpace {
    let _space_id = decode_u64(bytes, cursor);
    let name = decode_varint_string(bytes, cursor);
    let _fault_tolerance = decode_u64(bytes, cursor);
    let attrs_len = decode_u16(bytes, cursor) as usize;
    let subspaces_len = decode_u16(bytes, cursor) as usize;
    let indices_len = decode_u16(bytes, cursor) as usize;
    let mut attributes = Vec::with_capacity(attrs_len);

    for _ in 0..attrs_len {
        let attr_name = decode_varint_string(bytes, cursor);
        let datatype = decode_u16(bytes, cursor);
        attributes.push((attr_name, datatype));
    }

    for _ in 0..subspaces_len {
        let _subspace_id = decode_u64(bytes, cursor);
        let attrs_len = decode_u16(bytes, cursor) as usize;
        let regions_len = decode_u32(bytes, cursor) as usize;

        for _ in 0..attrs_len {
            let _attr = decode_u16(bytes, cursor);
        }

        for _ in 0..regions_len {
            let _region_id = decode_u64(bytes, cursor);
            let bounds_len = decode_u16(bytes, cursor) as usize;
            let replicas_len = decode_u8(bytes, cursor) as usize;

            for _ in 0..bounds_len {
                let _lower = decode_u64(bytes, cursor);
                let _upper = decode_u64(bytes, cursor);
            }

            for _ in 0..replicas_len {
                let _server_id = decode_u64(bytes, cursor);
                let _virtual_server_id = decode_u64(bytes, cursor);
            }
        }
    }

    for _ in 0..indices_len {
        let _index_type = decode_u8(bytes, cursor);
        let _index_id = decode_u64(bytes, cursor);
        let _attr = decode_u16(bytes, cursor);
        let _extra = decode_varint_bytes(bytes, cursor);
    }

    DecodedLegacySpace { name, attributes }
}

fn skip_legacy_location(bytes: &[u8], cursor: &mut usize) {
    let family = decode_u8(bytes, cursor);
    let addr_len = match family {
        4 => 4,
        6 => 16,
        other => panic!("unexpected legacy location family {other}"),
    };
    *cursor += addr_len + std::mem::size_of::<u16>();
}

fn decode_varint_string(bytes: &[u8], cursor: &mut usize) -> String {
    String::from_utf8(decode_varint_bytes(bytes, cursor)).unwrap()
}

fn decode_varint_bytes(bytes: &[u8], cursor: &mut usize) -> Vec<u8> {
    let len = decode_varint(bytes, cursor) as usize;
    let start = *cursor;
    let end = start + len;
    let value = bytes[start..end].to_vec();
    *cursor = end;
    value
}

fn decode_u64(bytes: &[u8], cursor: &mut usize) -> u64 {
    let start = *cursor;
    let end = start + std::mem::size_of::<u64>();
    let value = u64::from_be_bytes(bytes[start..end].try_into().unwrap());
    *cursor = end;
    value
}

fn decode_u32(bytes: &[u8], cursor: &mut usize) -> u32 {
    let start = *cursor;
    let end = start + std::mem::size_of::<u32>();
    let value = u32::from_be_bytes(bytes[start..end].try_into().unwrap());
    *cursor = end;
    value
}

fn decode_varint(bytes: &[u8], cursor: &mut usize) -> u64 {
    let mut shift = 0_u32;
    let mut value = 0_u64;

    loop {
        let byte = bytes[*cursor];
        *cursor += 1;
        value |= u64::from(byte & 0x7f) << shift;

        if byte & 0x80 == 0 {
            return value;
        }

        shift += 7;
        assert!(shift < 64, "legacy config varint should fit in u64");
    }
}

fn decode_u16(bytes: &[u8], cursor: &mut usize) -> u16 {
    let start = *cursor;
    let end = start + std::mem::size_of::<u16>();
    let value = u16::from_be_bytes(bytes[start..end].try_into().unwrap());
    *cursor = end;
    value
}

fn decode_u8(bytes: &[u8], cursor: &mut usize) -> u8 {
    let value = bytes[*cursor];
    *cursor += 1;
    value
}

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

#[test]
fn runtime_rejects_missing_local_node() {
    let mut config = ClusterConfig::default();
    config.nodes = vec![
        ClusterNode {
            id: 1,
            host: "127.0.0.1".to_owned(),
            control_port: 1982,
            data_port: 2012,
        },
        ClusterNode {
            id: 2,
            host: "127.0.0.1".to_owned(),
            control_port: 1983,
            data_port: 2013,
        },
    ];

    let err = ClusterRuntime::for_node(config, 9)
        .err()
        .expect("missing local node should be rejected")
        .to_string();

    assert!(err.contains("local node 9"));
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
async fn runtime_dump_config_tracks_space_lifecycle_and_stability() {
    let mut config = ClusterConfig::default();
    config.consensus = ConsensusBackend::Mirror;
    config.placement = PlacementBackend::Rendezvous;
    config.storage = StorageBackend::Memory;
    config.internode_transport = TransportBackend::Grpc;
    let runtime = ClusterRuntime::single_node(config.clone()).unwrap();

    let response = HyperdexAdminService::handle(&runtime, AdminRequest::DumpConfig)
        .await
        .unwrap();
    assert_eq!(
        response,
        AdminResponse::Config(ConfigView {
            version: 0,
            stable_through: 0,
            cluster: config.clone(),
            spaces: Vec::new(),
        })
    );

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

    let response = HyperdexAdminService::handle(&runtime, AdminRequest::DumpConfig)
        .await
        .unwrap();
    let AdminResponse::Config(config_view) = response else {
        panic!("expected config response after create");
    };
    assert_eq!(config_view.version, 1);
    assert_eq!(config_view.stable_through, 1);
    assert_eq!(config_view.cluster, config);
    assert_eq!(config_view.spaces.len(), 1);
    assert_eq!(config_view.spaces[0].name, "profiles");
    assert_eq!(
        HyperdexAdminService::handle(&runtime, AdminRequest::WaitUntilStable)
            .await
            .unwrap(),
        AdminResponse::Stable { version: 1 }
    );

    HyperdexAdminService::handle(&runtime, AdminRequest::DropSpace("profiles".to_owned()))
        .await
        .unwrap();

    let response = HyperdexAdminService::handle(&runtime, AdminRequest::DumpConfig)
        .await
        .unwrap();
    assert_eq!(
        response,
        AdminResponse::Config(ConfigView {
            version: 2,
            stable_through: 2,
            cluster: config,
            spaces: Vec::new(),
        })
    );
    assert_eq!(
        HyperdexAdminService::handle(&runtime, AdminRequest::WaitUntilStable)
            .await
            .unwrap(),
        AdminResponse::Stable { version: 2 }
    );
}

#[tokio::test]
async fn runtime_register_daemon_updates_config_and_layout() {
    let runtime = ClusterRuntime::single_node(coordinator_cluster_config()).unwrap();

    assert_eq!(runtime.catalog.layout().unwrap().nodes, Vec::<u64>::new());

    HyperdexAdminService::handle(
        &runtime,
        AdminRequest::RegisterDaemon(ClusterNode {
            id: 4,
            host: "10.0.0.4".to_owned(),
            control_port: 2982,
            data_port: 3012,
        }),
    )
    .await
    .unwrap();
    HyperdexAdminService::handle(
        &runtime,
        AdminRequest::RegisterDaemon(ClusterNode {
            id: 9,
            host: "10.0.0.9".to_owned(),
            control_port: 3982,
            data_port: 4012,
        }),
    )
    .await
    .unwrap();

    assert_eq!(runtime.catalog.layout().unwrap().nodes, vec![4, 9]);

    let AdminResponse::Config(config_view) =
        HyperdexAdminService::handle(&runtime, AdminRequest::DumpConfig)
            .await
            .unwrap()
    else {
        panic!("expected config response after daemon registration");
    };
    assert_eq!(config_view.version, 2);
    assert_eq!(config_view.stable_through, 2);
    assert_eq!(
        config_view.cluster.nodes,
        vec![
            ClusterNode {
                id: 4,
                host: "10.0.0.4".to_owned(),
                control_port: 2982,
                data_port: 3012,
            },
            ClusterNode {
                id: 9,
                host: "10.0.0.9".to_owned(),
                control_port: 3982,
                data_port: 4012,
            },
        ]
    );
}

#[tokio::test]
async fn apply_config_view_removes_departed_nodes_from_routing_layout() {
    let stale_config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "10.0.0.1".to_owned(),
                control_port: 1982,
                data_port: 2012,
            },
            ClusterNode {
                id: 2,
                host: "10.0.0.2".to_owned(),
                control_port: 2982,
                data_port: 3012,
            },
            ClusterNode {
                id: 3,
                host: "10.0.0.3".to_owned(),
                control_port: 3982,
                data_port: 4012,
            },
        ],
        replicas: 1,
        ..ClusterConfig::default()
    };
    let runtime = ClusterRuntime::for_node(stale_config, 2).unwrap();
    let profiles = parse_hyperdex_space(
        "space profiles\n\
         key username\n\
         attributes\n\
            string first,\n\
            int profile_views\n\
         tolerate 0 failures\n",
    )
    .unwrap();
    HyperdexAdminService::handle(&runtime, AdminRequest::CreateSpace(profiles.clone()))
        .await
        .unwrap();

    let stale_key = (0..65536)
        .map(|i| format!("rejoin-routing-{i}"))
        .find(|key| runtime.route_primary_for_space("profiles", key.as_bytes()).unwrap() == 3)
        .expect("expected a key routed to departed node 3 before convergence");

    runtime
        .apply_config_view(&ConfigView {
            version: 7,
            stable_through: 7,
            cluster: ClusterConfig {
                nodes: vec![
                    ClusterNode {
                        id: 1,
                        host: "10.0.0.1".to_owned(),
                        control_port: 1982,
                        data_port: 2012,
                    },
                    ClusterNode {
                        id: 2,
                        host: "10.0.0.2".to_owned(),
                        control_port: 2982,
                        data_port: 3012,
                    },
                ],
                replicas: 1,
                ..ClusterConfig::default()
            },
            spaces: vec![profiles],
        })
        .unwrap();

    assert_eq!(runtime.catalog.layout().unwrap().nodes, vec![1, 2]);
    assert_ne!(
        runtime
            .route_primary_for_space("profiles", stale_key.as_bytes())
            .unwrap(),
        3
    );
}

#[tokio::test]
async fn legacy_admin_space_add_success_maps_to_hyperdex_status() {
    let runtime = bootstrap_runtime();

    let status = handle_legacy_admin_request(
        &runtime,
        LegacyAdminRequest::SpaceAddDsl(
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

    assert_eq!(status, LegacyAdminReturnCode::Success);
}

#[tokio::test]
async fn legacy_admin_space_add_duplicate_maps_to_hyperdex_status() {
    let runtime = bootstrap_runtime();
    let request = LegacyAdminRequest::SpaceAddDsl(
        "space profiles\n\
         key username\n\
         attributes\n\
            string first,\n\
            int profile_views\n\
         tolerate 0 failures\n"
            .to_owned(),
    );

    assert_eq!(
        handle_legacy_admin_request(&runtime, request.clone())
            .await
            .unwrap(),
        LegacyAdminReturnCode::Success
    );
    assert_eq!(
        handle_legacy_admin_request(&runtime, request)
            .await
            .unwrap(),
        LegacyAdminReturnCode::Duplicate
    );
}

#[tokio::test]
async fn legacy_admin_space_add_bad_schema_maps_to_badspace() {
    let runtime = bootstrap_runtime();

    let status = handle_legacy_admin_request(
        &runtime,
        LegacyAdminRequest::SpaceAddDsl("space broken".to_owned()),
    )
    .await
    .unwrap();

    assert_eq!(status, LegacyAdminReturnCode::BadSpace);
}

#[tokio::test]
async fn legacy_admin_space_rm_missing_maps_to_notfound() {
    let runtime = bootstrap_runtime();

    let status =
        handle_legacy_admin_request(&runtime, LegacyAdminRequest::SpaceRm("profiles".to_owned()))
            .await
            .unwrap();

    assert_eq!(status, LegacyAdminReturnCode::NotFound);
}

#[tokio::test]
async fn replicant_space_add_request_maps_to_call_completion() {
    let runtime = bootstrap_runtime();
    let response = handle_replicant_admin_request(
        &runtime,
        ReplicantAdminRequestMessage::space_add(41, encode_test_space_payload()),
    )
    .await
    .unwrap();
    let completion = ReplicantCallCompletion::decode(&response).unwrap();

    assert_eq!(completion.nonce, 41);
    assert_eq!(completion.status, ReplicantReturnCode::Success);
    assert_eq!(
        CoordinatorReturnCode::decode(&completion.output).unwrap(),
        CoordinatorReturnCode::Success
    );
}

#[tokio::test]
async fn replicant_wait_until_stable_maps_to_condition_completion() {
    let runtime = bootstrap_runtime();
    let response = handle_replicant_admin_request(
        &runtime,
        ReplicantAdminRequestMessage::wait_until_stable(7, 0),
    )
    .await
    .unwrap();
    let completion = ReplicantConditionCompletion::decode(&response).unwrap();

    assert_eq!(completion.nonce, 7);
    assert_eq!(completion.status, ReplicantReturnCode::Success);
    assert_eq!(
        completion.state,
        legacy_condition_state(runtime.stable_version().unwrap())
    );
    assert!(completion.data.is_empty());
}

#[tokio::test]
async fn replicant_config_get_maps_to_packed_condition_completion() {
    let runtime = bootstrap_runtime();
    let response = handle_replicant_admin_request(
        &runtime,
        ReplicantAdminRequestMessage::CondWait {
            nonce: 9,
            object: b"hyperdex".to_vec(),
            condition: b"config".to_vec(),
            state: 0,
        },
    )
    .await
    .unwrap();
    let completion = ReplicantConditionCompletion::decode(&response).unwrap();
    let config = decode_legacy_config(&completion.data);

    assert_eq!(completion.nonce, 9);
    assert_eq!(completion.status, ReplicantReturnCode::Success);
    assert_eq!(completion.state, 1);
    assert_eq!(config.version, 1);
    assert_eq!(config.server_ids, vec![1]);
    assert!(config.spaces.is_empty());
}

#[test]
fn legacy_bootstrap_response_matches_replicant_sender_identity_contract() {
    let address: std::net::SocketAddr = "127.0.0.1:1982".parse().unwrap();
    let response = ReplicantBootstrapResponse {
        server: ReplicantBootstrapServer {
            id: LEGACY_COORDINATOR_SERVER_ID,
            address,
        },
        configuration: legacy_bootstrap_configuration(LEGACY_COORDINATOR_SERVER_ID, address),
    };

    let sender_id = LEGACY_COORDINATOR_SERVER_ID;
    assert_eq!(sender_id, response.server.id);
    assert!(response
        .configuration
        .servers
        .iter()
        .any(|server| server.id == response.server.id));
}

#[tokio::test]
async fn coordinator_admin_space_rm_maps_to_exact_coordinator_code() {
    let runtime = bootstrap_runtime();

    let status = handle_coordinator_admin_request(
        &runtime,
        CoordinatorAdminRequest::SpaceRm("profiles".to_owned()),
    )
    .await
    .unwrap();

    assert_eq!(status, CoordinatorReturnCode::NotFound);
    assert_eq!(
        CoordinatorReturnCode::decode(&status.encode()).unwrap(),
        CoordinatorReturnCode::NotFound
    );
}

#[tokio::test]
async fn coordinator_admin_method_dispatch_returns_wire_bytes() {
    let runtime = bootstrap_runtime();

    let bytes = handle_coordinator_admin_method(
        &runtime,
        "space_rm",
        CoordinatorAdminRequest::SpaceRm("profiles".to_owned()),
    )
    .await
    .unwrap();
    assert_eq!(
        CoordinatorReturnCode::decode(&bytes).unwrap(),
        CoordinatorReturnCode::NotFound
    );

    let malformed = handle_coordinator_admin_method(
        &runtime,
        "space_rm",
        CoordinatorAdminRequest::SpaceAdd(
            parse_hyperdex_space(
                "space profiles\n\
             key username\n\
             attributes\n\
                string first\n\
             tolerate 0 failures\n",
            )
            .unwrap(),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        CoordinatorReturnCode::decode(&malformed).unwrap(),
        CoordinatorReturnCode::Malformed
    );
}

#[tokio::test]
async fn coordinator_control_service_routes_space_add_over_tcp() {
    let runtime = Arc::new(bootstrap_runtime());
    let service = CoordinatorControlService::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let address = service.local_addr().unwrap();

    let server = tokio::spawn(async move {
        service
            .serve_once_with(move |method, request| {
                let runtime = runtime.clone();
                async move {
                    handle_coordinator_control_method(runtime.as_ref(), &method, request).await
                }
            })
            .await
            .unwrap()
    });

    let response = request_coordinator_control_once(
        address,
        "space_add",
        &CoordinatorAdminRequest::SpaceAdd(
            parse_hyperdex_space(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n",
            )
            .unwrap(),
        ),
    )
    .await
    .unwrap();

    server.await.unwrap();
    assert_eq!(
        CoordinatorReturnCode::decode(&response).unwrap(),
        CoordinatorReturnCode::Success
    );
}

fn encode_test_space_payload() -> Vec<u8> {
    let mut out = Vec::new();
    encode_u64(&mut out, 0);
    encode_slice(&mut out, b"profiles");
    encode_u64(&mut out, 2);
    encode_u16(&mut out, 3);
    encode_u16(&mut out, 2);
    encode_u16(&mut out, 1);

    encode_slice(&mut out, b"username");
    encode_u16(&mut out, 9217);
    encode_slice(&mut out, b"first");
    encode_u16(&mut out, 9217);
    encode_slice(&mut out, b"profile_views");
    encode_u16(&mut out, 9218);

    encode_subspace(&mut out, 0, &[0], 4);
    encode_subspace(&mut out, 1, &[2], 4);

    encode_u8(&mut out, 0);
    encode_u64(&mut out, 0);
    encode_u16(&mut out, 2);
    encode_slice(&mut out, b"");

    out
}

fn encode_subspace(out: &mut Vec<u8>, id: u64, attrs: &[u16], partitions: u32) {
    encode_u64(out, id);
    encode_u16(out, attrs.len() as u16);
    encode_u32(out, partitions);
    for attr in attrs {
        encode_u16(out, *attr);
    }
    for partition in 0..partitions {
        encode_u64(out, partition as u64);
        encode_u16(out, 1);
        encode_u8(out, 0);
        encode_u64(out, partition as u64);
        encode_u64(out, partition as u64);
    }
}

fn encode_slice(out: &mut Vec<u8>, bytes: &[u8]) {
    encode_varint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

fn encode_varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;

        if value != 0 {
            byte |= 0x80;
        }

        out.push(byte);

        if value == 0 {
            break;
        }
    }
}

fn encode_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn encode_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn encode_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn encode_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

#[tokio::test]
async fn coordinator_control_service_registers_multiple_daemons_over_tcp() {
    let runtime = Arc::new(ClusterRuntime::single_node(coordinator_cluster_config()).unwrap());

    for node in [
        ClusterNode {
            id: 2,
            host: "10.0.0.2".to_owned(),
            control_port: 2982,
            data_port: 3012,
        },
        ClusterNode {
            id: 8,
            host: "10.0.0.8".to_owned(),
            control_port: 3982,
            data_port: 4012,
        },
    ] {
        let service = CoordinatorControlService::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let address = service.local_addr().unwrap();
        let runtime_for_server = runtime.clone();

        let server = tokio::spawn(async move {
            service
                .serve_once_with(move |method, request| {
                    let runtime = runtime_for_server.clone();
                    async move {
                        handle_coordinator_control_method(runtime.as_ref(), &method, request).await
                    }
                })
                .await
                .unwrap()
        });

        let response = request_coordinator_control_once(
            address,
            "daemon_register",
            &CoordinatorAdminRequest::DaemonRegister(node),
        )
        .await
        .unwrap();

        server.await.unwrap();
        assert_eq!(
            CoordinatorReturnCode::decode(&response).unwrap(),
            CoordinatorReturnCode::Success
        );
    }

    let AdminResponse::Config(config_view) =
        HyperdexAdminService::handle(runtime.as_ref(), AdminRequest::DumpConfig)
            .await
            .unwrap()
    else {
        panic!("expected config response after daemon registration");
    };
    assert_eq!(config_view.version, 2);
    assert_eq!(
        config_view
            .cluster
            .nodes
            .iter()
            .map(|node| node.id)
            .collect::<Vec<_>>(),
        vec![2, 8]
    );
    assert_eq!(runtime.catalog.layout().unwrap().nodes, vec![2, 8]);
}

#[tokio::test]
async fn coordinator_control_service_returns_malformed_for_method_request_mismatch() {
    let runtime = Arc::new(bootstrap_runtime());
    let service = CoordinatorControlService::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let address = service.local_addr().unwrap();

    let server = tokio::spawn(async move {
        service
            .serve_once_with(move |method, request| {
                let runtime = runtime.clone();
                async move {
                    handle_coordinator_control_method(runtime.as_ref(), &method, request).await
                }
            })
            .await
            .unwrap()
    });

    let response = request_coordinator_control_once(
        address,
        "space_rm",
        &CoordinatorAdminRequest::SpaceAdd(
            parse_hyperdex_space(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n",
            )
            .unwrap(),
        ),
    )
    .await
    .unwrap();

    server.await.unwrap();
    assert_eq!(
        CoordinatorReturnCode::decode(&response).unwrap(),
        CoordinatorReturnCode::Malformed
    );
}

#[tokio::test]
async fn coordinator_control_service_wait_until_stable_returns_version_body() {
    let runtime = Arc::new(bootstrap_runtime());
    HyperdexAdminService::handle(
        runtime.as_ref(),
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
    let service = CoordinatorControlService::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let address = service.local_addr().unwrap();

    let server = tokio::spawn(async move {
        service
            .serve_once_with(move |method, request| {
                let runtime = runtime.clone();
                async move {
                    handle_coordinator_control_method(runtime.as_ref(), &method, request).await
                }
            })
            .await
            .unwrap()
    });

    let response = request_coordinator_control_with_body_once(
        address,
        "wait_until_stable",
        &CoordinatorAdminRequest::WaitUntilStable,
    )
    .await
    .unwrap();

    server.await.unwrap();
    assert_eq!(
        CoordinatorReturnCode::decode(&response.status).unwrap(),
        CoordinatorReturnCode::Success
    );
    let version: u64 = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(version, 1);
}

#[tokio::test]
async fn coordinator_control_service_config_get_returns_config_snapshot() {
    let runtime = Arc::new(bootstrap_runtime());
    HyperdexAdminService::handle(
        runtime.as_ref(),
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
    let service = CoordinatorControlService::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let address = service.local_addr().unwrap();

    let server = tokio::spawn(async move {
        service
            .serve_once_with(move |method, request| {
                let runtime = runtime.clone();
                async move {
                    handle_coordinator_control_method(runtime.as_ref(), &method, request).await
                }
            })
            .await
            .unwrap()
    });

    let response = request_coordinator_control_with_body_once(
        address,
        "config_get",
        &CoordinatorAdminRequest::ConfigGet,
    )
    .await
    .unwrap();

    server.await.unwrap();
    assert_eq!(
        CoordinatorReturnCode::decode(&response.status).unwrap(),
        CoordinatorReturnCode::Success
    );
    let config_view: ConfigView = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(config_view.version, 1);
    assert_eq!(config_view.stable_through, 1);
    assert_eq!(config_view.spaces.len(), 1);
    assert_eq!(config_view.spaces[0].name, "profiles");
}

#[tokio::test]
async fn coordinator_control_service_ignores_early_eof_and_continues() {
    let runtime = Arc::new(bootstrap_runtime());
    let service = CoordinatorControlService::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let address = service.local_addr().unwrap();

    let server = tokio::spawn(async move {
        service
            .serve_forever_with(move |method, request| {
                let runtime = runtime.clone();
                async move {
                    handle_coordinator_control_method(runtime.as_ref(), &method, request).await
                }
            })
            .await
    });

    let stream = tokio::net::TcpStream::connect(address).await.unwrap();
    drop(stream);

    let response = request_coordinator_control_once(
        address,
        "space_add",
        &CoordinatorAdminRequest::SpaceAdd(
            parse_hyperdex_space(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n",
            )
            .unwrap(),
        ),
    )
    .await
    .unwrap();

    server.abort();
    let _ = server.await;
    assert_eq!(
        CoordinatorReturnCode::decode(&response).unwrap(),
        CoordinatorReturnCode::Success
    );
}

#[tokio::test]
async fn coordinator_public_port_accepts_control_while_legacy_follow_is_open() {
    let runtime = Arc::new(bootstrap_runtime());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let mut tasks = Vec::new();
        for _ in 0..2 {
            let (stream, _) = listener.accept().await.unwrap();
            let runtime = runtime.clone();
            tasks.push(tokio::spawn(async move {
                serve_coordinator_public_connection(stream, runtime)
                    .await
                    .unwrap();
            }));
        }

        for task in tasks {
            task.await.unwrap();
        }
    });

    let mut legacy_stream = tokio::net::TcpStream::connect(address).await.unwrap();
    legacy_stream
        .write_all(&BusyBeeFrame::identify(vec![0_u8; 16]).encode().unwrap())
        .await
        .unwrap();
    legacy_stream.flush().await.unwrap();
    let identify = read_admin_response_frame(&mut legacy_stream).await;
    assert_eq!(
        identify.flags & hyperdex_admin_protocol::BUSYBEE_HEADER_IDENTIFY,
        hyperdex_admin_protocol::BUSYBEE_HEADER_IDENTIFY
    );
    let bootstrap_request = BusyBeeFrame::new(vec![ReplicantNetworkMsgtype::Bootstrap.encode()])
        .encode()
        .unwrap();
    legacy_stream.write_all(&bootstrap_request).await.unwrap();
    legacy_stream.flush().await.unwrap();

    let initial = read_admin_response_frame(&mut legacy_stream).await;
    let bootstrap = ReplicantBootstrapResponse::decode(&initial.payload).unwrap();
    assert_eq!(bootstrap.server.id, LEGACY_COORDINATOR_SERVER_ID);
    assert_eq!(bootstrap.configuration.version, 1);

    let response = request_coordinator_control_once(
        address,
        "space_add",
        &CoordinatorAdminRequest::SpaceAdd(
            parse_hyperdex_space(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n",
            )
            .unwrap(),
        ),
    )
    .await
    .unwrap();

    assert_eq!(
        CoordinatorReturnCode::decode(&response).unwrap(),
        CoordinatorReturnCode::Success
    );

    drop(legacy_stream);
    server.await.unwrap();
}

#[tokio::test]
async fn coordinator_admin_legacy_service_bootstrap_sends_bootstrap_reply() {
    let runtime = Arc::new(bootstrap_runtime());
    let service = CoordinatorAdminLegacyService::bind_with_codecs(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(dsl_space_add_decoder),
        Arc::new(default_legacy_config_encoder),
    )
    .await
    .unwrap();
    let address = service.local_addr().unwrap();

    let server = tokio::spawn(async move { service.serve_once(runtime.as_ref()).await.unwrap() });

    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut identify_request = Vec::new();
    encode_u64_be(&mut identify_request, 7);
    encode_u64_be(&mut identify_request, 19);
    stream
        .write_all(&BusyBeeFrame::identify(identify_request).encode().unwrap())
        .await
        .unwrap();
    stream.flush().await.unwrap();
    let identify = read_admin_response_frame(&mut stream).await;
    assert_eq!(
        identify.flags & hyperdex_admin_protocol::BUSYBEE_HEADER_IDENTIFY,
        hyperdex_admin_protocol::BUSYBEE_HEADER_IDENTIFY
    );
    let mut identify_cursor = 0;
    assert_eq!(decode_u64(&identify.payload, &mut identify_cursor), 19);
    assert_eq!(decode_u64(&identify.payload, &mut identify_cursor), 7);
    assert_eq!(identify_cursor, identify.payload.len());
    let bootstrap_request = BusyBeeFrame::new(vec![ReplicantNetworkMsgtype::Bootstrap.encode()])
        .encode()
        .unwrap();
    stream.write_all(&bootstrap_request).await.unwrap();
    stream.flush().await.unwrap();

    let frame = read_admin_response_frame(&mut stream).await;
    let bootstrap = ReplicantBootstrapResponse::decode(&frame.payload).unwrap();
    assert_eq!(bootstrap.server.id, 19);
    assert_eq!(bootstrap.server.address, address);
    assert_eq!(bootstrap.configuration.cluster_id, 1);
    assert_eq!(bootstrap.configuration.version, 1);
    assert_eq!(bootstrap.configuration.first_slot, 1);
    assert_eq!(
        bootstrap.configuration.servers,
        vec![ReplicantBootstrapServer { id: 19, address }]
    );

    drop(stream);
    server.await.unwrap();
}

#[tokio::test]
async fn coordinator_admin_legacy_service_repeated_identify_is_validate_only() {
    let runtime = Arc::new(bootstrap_runtime());
    let service = CoordinatorAdminLegacyService::bind_with_codecs(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(dsl_space_add_decoder),
        Arc::new(default_legacy_config_encoder),
    )
    .await
    .unwrap();
    let address = service.local_addr().unwrap();

    let server = tokio::spawn(async move { service.serve_once(runtime.as_ref()).await.unwrap() });

    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut identify_request = Vec::new();
    encode_u64_be(&mut identify_request, 7);
    encode_u64_be(&mut identify_request, 19);
    stream
        .write_all(&BusyBeeFrame::identify(identify_request).encode().unwrap())
        .await
        .unwrap();
    stream.flush().await.unwrap();
    let identify = read_admin_response_frame(&mut stream).await;
    assert_eq!(
        identify.flags & hyperdex_admin_protocol::BUSYBEE_HEADER_IDENTIFY,
        hyperdex_admin_protocol::BUSYBEE_HEADER_IDENTIFY
    );

    let mut repeated_identify = Vec::new();
    encode_u64_be(&mut repeated_identify, 0);
    encode_u64_be(&mut repeated_identify, 19);
    stream
        .write_all(&BusyBeeFrame::identify(repeated_identify).encode().unwrap())
        .await
        .unwrap();
    stream.flush().await.unwrap();

    let repeated = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        read_busybee_frame_from_stream(&mut stream),
    )
    .await;
    assert!(
        repeated.is_err(),
        "repeated identify should not trigger another identify reply"
    );

    let bootstrap_request = BusyBeeFrame::new(vec![ReplicantNetworkMsgtype::Bootstrap.encode()])
        .encode()
        .unwrap();
    stream.write_all(&bootstrap_request).await.unwrap();
    stream.flush().await.unwrap();

    let bootstrap = read_admin_response_frame(&mut stream).await;
    let bootstrap = ReplicantBootstrapResponse::decode(&bootstrap.payload).unwrap();
    assert_eq!(bootstrap.server.id, 19);

    drop(stream);
    server.await.unwrap();
}

#[tokio::test]
async fn coordinator_admin_legacy_service_space_add_triggers_follow_update() {
    let runtime = Arc::new(bootstrap_runtime());
    let service = CoordinatorAdminLegacyService::bind_with_codecs(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(dsl_space_add_decoder),
        Arc::new(default_legacy_config_encoder),
    )
    .await
    .unwrap();
    let address = service.local_addr().unwrap();

    let server = tokio::spawn(async move { service.serve_once(runtime.as_ref()).await.unwrap() });

    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream
        .write_all(&BusyBeeFrame::identify(vec![0_u8; 16]).encode().unwrap())
        .await
        .unwrap();
    stream.flush().await.unwrap();
    let identify = read_admin_response_frame(&mut stream).await;
    assert_eq!(
        identify.flags & hyperdex_admin_protocol::BUSYBEE_HEADER_IDENTIFY,
        hyperdex_admin_protocol::BUSYBEE_HEADER_IDENTIFY
    );
    let bootstrap_request = BusyBeeFrame::new(vec![ReplicantNetworkMsgtype::Bootstrap.encode()])
        .encode()
        .unwrap();
    stream.write_all(&bootstrap_request).await.unwrap();
    stream.flush().await.unwrap();
    let bootstrap = read_admin_response_frame(&mut stream).await;
    let bootstrap = ReplicantBootstrapResponse::decode(&bootstrap.payload).unwrap();
    assert_eq!(bootstrap.configuration.version, 1);

    let follow_request = BusyBeeFrame::new(
        ReplicantAdminRequestMessage::CondWait {
            nonce: 7,
            object: b"hyperdex".to_vec(),
            condition: b"config".to_vec(),
            state: 1,
        }
        .encode()
        .unwrap(),
    )
    .encode()
    .unwrap();
    stream.write_all(&follow_request).await.unwrap();
    stream.flush().await.unwrap();

    let initial = read_admin_response_frame(&mut stream).await;
    let initial_follow = ReplicantConditionCompletion::decode(&initial.payload).unwrap();
    assert_eq!(initial_follow.nonce, 7);
    assert_eq!(initial_follow.state, 1);

    let robust_params_request = BusyBeeFrame::new(
        ReplicantAdminRequestMessage::get_robust_params(10)
            .encode()
            .unwrap(),
    )
    .encode()
    .unwrap();
    stream.write_all(&robust_params_request).await.unwrap();
    stream.flush().await.unwrap();

    let robust_params_frame = read_admin_response_frame(&mut stream).await;
    let robust_params = ReplicantRobustParams::decode(&robust_params_frame.payload).unwrap();
    assert_eq!(robust_params.nonce, 10);
    assert!(robust_params.command_nonce > 0);

    let request = BusyBeeFrame::new(
        ReplicantAdminRequestMessage::CallRobust {
            nonce: 11,
            command_nonce: robust_params.command_nonce,
            min_slot: robust_params.min_slot,
            object: b"hyperdex".to_vec(),
            function: b"space_add".to_vec(),
            input: b"space profiles\n\
                    key username\n\
                    attributes\n\
                       string first,\n\
                       int profile_views\n\
                    tolerate 0 failures\n"
                .to_vec(),
        }
        .encode()
        .unwrap(),
    )
    .encode()
    .unwrap();
    stream.write_all(&request).await.unwrap();
    let pending_follow = BusyBeeFrame::new(
        ReplicantAdminRequestMessage::CondWait {
            nonce: 8,
            object: b"hyperdex".to_vec(),
            condition: b"config".to_vec(),
            state: 2,
        }
        .encode()
        .unwrap(),
    )
    .encode()
    .unwrap();
    stream.write_all(&pending_follow).await.unwrap();
    stream.flush().await.unwrap();

    let call_frame = read_admin_response_frame(&mut stream).await;
    let call_completion = ReplicantCallCompletion::decode(&call_frame.payload).unwrap();
    assert_eq!(call_completion.nonce, 11);
    assert_eq!(call_completion.status, ReplicantReturnCode::Success);
    assert_eq!(
        CoordinatorReturnCode::decode(&call_completion.output).unwrap(),
        CoordinatorReturnCode::Success
    );

    let follow_frame = read_admin_response_frame(&mut stream).await;
    let follow_completion = ReplicantConditionCompletion::decode(&follow_frame.payload).unwrap();
    let config = decode_legacy_config(&follow_completion.data);
    assert_eq!(follow_completion.nonce, 8);
    assert_eq!(follow_completion.state, 2);
    assert_eq!(config.version, 2);
    assert_eq!(config.server_ids, vec![1]);
    assert_eq!(
        config.spaces,
        vec![DecodedLegacySpace {
            name: "profiles".to_owned(),
            attributes: vec![
                ("username".to_owned(), LEGACY_HYPERDATATYPE_STRING),
                ("first".to_owned(), LEGACY_HYPERDATATYPE_STRING),
                ("profile_views".to_owned(), LEGACY_HYPERDATATYPE_INT64),
            ],
        }]
    );

    drop(stream);
    server.await.unwrap();
}

#[test]
fn legacy_config_encoder_preserves_profiles_attribute_names_and_types() {
    let view = ConfigView {
        version: 1,
        stable_through: 1,
        cluster: ClusterConfig::default(),
        spaces: vec![parse_hyperdex_space(
            "space profiles\n\
             key username\n\
             attributes\n\
                string first,\n\
                string last,\n\
                float score,\n\
                int profile_views,\n\
                list(string) pending_requests,\n\
                list(float) rankings,\n\
                list(int) todolist,\n\
                set(string) hobbies,\n\
                set(float) imonafloat,\n\
                set(int) friendids,\n\
                map(string, int) unread_messages,\n\
                map(string, float) upvotes,\n\
                map(string, string) friendranks,\n\
                map(int, int) posts,\n\
                map(int, string) friendremapping,\n\
                map(int, float) intfloatmap,\n\
                map(float, int) still_looking,\n\
                map(float, string) for_a_reason,\n\
                map(float, float) for_float_keyed_map\n\
             tolerate 0 failures\n",
        )
        .unwrap()],
    };

    let encoded = default_legacy_config_encoder(&view).unwrap();
    let decoded = decode_legacy_config(&encoded);

    assert_eq!(decoded.version, 2);
    assert_eq!(
        decoded.spaces,
        vec![DecodedLegacySpace {
            name: "profiles".to_owned(),
            attributes: vec![
                ("username".to_owned(), LEGACY_HYPERDATATYPE_STRING),
                ("first".to_owned(), LEGACY_HYPERDATATYPE_STRING),
                ("last".to_owned(), LEGACY_HYPERDATATYPE_STRING),
                ("score".to_owned(), LEGACY_HYPERDATATYPE_FLOAT),
                ("profile_views".to_owned(), LEGACY_HYPERDATATYPE_INT64),
                (
                    "pending_requests".to_owned(),
                    LEGACY_HYPERDATATYPE_LIST_GENERIC | 0x0001,
                ),
                (
                    "rankings".to_owned(),
                    LEGACY_HYPERDATATYPE_LIST_GENERIC | 0x0003
                ),
                (
                    "todolist".to_owned(),
                    LEGACY_HYPERDATATYPE_LIST_GENERIC | 0x0002
                ),
                (
                    "hobbies".to_owned(),
                    LEGACY_HYPERDATATYPE_SET_GENERIC | 0x0001
                ),
                (
                    "imonafloat".to_owned(),
                    LEGACY_HYPERDATATYPE_SET_GENERIC | 0x0003
                ),
                (
                    "friendids".to_owned(),
                    LEGACY_HYPERDATATYPE_SET_GENERIC | 0x0002
                ),
                (
                    "unread_messages".to_owned(),
                    LEGACY_HYPERDATATYPE_MAP_GENERIC | (0x0001 << 3) | 0x0002,
                ),
                (
                    "upvotes".to_owned(),
                    LEGACY_HYPERDATATYPE_MAP_GENERIC | (0x0001 << 3) | 0x0003,
                ),
                (
                    "friendranks".to_owned(),
                    LEGACY_HYPERDATATYPE_MAP_GENERIC | (0x0001 << 3) | 0x0001,
                ),
                (
                    "posts".to_owned(),
                    LEGACY_HYPERDATATYPE_MAP_GENERIC | (0x0002 << 3) | 0x0002,
                ),
                (
                    "friendremapping".to_owned(),
                    LEGACY_HYPERDATATYPE_MAP_GENERIC | (0x0002 << 3) | 0x0001,
                ),
                (
                    "intfloatmap".to_owned(),
                    LEGACY_HYPERDATATYPE_MAP_GENERIC | (0x0002 << 3) | 0x0003,
                ),
                (
                    "still_looking".to_owned(),
                    LEGACY_HYPERDATATYPE_MAP_GENERIC | (0x0003 << 3) | 0x0002,
                ),
                (
                    "for_a_reason".to_owned(),
                    LEGACY_HYPERDATATYPE_MAP_GENERIC | (0x0003 << 3) | 0x0001,
                ),
                (
                    "for_float_keyed_map".to_owned(),
                    LEGACY_HYPERDATATYPE_MAP_GENERIC | (0x0003 << 3) | 0x0003,
                ),
            ],
        }]
    );
}

#[test]
fn legacy_partition_regions_cover_full_u64_space_for_single_dimension() {
    let regions = legacy_partition_regions(1, 64);
    let interval = (0x8000_0000_0000_0000_u64 / 64) * 2;

    assert_eq!(regions.len(), 64);
    assert_eq!(regions.first().unwrap(), &(vec![0], vec![interval - 1]));
    assert_eq!(
        regions.last().unwrap(),
        &(vec![interval * 63], vec![u64::MAX])
    );

    for window in regions.windows(2) {
        let (_, lhs_upper) = &window[0];
        let (rhs_lower, _) = &window[1];
        assert_eq!(lhs_upper[0].saturating_add(1), rhs_lower[0]);
    }
}

#[test]
fn legacy_config_uses_shared_nonzero_ids() {
    let view = ConfigView {
        version: 1,
        stable_through: 1,
        cluster: ClusterConfig::default(),
        spaces: vec![parse_hyperdex_space(
            "space profiles\n\
             key username\n\
             attributes\n\
                string first,\n\
                int profile_views\n\
             tolerate 0 failures\n",
        )
        .unwrap()],
    };

    let encoded = default_legacy_config_encoder(&view).unwrap();
    let mut cursor = 0;
    let _cluster = decode_u64(&encoded, &mut cursor);
    let _version = decode_u64(&encoded, &mut cursor);
    let _flags = decode_u64(&encoded, &mut cursor);
    let servers_len = decode_u64(&encoded, &mut cursor) as usize;
    let spaces_len = decode_u64(&encoded, &mut cursor) as usize;
    let transfers_len = decode_u64(&encoded, &mut cursor) as usize;

    assert_eq!(spaces_len, 1);
    assert_eq!(transfers_len, 0);

    for _ in 0..servers_len {
        let _state = decode_u8(&encoded, &mut cursor);
        let _server_id = decode_u64(&encoded, &mut cursor);
        skip_legacy_location(&encoded, &mut cursor);
    }

    let space_id = decode_u64(&encoded, &mut cursor);
    let _space_name = decode_varint_string(&encoded, &mut cursor);
    let _fault_tolerance = decode_u64(&encoded, &mut cursor);
    let attrs_len = decode_u16(&encoded, &mut cursor) as usize;
    let subspaces_len = decode_u16(&encoded, &mut cursor) as usize;
    let _indices_len = decode_u16(&encoded, &mut cursor) as usize;

    for _ in 0..attrs_len {
        let _attr_name = decode_varint_string(&encoded, &mut cursor);
        let _datatype = decode_u16(&encoded, &mut cursor);
    }

    let subspace_id = decode_u64(&encoded, &mut cursor);
    let attrs_in_subspace = decode_u16(&encoded, &mut cursor) as usize;
    let regions_len = decode_u32(&encoded, &mut cursor) as usize;

    for _ in 0..attrs_in_subspace {
        let _attr = decode_u16(&encoded, &mut cursor);
    }

    let region_id = decode_u64(&encoded, &mut cursor);
    let bounds_len = decode_u16(&encoded, &mut cursor) as usize;
    let replicas_len = decode_u8(&encoded, &mut cursor) as usize;

    for _ in 0..bounds_len {
        let _lower = decode_u64(&encoded, &mut cursor);
        let _upper = decode_u64(&encoded, &mut cursor);
    }

    let first_replica_server_id = decode_u64(&encoded, &mut cursor);
    let first_virtual_server_id = decode_u64(&encoded, &mut cursor);

    assert_eq!(subspaces_len, 1);
    assert_eq!(regions_len, 64);
    assert_eq!(replicas_len, 1);
    assert_eq!(first_replica_server_id, 1);
    assert_eq!(space_id, 1);
    assert_eq!(subspace_id, 2);
    assert_eq!(region_id, 3);
    assert_eq!(first_virtual_server_id, 67);
}

#[test]
fn legacy_config_distributes_key_regions_across_two_servers() {
    let view = ConfigView {
        version: 1,
        stable_through: 1,
        cluster: ClusterConfig {
            nodes: vec![
                ClusterNode {
                    id: 1,
                    host: "127.0.0.1".to_owned(),
                    control_port: 1982,
                    data_port: 2012,
                },
                ClusterNode {
                    id: 2,
                    host: "127.0.0.1".to_owned(),
                    control_port: 1983,
                    data_port: 2013,
                },
            ],
            replicas: 1,
            ..ClusterConfig::default()
        },
        spaces: vec![parse_hyperdex_space(
            "space profiles\n\
             key username\n\
             attributes\n\
                string first,\n\
                int profile_views\n\
             tolerate 0 failures\n",
        )
        .unwrap()],
    };

    let encoded = default_legacy_config_encoder(&view).unwrap();
    let mut cursor = 0;
    let _cluster = decode_u64(&encoded, &mut cursor);
    let _version = decode_u64(&encoded, &mut cursor);
    let _flags = decode_u64(&encoded, &mut cursor);
    let servers_len = decode_u64(&encoded, &mut cursor) as usize;
    let spaces_len = decode_u64(&encoded, &mut cursor) as usize;
    let _transfers_len = decode_u64(&encoded, &mut cursor) as usize;

    assert_eq!(servers_len, 2);
    assert_eq!(spaces_len, 1);

    for _ in 0..servers_len {
        let _state = decode_u8(&encoded, &mut cursor);
        let _server_id = decode_u64(&encoded, &mut cursor);
        skip_legacy_location(&encoded, &mut cursor);
    }

    let _space_id = decode_u64(&encoded, &mut cursor);
    let _space_name = decode_varint_string(&encoded, &mut cursor);
    let _fault_tolerance = decode_u64(&encoded, &mut cursor);
    let attrs_len = decode_u16(&encoded, &mut cursor) as usize;
    let subspaces_len = decode_u16(&encoded, &mut cursor) as usize;
    let _indices_len = decode_u16(&encoded, &mut cursor) as usize;

    assert_eq!(subspaces_len, 1);

    for _ in 0..attrs_len {
        let _attr_name = decode_varint_string(&encoded, &mut cursor);
        let _datatype = decode_u16(&encoded, &mut cursor);
    }

    let _subspace_id = decode_u64(&encoded, &mut cursor);
    let attrs_in_subspace = decode_u16(&encoded, &mut cursor) as usize;
    let regions_len = decode_u32(&encoded, &mut cursor) as usize;

    for _ in 0..attrs_in_subspace {
        let _attr = decode_u16(&encoded, &mut cursor);
    }

    let mut first_replica_server_ids = Vec::with_capacity(regions_len);

    for _ in 0..regions_len {
        let _region_id = decode_u64(&encoded, &mut cursor);
        let bounds_len = decode_u16(&encoded, &mut cursor) as usize;
        let replicas_len = decode_u8(&encoded, &mut cursor) as usize;

        for _ in 0..bounds_len {
            let _lower = decode_u64(&encoded, &mut cursor);
            let _upper = decode_u64(&encoded, &mut cursor);
        }

        let first_replica_server_id = decode_u64(&encoded, &mut cursor);
        let _first_virtual_server_id = decode_u64(&encoded, &mut cursor);
        first_replica_server_ids.push(first_replica_server_id);

        for _ in 1..replicas_len {
            let _server_id = decode_u64(&encoded, &mut cursor);
            let _virtual_server_id = decode_u64(&encoded, &mut cursor);
        }
    }

    assert_eq!(regions_len, 64);
    assert_eq!(first_replica_server_ids.len(), 64);
    assert!(first_replica_server_ids[..32]
        .iter()
        .all(|server_id| *server_id == 1));
    assert!(first_replica_server_ids[32..]
        .iter()
        .all(|server_id| *server_id == 2));
}

#[test]
fn legacy_string_key_hash_matches_hyperdex_cityhash64() {
    assert_eq!(cityhash64::<u64>(b"a"), 12_917_804_110_809_363_939);
    assert_eq!(cityhash64::<u64>(b"K"), 17_790_691_183_158_543_131);
    assert_eq!(cityhash64::<u64>(b"K\x0b"), 17_582_107_272_978_808_922);
    assert_eq!(cityhash64::<u64>(b"K\x0b\\"), 13_859_931_219_248_667_929);
    assert_eq!(
        cityhash64::<u64>(b"H\x1fUS-K\x0bv\\"),
        14_723_391_480_779_626_874
    );
}

#[test]
fn legacy_string_point_routing_matches_encoded_two_node_regions() {
    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: 1982,
                data_port: 2012,
            },
            ClusterNode {
                id: 2,
                host: "127.0.0.1".to_owned(),
                control_port: 1983,
                data_port: 2013,
            },
        ],
        replicas: 1,
        ..ClusterConfig::default()
    };
    let runtime = ClusterRuntime::single_node(config).unwrap();
    let space = parse_hyperdex_space(
        "space profiles\n\
         key username\n\
         attributes\n\
            string first,\n\
            int profile_views\n\
         tolerate 0 failures\n",
    )
    .unwrap();
    runtime.catalog.create_space(space).unwrap();

    for key in [
        b"a".as_slice(),
        b"K",
        b"K\x0b",
        b"K\x0b\\",
        b"H\x1fUS-K\x0bv\\",
    ] {
        assert_eq!(runtime.route_primary_for_space("profiles", key).unwrap(), 2);
    }
}

#[test]
fn legacy_value_from_protocol_accepts_empty_string_map() {
    let value = legacy_value_from_protocol(HYPERDATATYPE_MAP_GENERIC | (1_u16 << 3) | 1, &[])
        .expect("empty map(string,string) should decode");
    assert_eq!(value, Value::Map(BTreeMap::new()));
}

#[tokio::test]
async fn coordinator_admin_legacy_service_wait_until_stable_completes_after_space_add() {
    let runtime = Arc::new(bootstrap_runtime());
    let service = CoordinatorAdminLegacyService::bind_with_codecs(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(dsl_space_add_decoder),
        Arc::new(default_legacy_config_encoder),
    )
    .await
    .unwrap();
    let address = service.local_addr().unwrap();

    let server = tokio::spawn(async move { service.serve_once(runtime.as_ref()).await.unwrap() });

    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let wait_request = BusyBeeFrame::new(
        ReplicantAdminRequestMessage::wait_until_stable(19, 2)
            .encode()
            .unwrap(),
    )
    .encode()
    .unwrap();
    stream.write_all(&wait_request).await.unwrap();
    stream.flush().await.unwrap();

    let pending = tokio::time::timeout(
        Duration::from_millis(50),
        read_busybee_frame_from_stream(&mut stream),
    )
    .await;
    assert!(pending.is_err(), "wait_until_stable should remain pending");

    let space_add = BusyBeeFrame::new(
        ReplicantAdminRequestMessage::space_add(
            20,
            b"space profiles\n\
              key username\n\
              attributes\n\
                 string first,\n\
                 int profile_views\n\
              tolerate 0 failures\n"
                .to_vec(),
        )
        .encode()
        .unwrap(),
    )
    .encode()
    .unwrap();
    stream.write_all(&space_add).await.unwrap();
    stream.flush().await.unwrap();

    let call_frame = read_admin_response_frame(&mut stream).await;
    let call_completion = ReplicantCallCompletion::decode(&call_frame.payload).unwrap();
    assert_eq!(call_completion.nonce, 20);
    assert_eq!(
        CoordinatorReturnCode::decode(&call_completion.output).unwrap(),
        CoordinatorReturnCode::Success
    );

    let wait_frame = read_admin_response_frame(&mut stream).await;
    let wait_completion = ReplicantConditionCompletion::decode(&wait_frame.payload).unwrap();
    assert_eq!(wait_completion.nonce, 19);
    assert_eq!(wait_completion.status, ReplicantReturnCode::Success);
    assert_eq!(wait_completion.state, 2);
    assert!(wait_completion.data.is_empty());

    drop(stream);
    server.await.unwrap();
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
async fn client_put_materializes_key_attribute_for_search_and_count() {
    let runtime = bootstrap_runtime();
    HyperdexAdminService::handle(
        &runtime,
        AdminRequest::CreateSpaceDsl(
            "space profiles\n\
             key username\n\
             attributes\n\
                string first,\n\
                map(float, string) still_looking\n\
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
                name: "still_looking".to_owned(),
                value: Value::Map(BTreeMap::new()),
            })],
        },
    )
    .await
    .unwrap();

    let search = HyperdexClientService::handle(
        &runtime,
        ClientRequest::Search {
            space: "profiles".to_owned(),
            checks: vec![Check {
                attribute: "username".to_owned(),
                predicate: Predicate::Equal,
                value: Value::Bytes(Bytes::from_static(b"ada")),
            }],
        },
    )
    .await
    .unwrap();
    assert!(matches!(search, ClientResponse::SearchResult(records) if records.len() == 1));

    let count = HyperdexClientService::handle(
        &runtime,
        ClientRequest::Count {
            space: "profiles".to_owned(),
            checks: vec![Check {
                attribute: "username".to_owned(),
                predicate: Predicate::Equal,
                value: Value::Bytes(Bytes::from_static(b"ada")),
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(count, ClientResponse::Count(1));
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
        &legacy_request_body(19, encode_protocol_get_request(b"ada")),
    )
    .await
    .unwrap();

    assert_eq!(header.message_type, LegacyMessageType::RespGet);
    let response = decode_protocol_get_response(&body).unwrap();
    assert_eq!(response.status, LegacyReturnCode::Success as u16);
    assert_eq!(
        response.values,
        vec![b"Ada".to_vec(), 5_i64.to_le_bytes().to_vec()]
    );
}

#[tokio::test]
async fn legacy_get_fills_defaults_for_sparse_record_attributes() {
    let runtime = bootstrap_runtime();
    HyperdexAdminService::handle(
        &runtime,
        AdminRequest::CreateSpaceDsl(
            "space profiles\n\
             key username\n\
             attributes\n\
                string first,\n\
                int profile_views,\n\
                list(string) pending_requests,\n\
                set(int) friendids,\n\
                map(string, int) upvotes\n\
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

    let (header, body) = handle_legacy_request(
        &runtime,
        RequestHeader {
            message_type: LegacyMessageType::ReqGet,
            flags: 0,
            version: 1,
            target_virtual_server: 11,
            nonce: 19,
        },
        &legacy_request_body(19, encode_protocol_get_request(b"ada")),
    )
    .await
    .unwrap();

    assert_eq!(header.message_type, LegacyMessageType::RespGet);
    let response = decode_protocol_get_response(&body).unwrap();
    assert_eq!(response.status, LegacyReturnCode::Success as u16);
    assert_eq!(
        response.values,
        vec![
            b"Ada".to_vec(),
            0_i64.to_le_bytes().to_vec(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ]
    );
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
        &legacy_request_body(
            19,
            encode_protocol_atomic_request(&ProtocolKeyChange {
                key: b"ada".to_vec(),
                erase: false,
                fail_if_not_found: false,
                fail_if_found: false,
                checks: Vec::new(),
                funcalls: vec![
                    ProtocolFuncall {
                        attr: 1,
                        name: FUNC_SET,
                        arg1: b"Ada".to_vec(),
                        arg1_datatype: HYPERDATATYPE_STRING,
                        arg2: Vec::new(),
                        arg2_datatype: 0,
                    },
                    ProtocolFuncall {
                        attr: 2,
                        name: FUNC_SET,
                        arg1: 5_i64.to_le_bytes().to_vec(),
                        arg1_datatype: HYPERDATATYPE_INT64,
                        arg2: Vec::new(),
                        arg2_datatype: 0,
                    },
                ],
            }),
        ),
    )
    .await
    .unwrap();

    assert_eq!(header.message_type, LegacyMessageType::RespAtomic);
    assert_eq!(
        decode_protocol_atomic_response(&body).unwrap(),
        LegacyReturnCode::Success as u16
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
        Some(&Value::Bytes(Bytes::from_static(b"Ada")))
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
        &legacy_request_body(
            19,
            encode_protocol_atomic_request(&ProtocolKeyChange {
                key: b"ada".to_vec(),
                erase: false,
                fail_if_not_found: false,
                fail_if_found: true,
                checks: Vec::new(),
                funcalls: vec![ProtocolFuncall {
                    attr: 1,
                    name: FUNC_SET,
                    arg1: b"Grace".to_vec(),
                    arg1_datatype: HYPERDATATYPE_STRING,
                    arg2: Vec::new(),
                    arg2_datatype: 0,
                }],
            }),
        ),
    )
    .await
    .unwrap();

    assert_eq!(
        decode_protocol_atomic_response(&body).unwrap(),
        LegacyReturnCode::CompareFailed as u16
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
        &legacy_request_body(
            19,
            encode_protocol_atomic_request(&ProtocolKeyChange {
                key: b"ada".to_vec(),
                erase: false,
                fail_if_not_found: false,
                fail_if_found: false,
                checks: vec![ProtocolAttributeCheck {
                    attr: 2,
                    value: 5_i64.to_le_bytes().to_vec(),
                    datatype: HYPERDATATYPE_INT64,
                    predicate: HYPERPREDICATE_GREATER_EQUAL,
                }],
                funcalls: vec![ProtocolFuncall {
                    attr: 1,
                    name: FUNC_SET,
                    arg1: b"Grace".to_vec(),
                    arg1_datatype: HYPERDATATYPE_STRING,
                    arg2: Vec::new(),
                    arg2_datatype: 0,
                }],
            }),
        ),
    )
    .await
    .unwrap();

    assert_eq!(
        decode_protocol_atomic_response(&body).unwrap(),
        LegacyReturnCode::CompareFailed as u16
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
async fn legacy_atomic_returns_bad_dim_spec_for_schema_mismatched_set() {
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

    let (header, body) = handle_legacy_request(
        &runtime,
        RequestHeader {
            message_type: LegacyMessageType::ReqAtomic,
            flags: 0,
            version: 1,
            target_virtual_server: 11,
            nonce: 19,
        },
        &legacy_request_body(
            19,
            encode_protocol_atomic_request(&ProtocolKeyChange {
                key: b"ada".to_vec(),
                erase: false,
                fail_if_not_found: false,
                fail_if_found: false,
                checks: Vec::new(),
                funcalls: vec![ProtocolFuncall {
                    attr: 1,
                    name: FUNC_SET,
                    arg1: b"wrong".to_vec(),
                    arg1_datatype: HYPERDATATYPE_STRING,
                    arg2: Vec::new(),
                    arg2_datatype: 0,
                }],
            }),
        ),
    )
    .await
    .unwrap();

    assert_eq!(header.message_type, LegacyMessageType::RespAtomic);
    assert_eq!(
        decode_protocol_atomic_response(&body).unwrap(),
        LegacyReturnCode::BadDimensionSpec as u16
    );
}

#[tokio::test]
async fn legacy_atomic_returns_bad_dim_spec_for_erase_with_funcalls() {
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
            message_type: LegacyMessageType::ReqAtomic,
            flags: 0,
            version: 1,
            target_virtual_server: 11,
            nonce: 19,
        },
        &legacy_request_body(
            19,
            encode_protocol_atomic_request(&ProtocolKeyChange {
                key: b"ada".to_vec(),
                erase: true,
                fail_if_not_found: false,
                fail_if_found: false,
                checks: Vec::new(),
                funcalls: vec![ProtocolFuncall {
                    attr: 1,
                    name: FUNC_SET,
                    arg1: b"Ada".to_vec(),
                    arg1_datatype: HYPERDATATYPE_STRING,
                    arg2: Vec::new(),
                    arg2_datatype: 0,
                }],
            }),
        ),
    )
    .await
    .unwrap();

    assert_eq!(header.message_type, LegacyMessageType::RespAtomic);
    assert_eq!(
        decode_protocol_atomic_response(&body).unwrap(),
        LegacyReturnCode::BadDimensionSpec as u16
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
        &legacy_request_body(
            19,
            encode_protocol_atomic_request(&ProtocolKeyChange {
                key: b"ada".to_vec(),
                erase: false,
                fail_if_not_found: false,
                fail_if_found: false,
                checks: Vec::new(),
                funcalls: vec![ProtocolFuncall {
                    attr: 1,
                    name: FUNC_NUM_ADD,
                    arg1: 3_i64.to_le_bytes().to_vec(),
                    arg1_datatype: HYPERDATATYPE_INT64,
                    arg2: Vec::new(),
                    arg2_datatype: 0,
                }],
            }),
        ),
    )
    .await
    .unwrap();

    assert_eq!(
        decode_protocol_atomic_response(&body).unwrap(),
        LegacyReturnCode::Success as u16
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
async fn legacy_atomic_integer_div_and_mod_follow_hyperdex_signed_semantics() {
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
                value: Value::Int(7),
            })],
        },
    )
    .await
    .unwrap();

    let (_, div_body) = handle_legacy_request(
        &runtime,
        RequestHeader {
            message_type: LegacyMessageType::ReqAtomic,
            flags: 0,
            version: 1,
            target_virtual_server: 11,
            nonce: 19,
        },
        &legacy_request_body(
            19,
            encode_protocol_atomic_request(&ProtocolKeyChange {
                key: b"ada".to_vec(),
                erase: false,
                fail_if_not_found: false,
                fail_if_found: false,
                checks: Vec::new(),
                funcalls: vec![ProtocolFuncall {
                    attr: 1,
                    name: FUNC_NUM_DIV,
                    arg1: (-3_i64).to_le_bytes().to_vec(),
                    arg1_datatype: HYPERDATATYPE_INT64,
                    arg2: Vec::new(),
                    arg2_datatype: 0,
                }],
            }),
        ),
    )
    .await
    .unwrap();

    assert_eq!(
        decode_protocol_atomic_response(&div_body).unwrap(),
        LegacyReturnCode::Success as u16
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
        panic!("expected stored record after div");
    };
    assert_eq!(
        record.attributes.get("profile_views"),
        Some(&Value::Int(-3))
    );

    let (_, mod_body) = handle_legacy_request(
        &runtime,
        RequestHeader {
            message_type: LegacyMessageType::ReqAtomic,
            flags: 0,
            version: 1,
            target_virtual_server: 11,
            nonce: 20,
        },
        &legacy_request_body(
            20,
            encode_protocol_atomic_request(&ProtocolKeyChange {
                key: b"ada".to_vec(),
                erase: false,
                fail_if_not_found: false,
                fail_if_found: false,
                checks: Vec::new(),
                funcalls: vec![ProtocolFuncall {
                    attr: 1,
                    name: FUNC_NUM_MOD,
                    arg1: (2_i64).to_le_bytes().to_vec(),
                    arg1_datatype: HYPERDATATYPE_INT64,
                    arg2: Vec::new(),
                    arg2_datatype: 0,
                }],
            }),
        ),
    )
    .await
    .unwrap();

    assert_eq!(
        decode_protocol_atomic_response(&mod_body).unwrap(),
        LegacyReturnCode::Success as u16
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
        panic!("expected stored record after mod");
    };
    assert_eq!(record.attributes.get("profile_views"), Some(&Value::Int(1)));
}

#[tokio::test]
async fn legacy_atomic_map_string_prepend_updates_string_valued_map_entry() {
    let runtime = bootstrap_runtime();
    HyperdexAdminService::handle(
        &runtime,
        AdminRequest::CreateSpaceDsl(
            "space profiles\n\
             key username\n\
             attributes\n\
                map(string, string) unread_messages\n\
             tolerate 0 failures\n"
                .to_owned(),
        ),
    )
    .await
    .unwrap();

    let mut unread_messages = BTreeMap::new();
    unread_messages.insert(
        Value::Bytes(Bytes::from_static(b"KZ")),
        Value::Bytes(Bytes::from_static(b"+Y\\")),
    );

    HyperdexClientService::handle(
        &runtime,
        ClientRequest::Put {
            space: "profiles".to_owned(),
            key: Bytes::from_static(b"ada"),
            mutations: vec![Mutation::Set(Attribute {
                name: "unread_messages".to_owned(),
                value: Value::Map(unread_messages),
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
        &legacy_request_body(
            19,
            encode_protocol_atomic_request(&ProtocolKeyChange {
                key: b"ada".to_vec(),
                erase: false,
                fail_if_not_found: false,
                fail_if_found: false,
                checks: Vec::new(),
                funcalls: vec![ProtocolFuncall {
                    attr: 1,
                    name: FUNC_STRING_PREPEND,
                    arg1: b"'\\x02*".to_vec(),
                    arg1_datatype: HYPERDATATYPE_STRING,
                    arg2: b"KZ".to_vec(),
                    arg2_datatype: HYPERDATATYPE_STRING,
                }],
            }),
        ),
    )
    .await
    .unwrap();

    assert_eq!(
        decode_protocol_atomic_response(&body).unwrap(),
        LegacyReturnCode::Success as u16
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

    let Value::Map(values) = record.attributes.get("unread_messages").unwrap() else {
        panic!("expected unread_messages map");
    };
    assert_eq!(
        values.get(&Value::Bytes(Bytes::from_static(b"KZ"))),
        Some(&Value::Bytes(Bytes::from_static(b"'\\x02*+Y\\")))
    );
}

#[tokio::test]
async fn legacy_atomic_map_numeric_add_updates_int_int_entry() {
    let runtime = bootstrap_runtime();
    HyperdexAdminService::handle(
        &runtime,
        AdminRequest::CreateSpaceDsl(
            "space profiles\n\
             key username\n\
             attributes\n\
                map(int, int) login_counts\n\
             tolerate 0 failures\n"
                .to_owned(),
        ),
    )
    .await
    .unwrap();

    let mut login_counts = BTreeMap::new();
    login_counts.insert(Value::Int(7), Value::Int(2));
    HyperdexClientService::handle(
        &runtime,
        ClientRequest::Put {
            space: "profiles".to_owned(),
            key: Bytes::from_static(b"ada"),
            mutations: vec![Mutation::Set(Attribute {
                name: "login_counts".to_owned(),
                value: Value::Map(login_counts),
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
        &legacy_request_body(
            19,
            encode_protocol_atomic_request(&ProtocolKeyChange {
                key: b"ada".to_vec(),
                erase: false,
                fail_if_not_found: false,
                fail_if_found: false,
                checks: Vec::new(),
                funcalls: vec![ProtocolFuncall {
                    attr: 1,
                    name: FUNC_NUM_ADD,
                    arg1: 3_i64.to_le_bytes().to_vec(),
                    arg1_datatype: HYPERDATATYPE_INT64,
                    arg2: 7_i64.to_le_bytes().to_vec(),
                    arg2_datatype: HYPERDATATYPE_INT64,
                }],
            }),
        ),
    )
    .await
    .unwrap();

    assert_eq!(
        decode_protocol_atomic_response(&body).unwrap(),
        LegacyReturnCode::Success as u16
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

    let Value::Map(values) = record.attributes.get("login_counts").unwrap() else {
        panic!("expected login_counts map");
    };
    assert_eq!(values.get(&Value::Int(7)), Some(&Value::Int(5)));
}

#[tokio::test]
async fn legacy_atomic_map_numeric_add_updates_string_float_entry() {
    let runtime = bootstrap_runtime();
    HyperdexAdminService::handle(
        &runtime,
        AdminRequest::CreateSpaceDsl(
            "space profiles\n\
             key username\n\
             attributes\n\
                map(string, float) ratings\n\
             tolerate 0 failures\n"
                .to_owned(),
        ),
    )
    .await
    .unwrap();

    let mut ratings = BTreeMap::new();
    ratings.insert(
        Value::Bytes(Bytes::from_static(b"compiler")),
        Value::Float(1.5.into()),
    );
    HyperdexClientService::handle(
        &runtime,
        ClientRequest::Put {
            space: "profiles".to_owned(),
            key: Bytes::from_static(b"ada"),
            mutations: vec![Mutation::Set(Attribute {
                name: "ratings".to_owned(),
                value: Value::Map(ratings),
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
        &legacy_request_body(
            19,
            encode_protocol_atomic_request(&ProtocolKeyChange {
                key: b"ada".to_vec(),
                erase: false,
                fail_if_not_found: false,
                fail_if_found: false,
                checks: Vec::new(),
                funcalls: vec![ProtocolFuncall {
                    attr: 1,
                    name: FUNC_NUM_ADD,
                    arg1: 2.0_f64.to_le_bytes().to_vec(),
                    arg1_datatype: HYPERDATATYPE_FLOAT,
                    arg2: b"compiler".to_vec(),
                    arg2_datatype: HYPERDATATYPE_STRING,
                }],
            }),
        ),
    )
    .await
    .unwrap();

    assert_eq!(
        decode_protocol_atomic_response(&body).unwrap(),
        LegacyReturnCode::Success as u16
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

    let Value::Map(values) = record.attributes.get("ratings").unwrap() else {
        panic!("expected ratings map");
    };
    assert_eq!(
        values.get(&Value::Bytes(Bytes::from_static(b"compiler"))),
        Some(&Value::Float(3.5.into()))
    );
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
        &legacy_request_body(19, encode_protocol_count_request(&[])),
    )
    .await
    .unwrap();

    assert_eq!(header.message_type, LegacyMessageType::RespCount);
    assert_eq!(decode_protocol_count_response(&body).unwrap(), 0);
}

#[tokio::test]
async fn legacy_count_accepts_named_space_body() {
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
            nonce: 23,
        },
        &legacy_request_body(
            23,
            CountRequest {
                space: "profiles".to_owned(),
            }
            .encode_body(),
        ),
    )
    .await
    .unwrap();

    assert_eq!(header.message_type, LegacyMessageType::RespCount);
    assert_eq!(decode_protocol_count_response(&body).unwrap(), 0);
}

#[tokio::test]
async fn legacy_search_start_returns_first_matching_record() {
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

    for (key, first, views) in [("ada", "Ada", 5), ("grace", "Grace", 3), ("eve", "Eve", 1)] {
        HyperdexClientService::handle(
            &runtime,
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::copy_from_slice(key.as_bytes()),
                mutations: vec![
                    Mutation::Set(Attribute {
                        name: "first".to_owned(),
                        value: Value::String(first.to_owned()),
                    }),
                    Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(views),
                    }),
                ],
            },
        )
        .await
        .unwrap();
    }

    let (header, body) = handle_legacy_request(
        &runtime,
        RequestHeader {
            message_type: LegacyMessageType::ReqSearchStart,
            flags: 0,
            version: 1,
            target_virtual_server: 11,
            nonce: 19,
        },
        &legacy_request_body(
            19,
            encode_protocol_search_start(&ProtocolSearchStart {
                search_id: 41,
                checks: vec![ProtocolAttributeCheck {
                    attr: 2,
                    value: 3_i64.to_le_bytes().to_vec(),
                    datatype: HYPERDATATYPE_INT64,
                    predicate: HYPERPREDICATE_GREATER_EQUAL,
                }],
            }),
        ),
    )
    .await
    .unwrap();

    assert_eq!(header.message_type, LegacyMessageType::RespSearchItem);
    let item = decode_protocol_search_item(&body).unwrap();
    assert_eq!(item.key, b"ada".to_vec());
}

#[tokio::test]
async fn legacy_search_next_drains_cursor_then_returns_done() {
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

    for (key, first) in [("ada", "Ada"), ("grace", "Grace")] {
        HyperdexClientService::handle(
            &runtime,
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::copy_from_slice(key.as_bytes()),
                mutations: vec![Mutation::Set(Attribute {
                    name: "first".to_owned(),
                    value: Value::String(first.to_owned()),
                })],
            },
        )
        .await
        .unwrap();
    }

    let _ = handle_legacy_request(
        &runtime,
        RequestHeader {
            message_type: LegacyMessageType::ReqSearchStart,
            flags: 0,
            version: 1,
            target_virtual_server: 11,
            nonce: 19,
        },
        &legacy_request_body(
            19,
            encode_protocol_search_start(&ProtocolSearchStart {
                search_id: 99,
                checks: Vec::new(),
            }),
        ),
    )
    .await
    .unwrap();

    let (header, body) = handle_legacy_request(
        &runtime,
        RequestHeader {
            message_type: LegacyMessageType::ReqSearchNext,
            flags: 0,
            version: 1,
            target_virtual_server: 11,
            nonce: 20,
        },
        &legacy_request_body(20, encode_protocol_search_continue(99).to_vec()),
    )
    .await
    .unwrap();
    assert_eq!(header.message_type, LegacyMessageType::RespSearchItem);
    assert_eq!(
        decode_protocol_search_item(&body).unwrap().key,
        b"grace".to_vec()
    );

    let (header, body) = handle_legacy_request(
        &runtime,
        RequestHeader {
            message_type: LegacyMessageType::ReqSearchNext,
            flags: 0,
            version: 1,
            target_virtual_server: 11,
            nonce: 21,
        },
        &legacy_request_body(21, encode_protocol_search_continue(99).to_vec()),
    )
    .await
    .unwrap();
    assert_eq!(header.message_type, LegacyMessageType::RespSearchDone);
    assert_eq!(
        SearchDoneResponse::decode_body(&body).unwrap().search_id,
        99
    );
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
        "--node-id=7".to_owned(),
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
            node_id: 7,
            threads: 1,
            data_dir: "/tmp/daemon".to_owned(),
            listen_host: "127.0.0.1".to_owned(),
            listen_port: 2012,
            control_port: 2012,
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
        "--node-id=7".to_owned(),
        "--threads=1".to_owned(),
        "--data=/tmp/daemon".to_owned(),
        "--listen=127.0.0.1".to_owned(),
        "--listen-port=2012".to_owned(),
        "--control-port=3012".to_owned(),
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
            node_id: 7,
            threads: 1,
            data_dir: "/tmp/daemon".to_owned(),
            listen_host: "127.0.0.1".to_owned(),
            listen_port: 2012,
            control_port: 3012,
            coordinator_host: "127.0.0.1".to_owned(),
            coordinator_port: 1982,
            consensus: ConsensusBackend::Mirror,
            placement: PlacementBackend::Rendezvous,
            storage: StorageBackend::RocksDb,
            internode_transport: TransportBackend::Grpc,
        }
    );
}

#[test]
fn daemon_cluster_config_uses_daemon_identity() {
    let mode = ProcessMode::Daemon {
        node_id: 11,
        threads: 1,
        data_dir: "/tmp/daemon".to_owned(),
        listen_host: "10.0.0.11".to_owned(),
        listen_port: 2012,
        control_port: 3012,
        coordinator_host: "127.0.0.1".to_owned(),
        coordinator_port: 1982,
        consensus: ConsensusBackend::Mirror,
        placement: PlacementBackend::Rendezvous,
        storage: StorageBackend::Memory,
        internode_transport: TransportBackend::Grpc,
    };

    assert_eq!(
        daemon_cluster_config(&mode).nodes,
        vec![ClusterNode {
            id: 11,
            host: "10.0.0.11".to_owned(),
            control_port: 3012,
            data_port: 2012,
        }]
    );
}
