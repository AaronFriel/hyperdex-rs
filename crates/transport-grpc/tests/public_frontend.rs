use std::sync::Arc;
use std::time::Duration;

use cluster_config::{ClusterConfig, ClusterNode, TransportBackend};
use data_model::{Attribute as ModelAttribute, Check, Mutation, Predicate, Value as ModelValue};
use hyperdex_admin_protocol::{AdminRequest, HyperdexAdminService};
use hyperdex_client_protocol::{ClientRequest, ClientResponse, HyperdexClientService};
use legacy_frontend::{LegacyFrontend, request_once as legacy_request_once};
use legacy_protocol::{
    AtomicRequest, AtomicResponse, GetRequest as LegacyGetRequest,
    GetResponse as LegacyGetResponse, GetValue as LegacyGetValue, LEGACY_ATOMIC_FLAG_WRITE,
    LegacyFuncall, LegacyFuncallName, LegacyMessageType, LegacyReturnCode, RequestHeader,
};
use server::bootstrap_runtime;
use server::{ClusterRuntime, TransportRuntime, handle_legacy_request};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::sleep;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use transport_core::{DATA_PLANE_METHOD, DataPlaneRequest, DataPlaneResponse, InternodeRequest};
use transport_grpc::hyperdex::v1::hyperdex_admin_client::HyperdexAdminClient;
use transport_grpc::hyperdex::v1::hyperdex_admin_server::HyperdexAdminServer;
use transport_grpc::hyperdex::v1::hyperdex_client_client::HyperdexClientClient;
use transport_grpc::hyperdex::v1::hyperdex_client_server::HyperdexClientServer;
use transport_grpc::hyperdex::v1::internode_transport_server::InternodeTransportServer;
use transport_grpc::hyperdex::v1::value::Kind;
use transport_grpc::hyperdex::v1::{Attribute, CreateSpaceRequest, GetRequest, PutRequest, Value};
use transport_grpc::{GrpcTransportAdapter, HyperdexAdminGrpc, HyperdexClientGrpc, InternodeGrpc};

fn profiles_schema() -> String {
    "space profiles\n\
     key username\n\
     attributes\n\
        string first,\n\
        int profile_views\n\
     tolerate 0 failures\n"
        .to_owned()
}

fn replicated_profiles_schema() -> String {
    "space profiles\n\
     key username\n\
     attributes\n\
        string first,\n\
        int profile_views\n\
     tolerate 1 failures\n"
        .to_owned()
}

async fn serve_runtime(runtime: Arc<ClusterRuntime>, listener: TcpListener) -> oneshot::Sender<()> {
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let admin_svc = HyperdexAdminGrpc::new(runtime.clone());
    let client_svc = HyperdexClientGrpc::new(runtime.clone());
    let internode_svc = InternodeGrpc::new(runtime);

    tokio::spawn(async move {
        Server::builder()
            .add_service(HyperdexAdminServer::new(admin_svc))
            .add_service(HyperdexClientServer::new(client_svc))
            .add_service(InternodeTransportServer::new(internode_svc))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
                let _ = shutdown_rx.await;
            })
            .await
            .unwrap();
    });

    shutdown_tx
}

