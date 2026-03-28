use std::sync::Arc;

use anyhow::Result;
use bytes::Bytes;
use data_model::{Attribute as ModelAttribute, Mutation as ModelMutation, Value as ModelValue};
use grpc_api::v1;
use server::ClusterRuntime;
use std::future::Future;
use std::pin::Pin;
use transport_core::{ClusterTransport, InternodeRequest, InternodeResponse, RemoteNode};

pub mod hyperdex;

fn status_from_anyhow(err: anyhow::Error) -> tonic::Status {
    let msg = err.to_string();
    if msg.contains("already exists") {
        tonic::Status::already_exists(msg)
    } else if msg.contains("is missing") || msg.contains("not found") {
        tonic::Status::not_found(msg)
    } else {
        tonic::Status::internal(msg)
    }
}

fn model_value_from_proto(value: v1::Value) -> std::result::Result<ModelValue, tonic::Status> {
    use v1::value::Kind;

    let Some(kind) = value.kind else {
        return Err(tonic::Status::invalid_argument("missing value kind"));
    };

    Ok(match kind {
        Kind::BoolValue(v) => ModelValue::Bool(v),
        Kind::IntValue(v) => ModelValue::Int(v),
        Kind::FloatValue(v) => ModelValue::Float(ordered_float::OrderedFloat(v)),
        Kind::BytesValue(v) => ModelValue::Bytes(Bytes::from(v)),
        Kind::StringValue(v) => ModelValue::String(v),
        Kind::NullValue(_) => ModelValue::Null,
    })
}

fn proto_value_from_model(value: ModelValue) -> v1::Value {
    use v1::value::Kind;

    let kind = match value {
        ModelValue::Null => Kind::NullValue(v1::Null {}),
        ModelValue::Bool(v) => Kind::BoolValue(v),
        ModelValue::Int(v) => Kind::IntValue(v),
        ModelValue::Float(v) => Kind::FloatValue(v.into_inner()),
        ModelValue::Bytes(v) => Kind::BytesValue(v.to_vec()),
        ModelValue::String(v) => Kind::StringValue(v),
        ModelValue::List(v) => Kind::StringValue(format!("{v:?}")),
        ModelValue::Set(v) => Kind::StringValue(format!("{v:?}")),
        ModelValue::Map(v) => Kind::StringValue(format!("{v:?}")),
    };

    v1::Value { kind: Some(kind) }
}

fn model_mutations_from_attributes(
    attributes: Vec<v1::Attribute>,
) -> std::result::Result<Vec<ModelMutation>, tonic::Status> {
    let mut mutations = Vec::with_capacity(attributes.len());
    for attr in attributes {
        let value = model_value_from_proto(
            attr.value
                .ok_or_else(|| tonic::Status::invalid_argument("attribute is missing a value"))?,
        )?;
        mutations.push(ModelMutation::Set(ModelAttribute {
            name: attr.name,
            value,
        }));
    }
    Ok(mutations)
}

#[derive(Clone)]
pub struct HyperdexAdminGrpc {
    runtime: Arc<ClusterRuntime>,
}

impl HyperdexAdminGrpc {
    pub fn new(runtime: Arc<ClusterRuntime>) -> Self {
        Self { runtime }
    }
}

