use std::sync::Arc;

use server::bootstrap_runtime;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use transport_grpc::hyperdex::v1::hyperdex_admin_client::HyperdexAdminClient;
use transport_grpc::hyperdex::v1::hyperdex_admin_server::HyperdexAdminServer;
use transport_grpc::hyperdex::v1::hyperdex_client_client::HyperdexClientClient;
use transport_grpc::hyperdex::v1::hyperdex_client_server::HyperdexClientServer;
use transport_grpc::hyperdex::v1::value::Kind;
use transport_grpc::hyperdex::v1::{Attribute, CreateSpaceRequest, GetRequest, PutRequest, Value};
use transport_grpc::{HyperdexAdminGrpc, HyperdexClientGrpc};

#[tokio::test]
async fn grpc_create_space_put_get_roundtrip() {
    let runtime = Arc::new(bootstrap_runtime());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let admin_svc = HyperdexAdminGrpc::new(runtime.clone());
    let client_svc = HyperdexClientGrpc::new(runtime.clone());

    tokio::spawn(async move {
        Server::builder()
            .add_service(HyperdexAdminServer::new(admin_svc))
            .add_service(HyperdexClientServer::new(client_svc))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
                let _ = shutdown_rx.await;
            })
            .await
            .unwrap();
    });

    let endpoint = format!("http://{addr}");
    let mut admin = HyperdexAdminClient::connect(endpoint.clone()).await.unwrap();
    admin
        .create_space(CreateSpaceRequest {
            schema_dsl: "space profiles\n\
                    key username\n\
                    attributes\n\
                       string first,\n\
                       int profile_views\n\
                    tolerate 0 failures\n"
                .to_owned(),
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