#[tokio::test]
async fn grpc_create_space_put_get_roundtrip() {
    let runtime = Arc::new(bootstrap_runtime().unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let shutdown_tx = serve_runtime(runtime, listener).await;

    let endpoint = format!("http://{addr}");
    let mut admin = HyperdexAdminClient::connect(endpoint.clone())
        .await
        .unwrap();
    admin
        .create_space(CreateSpaceRequest {
            schema_dsl: profiles_schema(),
        })
        .await
        .unwrap();

    let mut client = HyperdexClientClient::connect(endpoint).await.unwrap();
    client
        .put(PutRequest {
            space: "profiles".to_owned(),
            key: b"ada".to_vec(),
            attributes: vec![
                Attribute {
                    name: "username".to_owned(),
                    value: Some(Value {
                        kind: Some(Kind::BytesValue(b"ada".to_vec())),
                    }),
                },
                Attribute {
                    name: "first".to_owned(),
                    value: Some(Value {
                        kind: Some(Kind::StringValue("Ada".to_owned())),
                    }),
                },
                Attribute {
                    name: "profile_views".to_owned(),
                    value: Some(Value {
                        kind: Some(Kind::IntValue(5)),
                    }),
                },
            ],
        })
        .await
        .unwrap();

    let record = client
        .get(GetRequest {
            space: "profiles".to_owned(),
            key: b"ada".to_vec(),
        })
        .await
        .unwrap()
        .into_inner();

    assert!(record.found);
    let first = record
        .attributes
        .iter()
        .find(|attr| attr.name == "first")
        .and_then(|attr| attr.value.as_ref())
        .and_then(|value| value.kind.as_ref());

    assert!(matches!(first, Some(Kind::StringValue(v)) if v == "Ada"));

    shutdown_tx.send(()).unwrap();
}

#[tokio::test]
async fn grpc_forwards_data_plane_requests_between_two_runtimes() {
    let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr1 = listener1.local_addr().unwrap();
    let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr2 = listener2.local_addr().unwrap();

    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: addr1.port(),
                data_port: addr1.port(),
            },
            ClusterNode {
                id: 2,
                host: "127.0.0.1".to_owned(),
                control_port: addr2.port(),
                data_port: addr2.port(),
            },
        ],
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    let mut runtime1 = ClusterRuntime::for_node(config.clone(), 1).unwrap();
    runtime1.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime1 = Arc::new(runtime1);

    let mut runtime2 = ClusterRuntime::for_node(config.clone(), 2).unwrap();
    runtime2.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime2 = Arc::new(runtime2);

    let shutdown1 = serve_runtime(runtime1.clone(), listener1).await;
    let shutdown2 = serve_runtime(runtime2.clone(), listener2).await;

    let endpoint1 = format!("http://{addr1}");
    let endpoint2 = format!("http://{addr2}");

    let mut admin1 = HyperdexAdminClient::connect(endpoint1.clone())
        .await
        .unwrap();
    admin1
        .create_space(CreateSpaceRequest {
            schema_dsl: profiles_schema(),
        })
        .await
        .unwrap();

    let mut admin2 = HyperdexAdminClient::connect(endpoint2).await.unwrap();
    admin2
        .create_space(CreateSpaceRequest {
            schema_dsl: profiles_schema(),
        })
        .await
        .unwrap();

    let key = (0..4096)
        .map(|i| format!("remote-user-{i}"))
        .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 2)
        .expect("expected a key routed to node 2");

    let mut client1 = HyperdexClientClient::connect(endpoint1).await.unwrap();
    client1
        .put(PutRequest {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec(),
            attributes: vec![
                Attribute {
                    name: "username".to_owned(),
                    value: Some(Value {
                        kind: Some(Kind::BytesValue(key.as_bytes().to_vec())),
                    }),
                },
                Attribute {
                    name: "first".to_owned(),
                    value: Some(Value {
                        kind: Some(Kind::StringValue("Ada".to_owned())),
                    }),
                },
                Attribute {
                    name: "profile_views".to_owned(),
                    value: Some(Value {
                        kind: Some(Kind::IntValue(5)),
                    }),
                },
            ],
        })
        .await
        .unwrap();

    let forwarded = client1
        .get(GetRequest {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec(),
        })
        .await
        .unwrap()
        .into_inner();

    assert!(forwarded.found);

    let local_probe = InternodeRequest::encode(
        DATA_PLANE_METHOD,
        &DataPlaneRequest::Get {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec().into(),
        },
    )
    .unwrap();
    let node1_record: DataPlaneResponse = runtime1
        .handle_internode_request(local_probe.clone())
        .await
        .unwrap()
        .decode()
        .unwrap();
    let node2_record: DataPlaneResponse = runtime2
        .handle_internode_request(local_probe)
        .await
        .unwrap()
        .decode()
        .unwrap();

    assert_eq!(node1_record, DataPlaneResponse::Record(None));
    assert!(matches!(node2_record, DataPlaneResponse::Record(Some(_))));

    shutdown1.send(()).unwrap();
    shutdown2.send(()).unwrap();
}

#[tokio::test]
async fn legacy_atomic_public_path_forwards_to_remote_primary_runtime() {
    let grpc_listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr1 = grpc_listener1.local_addr().unwrap();
    let grpc_listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr2 = grpc_listener2.local_addr().unwrap();

    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr1.port(),
                data_port: grpc_addr1.port(),
            },
            ClusterNode {
                id: 2,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr2.port(),
                data_port: grpc_addr2.port(),
            },
        ],
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    let mut runtime1 = ClusterRuntime::for_node(config.clone(), 1).unwrap();
    runtime1.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime1 = Arc::new(runtime1);

    let mut runtime2 = ClusterRuntime::for_node(config.clone(), 2).unwrap();
    runtime2.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime2 = Arc::new(runtime2);

    HyperdexAdminService::handle(
        runtime1.as_ref(),
        AdminRequest::CreateSpaceDsl(replicated_profiles_schema()),
    )
    .await
    .unwrap();
    HyperdexAdminService::handle(
        runtime2.as_ref(),
        AdminRequest::CreateSpaceDsl(replicated_profiles_schema()),
    )
    .await
    .unwrap();

    let grpc_shutdown1 = serve_runtime(runtime1.clone(), grpc_listener1).await;
    let grpc_shutdown2 = serve_runtime(runtime2.clone(), grpc_listener2).await;

    let legacy_frontend1 = LegacyFrontend::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let legacy_addr1 = legacy_frontend1.local_addr().unwrap();
    let runtime1_for_legacy = runtime1.clone();
    let legacy_server1 = tokio::spawn(async move {
        legacy_frontend1
            .serve_once_with(move |header, body| {
                let runtime = runtime1_for_legacy.clone();
                async move { handle_legacy_request(runtime.as_ref(), header, &body).await }
            })
            .await
            .unwrap()
    });

    let legacy_frontend2 = LegacyFrontend::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let legacy_addr2 = legacy_frontend2.local_addr().unwrap();
    let runtime2_for_legacy = runtime2.clone();
    let legacy_server2 = tokio::spawn(async move {
        legacy_frontend2
            .serve_once_with(move |header, body| {
                let runtime = runtime2_for_legacy.clone();
                async move { handle_legacy_request(runtime.as_ref(), header, &body).await }
            })
            .await
            .unwrap()
    });

    let key = (0..4096)
        .map(|i| format!("legacy-atomic-{i}"))
        .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 2)
        .expect("expected a key routed to node 2");

    let (atomic_header, atomic_body) = legacy_request_once(
        legacy_addr1,
        RequestHeader {
            message_type: LegacyMessageType::ReqAtomic,
            flags: 0,
            version: 7,
            target_virtual_server: 0,
            nonce: 91,
        },
        &AtomicRequest {
            flags: LEGACY_ATOMIC_FLAG_WRITE,
            key: key.as_bytes().to_vec(),
            checks: Vec::new(),
            funcalls: vec![LegacyFuncall {
                attribute: "profile_views".to_owned(),
                name: LegacyFuncallName::NumAdd,
                arg1: LegacyGetValue::Int(3),
                arg2: None,
            }],
        }
        .encode_body(),
    )
    .await
    .unwrap();
    assert_eq!(atomic_header.message_type, LegacyMessageType::RespAtomic);
    assert_eq!(
        AtomicResponse::decode_body(&atomic_body).unwrap().status,
        LegacyReturnCode::Success
    );

    let (get_header, get_body) = legacy_request_once(
        legacy_addr2,
        RequestHeader {
            message_type: LegacyMessageType::ReqGet,
            flags: 0,
            version: 7,
            target_virtual_server: 0,
            nonce: 92,
        },
        &LegacyGetRequest {
            key: key.as_bytes().to_vec(),
        }
        .encode_body(),
    )
    .await
    .unwrap();
    assert_eq!(get_header.message_type, LegacyMessageType::RespGet);
    let response = LegacyGetResponse::decode_body(&get_body).unwrap();
    assert_eq!(response.status, LegacyReturnCode::Success);
    assert!(
        response
            .attributes
            .iter()
            .any(|attr| { attr.name == "profile_views" && attr.value == LegacyGetValue::Int(3) })
    );

    legacy_server1.await.unwrap();
    legacy_server2.await.unwrap();
    grpc_shutdown1.send(()).unwrap();
    grpc_shutdown2.send(()).unwrap();
}