impl v1::hyperdex_admin_server::HyperdexAdmin for HyperdexAdminGrpc {
    fn create_space(
        &self,
        request: tonic::Request<v1::CreateSpaceRequest>,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = std::result::Result<
                        tonic::Response<v1::CreateSpaceResponse>,
                        tonic::Status,
                    >,
                > + Send,
        >,
    > {
        let runtime = self.runtime.clone();
        Box::pin(async move {
            let schema_dsl = request.into_inner().schema_dsl;
            hyperdex_admin_protocol::HyperdexAdminService::handle(
                runtime.as_ref(),
                hyperdex_admin_protocol::AdminRequest::CreateSpaceDsl(schema_dsl),
            )
            .await
            .map_err(status_from_anyhow)?;

            Ok(tonic::Response::new(v1::CreateSpaceResponse {}))
        })
    }
}

#[derive(Clone)]
pub struct HyperdexClientGrpc {
    runtime: Arc<ClusterRuntime>,
}

impl HyperdexClientGrpc {
    pub fn new(runtime: Arc<ClusterRuntime>) -> Self {
        Self { runtime }
    }
}

impl v1::hyperdex_client_server::HyperdexClient for HyperdexClientGrpc {
    fn put(
        &self,
        request: tonic::Request<v1::PutRequest>,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = std::result::Result<tonic::Response<v1::PutResponse>, tonic::Status>,
                > + Send,
        >,
    > {
        let runtime = self.runtime.clone();
        Box::pin(async move {
            let request = request.into_inner();
            let mutations = model_mutations_from_attributes(request.attributes)?;

            hyperdex_client_protocol::HyperdexClientService::handle(
                runtime.as_ref(),
                hyperdex_client_protocol::ClientRequest::Put {
                    space: request.space,
                    key: Bytes::from(request.key),
                    mutations,
                },
            )
            .await
            .map_err(status_from_anyhow)?;

            Ok(tonic::Response::new(v1::PutResponse {}))
        })
    }

    fn get(
        &self,
        request: tonic::Request<v1::GetRequest>,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = std::result::Result<tonic::Response<v1::GetResponse>, tonic::Status>,
                > + Send,
        >,
    > {
        let runtime = self.runtime.clone();
        Box::pin(async move {
            let request = request.into_inner();
            let response = hyperdex_client_protocol::HyperdexClientService::handle(
                runtime.as_ref(),
                hyperdex_client_protocol::ClientRequest::Get {
                    space: request.space,
                    key: Bytes::from(request.key),
                },
            )
            .await
            .map_err(status_from_anyhow)?;

            let record = match response {
                hyperdex_client_protocol::ClientResponse::Record(record) => record,
                other => {
                    return Err(tonic::Status::internal(format!(
                        "unexpected response from runtime: {other:?}"
                    )));
                }
            };

            let Some(record) = record else {
                return Ok(tonic::Response::new(v1::GetResponse {
                    found: false,
                    attributes: Vec::new(),
                }));
            };

            let attributes = record
                .attributes
                .into_iter()
                .map(|(name, value)| v1::Attribute {
                    name,
                    value: Some(proto_value_from_model(value)),
                })
                .collect();

            Ok(tonic::Response::new(v1::GetResponse {
                found: true,
                attributes,
            }))
        })
    }
}

#[derive(Clone)]
pub struct InternodeGrpc {
    runtime: Arc<ClusterRuntime>,
}

impl InternodeGrpc {
    pub fn new(runtime: Arc<ClusterRuntime>) -> Self {
        Self { runtime }
    }
}

impl v1::internode_transport_server::InternodeTransport for InternodeGrpc {
    fn send(
        &self,
        request: tonic::Request<v1::InternodeRpcRequest>,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = std::result::Result<
                        tonic::Response<v1::InternodeRpcResponse>,
                        tonic::Status,
                    >,
                > + Send,
        >,
    > {
        let runtime = self.runtime.clone();
        Box::pin(async move {
            let request = request.into_inner();
            let response = runtime
                .handle_internode_request(InternodeRequest {
                    method: request.method,
                    body: Bytes::from(request.body),
                })
                .await
                .map_err(status_from_anyhow)?;

            Ok(tonic::Response::new(v1::InternodeRpcResponse {
                status: response.status as u32,
                body: response.body.to_vec(),
            }))
        })
    }
}

#[derive(Default)]
pub struct GrpcTransportAdapter;

impl ClusterTransport for GrpcTransportAdapter {
    fn send<'a>(
        &'a self,
        node: &'a RemoteNode,
        request: InternodeRequest,
    ) -> Pin<Box<dyn Future<Output = Result<InternodeResponse>> + Send + 'a>> {
        Box::pin(async move {
            let endpoint = format!("http://{}:{}", node.host, node.port);
            let mut client =
                v1::internode_transport_client::InternodeTransportClient::connect(endpoint).await?;
            let response = client
                .send(v1::InternodeRpcRequest {
                    method: request.method,
                    body: request.body.to_vec(),
                })
                .await?
                .into_inner();

            Ok(InternodeResponse {
                status: response.status as u16,
                body: Bytes::from(response.body),
            })
        })
    }

    fn name(&self) -> &'static str {
        "grpc-prost"
    }
}
