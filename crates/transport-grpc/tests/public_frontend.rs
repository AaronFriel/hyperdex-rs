use std::sync::Arc;

use cluster_config::{ClusterConfig, ClusterNode, TransportBackend};
use server::bootstrap_runtime;
use server::{ClusterRuntime, TransportRuntime};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use data_model::{Attribute as ModelAttribute, Check, Mutation, Predicate, Value as ModelValue};
use hyperdex_client_protocol::{ClientRequest, ClientResponse, HyperdexClientService};
use transport_core::{DataPlaneRequest, DataPlaneResponse, InternodeRequest, DATA_PLANE_METHOD};
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
    let runtime = Arc::new(bootstrap_runtime());
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