#[tokio::test]
async fn grpc_forwards_delete_requests_between_two_runtimes() {
    let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr1 = listener1.local_addr().unwrap();
    let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr2 = listener2.local_addr().unwrap();

    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: addr1.port(),
                data_port: addr1.port(),
            },
            ClusterNode {
                id: 2,
                host: "127.0.0.1".to_owned(),
                control_port: addr2.port(),
                data_port: addr2.port(),
            },
        ],
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    let mut runtime1 = ClusterRuntime::for_node(config.clone(), 1).unwrap();
    runtime1.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime1 = Arc::new(runtime1);

    let mut runtime2 = ClusterRuntime::for_node(config.clone(), 2).unwrap();
    runtime2.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime2 = Arc::new(runtime2);

    let shutdown1 = serve_runtime(runtime1.clone(), listener1).await;
    let shutdown2 = serve_runtime(runtime2.clone(), listener2).await;

    let endpoint1 = format!("http://{addr1}");
    let endpoint2 = format!("http://{addr2}");

    let mut admin1 = HyperdexAdminClient::connect(endpoint1.clone())
        .await
        .unwrap();
    admin1
        .create_space(CreateSpaceRequest {
            schema_dsl: profiles_schema(),
        })
        .await
        .unwrap();

    let mut admin2 = HyperdexAdminClient::connect(endpoint2).await.unwrap();
    admin2
        .create_space(CreateSpaceRequest {
            schema_dsl: profiles_schema(),
        })
        .await
        .unwrap();

    let key = (0..4096)
        .map(|i| format!("delete-user-{i}"))
        .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 2)
        .expect("expected a key routed to node 2");

    let mut client1 = HyperdexClientClient::connect(endpoint1).await.unwrap();
    client1
        .put(PutRequest {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec(),
            attributes: vec![
                Attribute {
                    name: "username".to_owned(),
                    value: Some(Value {
                        kind: Some(Kind::BytesValue(key.as_bytes().to_vec())),
                    }),
                },
                Attribute {
                    name: "first".to_owned(),
                    value: Some(Value {
                        kind: Some(Kind::StringValue("Ada".to_owned())),
                    }),
                },
                Attribute {
                    name: "profile_views".to_owned(),
                    value: Some(Value {
                        kind: Some(Kind::IntValue(5)),
                    }),
                },
            ],
        })
        .await
        .unwrap();

    let delete = HyperdexClientService::handle(
        runtime1.as_ref(),
        ClientRequest::Delete {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec().into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(delete, ClientResponse::Unit);

    let local_probe = InternodeRequest::encode(
        DATA_PLANE_METHOD,
        &DataPlaneRequest::Get {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec().into(),
        },
    )
    .unwrap();
    let node2_record: DataPlaneResponse = runtime2
        .handle_internode_request(local_probe)
        .await
        .unwrap()
        .decode()
        .unwrap();

    assert_eq!(node2_record, DataPlaneResponse::Record(None));

    shutdown1.send(()).unwrap();
    shutdown2.send(()).unwrap();
}

#[tokio::test]
async fn grpc_forwards_conditional_put_requests_between_two_runtimes() {
    let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr1 = listener1.local_addr().unwrap();
    let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr2 = listener2.local_addr().unwrap();

    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: addr1.port(),
                data_port: addr1.port(),
            },
            ClusterNode {
                id: 2,
                host: "127.0.0.1".to_owned(),
                control_port: addr2.port(),
                data_port: addr2.port(),
            },
        ],
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    let mut runtime1 = ClusterRuntime::for_node(config.clone(), 1).unwrap();
    runtime1.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime1 = Arc::new(runtime1);

    let mut runtime2 = ClusterRuntime::for_node(config.clone(), 2).unwrap();
    runtime2.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime2 = Arc::new(runtime2);

    let shutdown1 = serve_runtime(runtime1.clone(), listener1).await;
    let shutdown2 = serve_runtime(runtime2.clone(), listener2).await;

    let endpoint1 = format!("http://{addr1}");
    let endpoint2 = format!("http://{addr2}");

    let mut admin1 = HyperdexAdminClient::connect(endpoint1.clone())
        .await
        .unwrap();
    admin1
        .create_space(CreateSpaceRequest {
            schema_dsl: profiles_schema(),
        })
        .await
        .unwrap();

    let mut admin2 = HyperdexAdminClient::connect(endpoint2).await.unwrap();
    admin2
        .create_space(CreateSpaceRequest {
            schema_dsl: profiles_schema(),
        })
        .await
        .unwrap();

    let key = (0..4096)
        .map(|i| format!("conditional-user-{i}"))
        .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 2)
        .expect("expected a key routed to node 2");

    let mut client1 = HyperdexClientClient::connect(endpoint1).await.unwrap();
    client1
        .put(PutRequest {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec(),
            attributes: vec![
                Attribute {
                    name: "username".to_owned(),
                    value: Some(Value {
                        kind: Some(Kind::BytesValue(key.as_bytes().to_vec())),
                    }),
                },
                Attribute {
                    name: "first".to_owned(),
                    value: Some(Value {
                        kind: Some(Kind::StringValue("Ada".to_owned())),
                    }),
                },
                Attribute {
                    name: "profile_views".to_owned(),
                    value: Some(Value {
                        kind: Some(Kind::IntValue(5)),
                    }),
                },
            ],
        })
        .await
        .unwrap();

    let success = HyperdexClientService::handle(
        runtime1.as_ref(),
        ClientRequest::ConditionalPut {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec().into(),
            checks: vec![Check {
                attribute: "first".to_owned(),
                predicate: Predicate::Equal,
                value: ModelValue::String("Ada".to_owned()),
            }],
            mutations: vec![Mutation::Set(ModelAttribute {
                name: "first".to_owned(),
                value: ModelValue::String("Grace".to_owned()),
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
            key: key.as_bytes().to_vec().into(),
            checks: vec![Check {
                attribute: "first".to_owned(),
                predicate: Predicate::Equal,
                value: ModelValue::String("Ada".to_owned()),
            }],
            mutations: vec![Mutation::Set(ModelAttribute {
                name: "first".to_owned(),
                value: ModelValue::String("Katherine".to_owned()),
            })],
        },
    )
    .await
    .unwrap();
    assert_eq!(compare_failed, ClientResponse::ConditionFailed);

    let local_probe = InternodeRequest::encode(
        DATA_PLANE_METHOD,
        &DataPlaneRequest::Get {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec().into(),
        },
    )
    .unwrap();
    let node2_record: DataPlaneResponse = runtime2
        .handle_internode_request(local_probe)
        .await
        .unwrap()
        .decode()
        .unwrap();

    match node2_record {
        DataPlaneResponse::Record(Some(record)) => {
            assert_eq!(
                record.attributes.get("first"),
                Some(&ModelValue::String("Grace".to_owned()))
            );
        }
        other => panic!("expected remote record after conditional put, got {other:?}"),
    }

    shutdown1.send(()).unwrap();
    shutdown2.send(()).unwrap();
}

#[tokio::test]
async fn grpc_forwards_numeric_mutation_requests_between_two_runtimes() {
    let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr1 = listener1.local_addr().unwrap();
    let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr2 = listener2.local_addr().unwrap();

    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: addr1.port(),
                data_port: addr1.port(),
            },
            ClusterNode {
                id: 2,
                host: "127.0.0.1".to_owned(),
                control_port: addr2.port(),
                data_port: addr2.port(),
            },
        ],
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    let mut runtime1 = ClusterRuntime::for_node(config.clone(), 1).unwrap();
    runtime1.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime1 = Arc::new(runtime1);

    let mut runtime2 = ClusterRuntime::for_node(config.clone(), 2).unwrap();
    runtime2.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime2 = Arc::new(runtime2);

    let shutdown1 = serve_runtime(runtime1.clone(), listener1).await;
    let shutdown2 = serve_runtime(runtime2.clone(), listener2).await;

    let endpoint1 = format!("http://{addr1}");
    let endpoint2 = format!("http://{addr2}");

    let mut admin1 = HyperdexAdminClient::connect(endpoint1.clone())
        .await
        .unwrap();
    admin1
        .create_space(CreateSpaceRequest {
            schema_dsl: profiles_schema(),
        })
        .await
        .unwrap();

    let mut admin2 = HyperdexAdminClient::connect(endpoint2).await.unwrap();
    admin2
        .create_space(CreateSpaceRequest {
            schema_dsl: profiles_schema(),
        })
        .await
        .unwrap();

    let key = (0..4096)
        .map(|i| format!("numeric-user-{i}"))
        .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 2)
        .expect("expected a key routed to node 2");

    let mut client1 = HyperdexClientClient::connect(endpoint1).await.unwrap();
    client1
        .put(PutRequest {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec(),
            attributes: vec![
                Attribute {
                    name: "username".to_owned(),
                    value: Some(Value {
                        kind: Some(Kind::BytesValue(key.as_bytes().to_vec())),
                    }),
                },
                Attribute {
                    name: "first".to_owned(),
                    value: Some(Value {
                        kind: Some(Kind::StringValue("Ada".to_owned())),
                    }),
                },
                Attribute {
                    name: "profile_views".to_owned(),
                    value: Some(Value {
                        kind: Some(Kind::IntValue(5)),
                    }),
                },
            ],
        })
        .await
        .unwrap();

    let numeric = HyperdexClientService::handle(
        runtime1.as_ref(),
        ClientRequest::Put {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec().into(),
            mutations: vec![Mutation::Numeric {
                attribute: "profile_views".to_owned(),
                op: data_model::NumericOp::Add,
                operand: 7,
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(numeric, ClientResponse::Unit);

    let local_probe = InternodeRequest::encode(
        DATA_PLANE_METHOD,
        &DataPlaneRequest::Get {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec().into(),
        },
    )
    .unwrap();
    let node2_record: DataPlaneResponse = runtime2
        .handle_internode_request(local_probe)
        .await
        .unwrap()
        .decode()
        .unwrap();

    match node2_record {
        DataPlaneResponse::Record(Some(record)) => {
            assert_eq!(
                record.attributes.get("profile_views"),
                Some(&ModelValue::Int(12))
            );
        }
        other => panic!("expected remote record after numeric mutation, got {other:?}"),
    }

    shutdown1.send(()).unwrap();
    shutdown2.send(()).unwrap();
}

#[tokio::test]
async fn legacy_atomic_replicates_to_secondary_runtime() {
    let grpc_listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr1 = grpc_listener1.local_addr().unwrap();
    let grpc_listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr2 = grpc_listener2.local_addr().unwrap();

    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr1.port(),
                data_port: grpc_addr1.port(),
            },
            ClusterNode {
                id: 2,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr2.port(),
                data_port: grpc_addr2.port(),
            },
        ],
        replicas: 2,
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    let mut runtime1 = ClusterRuntime::for_node(config.clone(), 1).unwrap();
    runtime1.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime1 = Arc::new(runtime1);

    let mut runtime2 = ClusterRuntime::for_node(config.clone(), 2).unwrap();
    runtime2.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime2 = Arc::new(runtime2);

    HyperdexAdminService::handle(
        runtime1.as_ref(),
        AdminRequest::CreateSpaceDsl(replicated_profiles_schema()),
    )
    .await
    .unwrap();
    HyperdexAdminService::handle(
        runtime2.as_ref(),
        AdminRequest::CreateSpaceDsl(replicated_profiles_schema()),
    )
    .await
    .unwrap();

    let grpc_shutdown1 = serve_runtime(runtime1.clone(), grpc_listener1).await;
    let grpc_shutdown2 = serve_runtime(runtime2.clone(), grpc_listener2).await;

    let legacy_frontend1 = LegacyFrontend::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let legacy_addr1 = legacy_frontend1.local_addr().unwrap();
    let runtime1_for_legacy = runtime1.clone();
    let legacy_server1 = tokio::spawn(async move {
        legacy_frontend1
            .serve_once_with(move |header, body| {
                let runtime = runtime1_for_legacy.clone();
                async move { handle_legacy_request(runtime.as_ref(), header, &body).await }
            })
            .await
            .unwrap()
    });

    let key = (0..4096)
        .map(|i| format!("legacy-replicated-{i}"))
        .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 2)
        .expect("expected a key routed to node 2");

    let (atomic_header, atomic_body) = legacy_request_once(
        legacy_addr1,
        RequestHeader {
            message_type: LegacyMessageType::ReqAtomic,
            flags: 0,
            version: 7,
            target_virtual_server: 0,
            nonce: 101,
        },
        &AtomicRequest {
            flags: LEGACY_ATOMIC_FLAG_WRITE,
            key: key.as_bytes().to_vec(),
            checks: Vec::new(),
            funcalls: vec![LegacyFuncall {
                attribute: "profile_views".to_owned(),
                name: LegacyFuncallName::NumAdd,
                arg1: LegacyGetValue::Int(3),
                arg2: None,
            }],
        }
        .encode_body(),
    )
    .await
    .unwrap();
    assert_eq!(atomic_header.message_type, LegacyMessageType::RespAtomic);
    assert_eq!(
        AtomicResponse::decode_body(&atomic_body).unwrap().status,
        LegacyReturnCode::Success
    );

    let local_probe = InternodeRequest::encode(
        DATA_PLANE_METHOD,
        &DataPlaneRequest::Get {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec().into(),
        },
    )
    .unwrap();
    let secondary_record: DataPlaneResponse = runtime1
        .handle_internode_request(local_probe.clone())
        .await
        .unwrap()
        .decode()
        .unwrap();
    let primary_record: DataPlaneResponse = runtime2
        .handle_internode_request(local_probe)
        .await
        .unwrap()
        .decode()
        .unwrap();

    assert!(matches!(
        secondary_record,
        DataPlaneResponse::Record(Some(_))
    ));
    assert!(matches!(primary_record, DataPlaneResponse::Record(Some(_))));

    legacy_server1.await.unwrap();
    grpc_shutdown1.send(()).unwrap();
    grpc_shutdown2.send(()).unwrap();
}

#[tokio::test]
async fn distributed_delete_removes_replicated_state_from_secondary_runtime() {
    let grpc_listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr1 = grpc_listener1.local_addr().unwrap();
    let grpc_listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr2 = grpc_listener2.local_addr().unwrap();

    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr1.port(),
                data_port: grpc_addr1.port(),
            },
            ClusterNode {
                id: 2,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr2.port(),
                data_port: grpc_addr2.port(),
            },
        ],
        replicas: 2,
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    let mut runtime1 = ClusterRuntime::for_node(config.clone(), 1).unwrap();
    runtime1.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime1 = Arc::new(runtime1);

    let mut runtime2 = ClusterRuntime::for_node(config.clone(), 2).unwrap();
    runtime2.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime2 = Arc::new(runtime2);

    HyperdexAdminService::handle(
        runtime1.as_ref(),
        AdminRequest::CreateSpaceDsl(replicated_profiles_schema()),
    )
    .await
    .unwrap();
    HyperdexAdminService::handle(
        runtime2.as_ref(),
        AdminRequest::CreateSpaceDsl(replicated_profiles_schema()),
    )
    .await
    .unwrap();

    let grpc_shutdown1 = serve_runtime(runtime1.clone(), grpc_listener1).await;
    let grpc_shutdown2 = serve_runtime(runtime2.clone(), grpc_listener2).await;

    let key = (0..4096)
        .map(|i| format!("replicated-delete-{i}"))
        .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 2)
        .expect("expected a key routed to node 2");

    HyperdexClientService::handle(
        runtime1.as_ref(),
        ClientRequest::Put {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec().into(),
            mutations: vec![Mutation::Numeric {
                attribute: "profile_views".to_owned(),
                op: data_model::NumericOp::Add,
                operand: 3,
            }],
        },
    )
    .await
    .unwrap();

    let delete = HyperdexClientService::handle(
        runtime1.as_ref(),
        ClientRequest::Delete {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec().into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(delete, ClientResponse::Unit);

    let local_probe = InternodeRequest::encode(
        DATA_PLANE_METHOD,
        &DataPlaneRequest::Get {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec().into(),
        },
    )
    .unwrap();
    let secondary_record: DataPlaneResponse = runtime1
        .handle_internode_request(local_probe.clone())
        .await
        .unwrap()
        .decode()
        .unwrap();
    let primary_record: DataPlaneResponse = runtime2
        .handle_internode_request(local_probe)
        .await
        .unwrap()
        .decode()
        .unwrap();

    assert_eq!(secondary_record, DataPlaneResponse::Record(None));
    assert_eq!(primary_record, DataPlaneResponse::Record(None));

    grpc_shutdown1.send(()).unwrap();
    grpc_shutdown2.send(()).unwrap();
}

#[tokio::test]
async fn distributed_delete_group_removes_matching_records_from_all_replicas() {
    let grpc_listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr1 = grpc_listener1.local_addr().unwrap();
    let grpc_listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr2 = grpc_listener2.local_addr().unwrap();

    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr1.port(),
                data_port: grpc_addr1.port(),
            },
            ClusterNode {
                id: 2,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr2.port(),
                data_port: grpc_addr2.port(),
            },
        ],
        replicas: 2,
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    let mut runtime1 = ClusterRuntime::for_node(config.clone(), 1).unwrap();
    runtime1.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime1 = Arc::new(runtime1);

    let mut runtime2 = ClusterRuntime::for_node(config.clone(), 2).unwrap();
    runtime2.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime2 = Arc::new(runtime2);

    HyperdexAdminService::handle(
        runtime1.as_ref(),
        AdminRequest::CreateSpaceDsl(replicated_profiles_schema()),
    )
    .await
    .unwrap();
    HyperdexAdminService::handle(
        runtime2.as_ref(),
        AdminRequest::CreateSpaceDsl(replicated_profiles_schema()),
    )
    .await
    .unwrap();

    let grpc_shutdown1 = serve_runtime(runtime1.clone(), grpc_listener1).await;
    let grpc_shutdown2 = serve_runtime(runtime2.clone(), grpc_listener2).await;

    let matching_key = b"group-match".to_vec();
    let survivor_key = b"group-survivor".to_vec();

    for (key, views) in [(&matching_key, 7_i64), (&survivor_key, 1_i64)] {
        HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: key.clone().into(),
                mutations: vec![Mutation::Numeric {
                    attribute: "profile_views".to_owned(),
                    op: data_model::NumericOp::Add,
                    operand: views,
                }],
            },
        )
        .await
        .unwrap();
    }

    let deleted = HyperdexClientService::handle(
        runtime1.as_ref(),
        ClientRequest::DeleteGroup {
            space: "profiles".to_owned(),
            checks: vec![Check {
                attribute: "profile_views".to_owned(),
                predicate: Predicate::GreaterThanOrEqual,
                value: ModelValue::Int(5),
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(deleted, ClientResponse::Deleted(1));

    for runtime in [&runtime1, &runtime2] {
        let match_probe = InternodeRequest::encode(
            DATA_PLANE_METHOD,
            &DataPlaneRequest::Get {
                space: "profiles".to_owned(),
                key: matching_key.clone().into(),
            },
        )
        .unwrap();
        let survivor_probe = InternodeRequest::encode(
            DATA_PLANE_METHOD,
            &DataPlaneRequest::Get {
                space: "profiles".to_owned(),
                key: survivor_key.clone().into(),
            },
        )
        .unwrap();

        let removed: DataPlaneResponse = runtime
            .handle_internode_request(match_probe)
            .await
            .unwrap()
            .decode()
            .unwrap();
        let survivor: DataPlaneResponse = runtime
            .handle_internode_request(survivor_probe)
            .await
            .unwrap()
            .decode()
            .unwrap();

        assert_eq!(removed, DataPlaneResponse::Record(None));
        assert!(matches!(survivor, DataPlaneResponse::Record(Some(_))));
    }

    grpc_shutdown1.send(()).unwrap();
    grpc_shutdown2.send(()).unwrap();
}

#[tokio::test]
async fn distributed_search_dedupes_replicated_records_across_runtimes() {
    let grpc_listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr1 = grpc_listener1.local_addr().unwrap();
    let grpc_listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr2 = grpc_listener2.local_addr().unwrap();

    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr1.port(),
                data_port: grpc_addr1.port(),
            },
            ClusterNode {
                id: 2,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr2.port(),
                data_port: grpc_addr2.port(),
            },
        ],
        replicas: 2,
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    let mut runtime1 = ClusterRuntime::for_node(config.clone(), 1).unwrap();
    runtime1.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime1 = Arc::new(runtime1);

    let mut runtime2 = ClusterRuntime::for_node(config.clone(), 2).unwrap();
    runtime2.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime2 = Arc::new(runtime2);

    HyperdexAdminService::handle(
        runtime1.as_ref(),
        AdminRequest::CreateSpaceDsl(replicated_profiles_schema()),
    )
    .await
    .unwrap();
    HyperdexAdminService::handle(
        runtime2.as_ref(),
        AdminRequest::CreateSpaceDsl(replicated_profiles_schema()),
    )
    .await
    .unwrap();

    let grpc_shutdown1 = serve_runtime(runtime1.clone(), grpc_listener1).await;
    let grpc_shutdown2 = serve_runtime(runtime2.clone(), grpc_listener2).await;

    let matching_keys = [b"search-match-a".to_vec(), b"search-match-b".to_vec()];
    let survivor_key = b"search-survivor".to_vec();

    for (key, views) in matching_keys
        .iter()
        .map(|key| (key, 7_i64))
        .chain(std::iter::once((&survivor_key, 1_i64)))
    {
        HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: key.clone().into(),
                mutations: vec![Mutation::Numeric {
                    attribute: "profile_views".to_owned(),
                    op: data_model::NumericOp::Add,
                    operand: views,
                }],
            },
        )
        .await
        .unwrap();
    }

    for runtime in [&runtime1, &runtime2] {
        let response = HyperdexClientService::handle(
            runtime.as_ref(),
            ClientRequest::Search {
                space: "profiles".to_owned(),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::GreaterThanOrEqual,
                    value: ModelValue::Int(5),
                }],
            },
        )
        .await
        .unwrap();

        let ClientResponse::SearchResult(records) = response else {
            panic!("expected search result response");
        };

        let mut keys: Vec<Vec<u8>> = records
            .into_iter()
            .map(|record| record.key.to_vec())
            .collect();
        keys.sort();

        assert_eq!(keys, matching_keys);
    }

    grpc_shutdown1.send(()).unwrap();
    grpc_shutdown2.send(()).unwrap();
}

#[tokio::test]
async fn distributed_count_returns_logical_matches_from_any_runtime() {
    let grpc_listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr1 = grpc_listener1.local_addr().unwrap();
    let grpc_listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr2 = grpc_listener2.local_addr().unwrap();
    let grpc_listener3 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr3 = grpc_listener3.local_addr().unwrap();

    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr1.port(),
                data_port: grpc_addr1.port(),
            },
            ClusterNode {
                id: 2,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr2.port(),
                data_port: grpc_addr2.port(),
            },
            ClusterNode {
                id: 3,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr3.port(),
                data_port: grpc_addr3.port(),
            },
        ],
        replicas: 2,
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    let mut runtime1 = ClusterRuntime::for_node(config.clone(), 1).unwrap();
    runtime1.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime1 = Arc::new(runtime1);

    let mut runtime2 = ClusterRuntime::for_node(config.clone(), 2).unwrap();
    runtime2.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime2 = Arc::new(runtime2);

    let mut runtime3 = ClusterRuntime::for_node(config.clone(), 3).unwrap();
    runtime3.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime3 = Arc::new(runtime3);

    for runtime in [&runtime1, &runtime2, &runtime3] {
        HyperdexAdminService::handle(
            runtime.as_ref(),
            AdminRequest::CreateSpaceDsl(profiles_schema()),
        )
        .await
        .unwrap();
    }

    let grpc_shutdown1 = serve_runtime(runtime1.clone(), grpc_listener1).await;
    let grpc_shutdown2 = serve_runtime(runtime2.clone(), grpc_listener2).await;
    let grpc_shutdown3 = serve_runtime(runtime3.clone(), grpc_listener3).await;

    let matching_keys = [
        b"count-match-a".to_vec(),
        b"count-match-b".to_vec(),
        b"count-match-c".to_vec(),
    ];
    let survivor_key = b"count-survivor".to_vec();

    for (key, views) in matching_keys
        .iter()
        .map(|key| (key, 9_i64))
        .chain(std::iter::once((&survivor_key, 1_i64)))
    {
        HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: key.clone().into(),
                mutations: vec![Mutation::Numeric {
                    attribute: "profile_views".to_owned(),
                    op: data_model::NumericOp::Add,
                    operand: views,
                }],
            },
        )
        .await
        .unwrap();
    }

    for runtime in [&runtime1, &runtime2, &runtime3] {
        let response = HyperdexClientService::handle(
            runtime.as_ref(),
            ClientRequest::Count {
                space: "profiles".to_owned(),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::GreaterThanOrEqual,
                    value: ModelValue::Int(5),
                }],
            },
        )
        .await
        .unwrap();

        assert_eq!(response, ClientResponse::Count(3));
    }

    grpc_shutdown1.send(()).unwrap();
    grpc_shutdown2.send(()).unwrap();
    grpc_shutdown3.send(()).unwrap();
}

#[tokio::test]
async fn distributed_get_falls_back_to_local_replica_when_primary_grpc_is_down() {
    let grpc_listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr1 = grpc_listener1.local_addr().unwrap();
    let grpc_listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr2 = grpc_listener2.local_addr().unwrap();

    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr1.port(),
                data_port: grpc_addr1.port(),
            },
            ClusterNode {
                id: 2,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr2.port(),
                data_port: grpc_addr2.port(),
            },
        ],
        replicas: 2,
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    let mut runtime1 = ClusterRuntime::for_node(config.clone(), 1).unwrap();
    runtime1.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime1 = Arc::new(runtime1);

    let mut runtime2 = ClusterRuntime::for_node(config.clone(), 2).unwrap();
    runtime2.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime2 = Arc::new(runtime2);

    HyperdexAdminService::handle(
        runtime1.as_ref(),
        AdminRequest::CreateSpaceDsl(replicated_profiles_schema()),
    )
    .await
    .unwrap();
    HyperdexAdminService::handle(
        runtime2.as_ref(),
        AdminRequest::CreateSpaceDsl(replicated_profiles_schema()),
    )
    .await
    .unwrap();

    let grpc_shutdown1 = serve_runtime(runtime1.clone(), grpc_listener1).await;
    let grpc_shutdown2 = serve_runtime(runtime2.clone(), grpc_listener2).await;

    let key = (0..4096)
        .map(|i| format!("fallback-get-{i}"))
        .find(|key| runtime1.route_primary(key.as_bytes()).unwrap() == 2)
        .expect("expected a key routed to node 2");

    HyperdexClientService::handle(
        runtime1.as_ref(),
        ClientRequest::Put {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec().into(),
            mutations: vec![Mutation::Numeric {
                attribute: "profile_views".to_owned(),
                op: data_model::NumericOp::Add,
                operand: 11,
            }],
        },
    )
    .await
    .unwrap();

    let local_probe = InternodeRequest::encode(
        DATA_PLANE_METHOD,
        &DataPlaneRequest::Get {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec().into(),
        },
    )
    .unwrap();
    let secondary_record: DataPlaneResponse = runtime1
        .handle_internode_request(local_probe)
        .await
        .unwrap()
        .decode()
        .unwrap();
    assert!(matches!(
        secondary_record,
        DataPlaneResponse::Record(Some(_))
    ));

    grpc_shutdown2.send(()).unwrap();
    sleep(Duration::from_millis(100)).await;

    let endpoint = format!("http://{grpc_addr1}");
    let mut client = HyperdexClientClient::connect(endpoint).await.unwrap();
    let record = client
        .get(GetRequest {
            space: "profiles".to_owned(),
            key: key.as_bytes().to_vec(),
        })
        .await
        .unwrap()
        .into_inner();

    assert!(record.found);
    assert!(record.attributes.iter().any(|attribute| {
        attribute.name == "profile_views"
            && matches!(
                attribute
                    .value
                    .as_ref()
                    .and_then(|value| value.kind.as_ref()),
                Some(Kind::IntValue(11))
            )
    }));

    grpc_shutdown1.send(()).unwrap();
}

#[tokio::test]
async fn distributed_search_and_count_survive_one_daemon_shutdown() {
    let grpc_listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr1 = grpc_listener1.local_addr().unwrap();
    let grpc_listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let grpc_addr2 = grpc_listener2.local_addr().unwrap();

    let config = ClusterConfig {
        nodes: vec![
            ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr1.port(),
                data_port: grpc_addr1.port(),
            },
            ClusterNode {
                id: 2,
                host: "127.0.0.1".to_owned(),
                control_port: grpc_addr2.port(),
                data_port: grpc_addr2.port(),
            },
        ],
        replicas: 2,
        internode_transport: TransportBackend::Grpc,
        ..ClusterConfig::default()
    };

    let mut runtime1 = ClusterRuntime::for_node(config.clone(), 1).unwrap();
    runtime1.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime1 = Arc::new(runtime1);

    let mut runtime2 = ClusterRuntime::for_node(config.clone(), 2).unwrap();
    runtime2.install_cluster_transport(Arc::new(GrpcTransportAdapter), TransportRuntime::Grpc);
    let runtime2 = Arc::new(runtime2);

    HyperdexAdminService::handle(
        runtime1.as_ref(),
        AdminRequest::CreateSpaceDsl(replicated_profiles_schema()),
    )
    .await
    .unwrap();
    HyperdexAdminService::handle(
        runtime2.as_ref(),
        AdminRequest::CreateSpaceDsl(replicated_profiles_schema()),
    )
    .await
    .unwrap();

    let grpc_shutdown1 = serve_runtime(runtime1.clone(), grpc_listener1).await;
    let grpc_shutdown2 = serve_runtime(runtime2.clone(), grpc_listener2).await;

    let matching_keys = [b"degraded-search-a".to_vec(), b"degraded-search-b".to_vec()];
    let survivor_key = b"degraded-search-survivor".to_vec();

    for (key, views) in matching_keys
        .iter()
        .map(|key| (key, 6_i64))
        .chain(std::iter::once((&survivor_key, 1_i64)))
    {
        HyperdexClientService::handle(
            runtime1.as_ref(),
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: key.clone().into(),
                mutations: vec![Mutation::Numeric {
                    attribute: "profile_views".to_owned(),
                    op: data_model::NumericOp::Add,
                    operand: views,
                }],
            },
        )
        .await
        .unwrap();
    }

    grpc_shutdown2.send(()).unwrap();
    sleep(Duration::from_millis(100)).await;

    let search = HyperdexClientService::handle(
        runtime1.as_ref(),
        ClientRequest::Search {
            space: "profiles".to_owned(),
            checks: vec![Check {
                attribute: "profile_views".to_owned(),
                predicate: Predicate::GreaterThanOrEqual,
                value: ModelValue::Int(5),
            }],
        },
    )
    .await
    .unwrap();
    let ClientResponse::SearchResult(records) = search else {
        panic!("expected search result response");
    };
    let mut keys: Vec<Vec<u8>> = records
        .into_iter()
        .map(|record| record.key.to_vec())
        .collect();
    keys.sort();
    assert_eq!(keys, matching_keys);

    let count = HyperdexClientService::handle(
        runtime1.as_ref(),
        ClientRequest::Count {
            space: "profiles".to_owned(),
            checks: vec![Check {
                attribute: "profile_views".to_owned(),
                predicate: Predicate::GreaterThanOrEqual,
                value: ModelValue::Int(5),
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(count, ClientResponse::Count(2));

    grpc_shutdown1.send(()).unwrap();
}
