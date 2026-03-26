use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use data_model::{Attribute as ModelAttribute, Mutation as ModelMutation, Value as ModelValue};
use prost::Message;
use server::ClusterRuntime;
use transport_core::{ClusterTransport, InternodeRequest, InternodeResponse};

pub mod hyperdex {
    pub mod v1 {
        tonic::include_proto!("hyperdex.v1");
    }
}

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

fn model_value_from_proto(
    value: hyperdex::v1::Value,
) -> std::result::Result<ModelValue, tonic::Status> {
    use hyperdex::v1::value::Kind;

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

fn proto_value_from_model(value: ModelValue) -> hyperdex::v1::Value {
    use hyperdex::v1::value::Kind;

    let kind = match value {
        ModelValue::Null => Kind::NullValue(hyperdex::v1::Null {}),
        ModelValue::Bool(v) => Kind::BoolValue(v),
        ModelValue::Int(v) => Kind::IntValue(v),
        ModelValue::Float(v) => Kind::FloatValue(v.into_inner()),
        ModelValue::Bytes(v) => Kind::BytesValue(v.to_vec()),
        ModelValue::String(v) => Kind::StringValue(v),
        ModelValue::List(v) => Kind::StringValue(format!("{v:?}")),
        ModelValue::Set(v) => Kind::StringValue(format!("{v:?}")),
        ModelValue::Map(v) => Kind::StringValue(format!("{v:?}")),
    };

    hyperdex::v1::Value { kind: Some(kind) }
}

fn model_mutations_from_attributes(
    attributes: Vec<hyperdex::v1::Attribute>,
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

#[tonic::async_trait]
impl hyperdex::v1::hyperdex_admin_server::HyperdexAdmin for HyperdexAdminGrpc {
    async fn create_space(
        &self,
        request: tonic::Request<hyperdex::v1::CreateSpaceRequest>,
    ) -> std::result::Result<tonic::Response<hyperdex::v1::CreateSpaceResponse>, tonic::Status>
    {
        let schema_dsl = request.into_inner().schema_dsl;
        hyperdex_admin_protocol::HyperdexAdminService::handle(
            self.runtime.as_ref(),
            hyperdex_admin_protocol::AdminRequest::CreateSpaceDsl(schema_dsl),
        )
        .await
        .map_err(status_from_anyhow)?;

        Ok(tonic::Response::new(hyperdex::v1::CreateSpaceResponse {}))
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

#[tonic::async_trait]
impl hyperdex::v1::hyperdex_client_server::HyperdexClient for HyperdexClientGrpc {
    async fn put(
        &self,
        request: tonic::Request<hyperdex::v1::PutRequest>,
    ) -> std::result::Result<tonic::Response<hyperdex::v1::PutResponse>, tonic::Status> {
        let request = request.into_inner();
        let mutations = model_mutations_from_attributes(request.attributes)?;

        hyperdex_client_protocol::HyperdexClientService::handle(
            self.runtime.as_ref(),
            hyperdex_client_protocol::ClientRequest::Put {
                space: request.space,
                key: Bytes::from(request.key),
                mutations,
            },
        )
        .await
        .map_err(status_from_anyhow)?;

        Ok(tonic::Response::new(hyperdex::v1::PutResponse {}))
    }

    async fn get(
        &self,
        request: tonic::Request<hyperdex::v1::GetRequest>,
    ) -> std::result::Result<tonic::Response<hyperdex::v1::GetResponse>, tonic::Status> {
        let request = request.into_inner();
        let response = hyperdex_client_protocol::HyperdexClientService::handle(
            self.runtime.as_ref(),
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
            return Ok(tonic::Response::new(hyperdex::v1::GetResponse {
                found: false,
                attributes: Vec::new(),
            }));
        };

        let attributes = record
            .attributes
            .into_iter()
            .map(|(name, value)| hyperdex::v1::Attribute {
                name,
                value: Some(proto_value_from_model(value)),
            })
            .collect();

        Ok(tonic::Response::new(hyperdex::v1::GetResponse {
            found: true,
            attributes,
        }))
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct RpcEnvelope {
    #[prost(string, tag = "1")]
    pub method: String,
    #[prost(bytes = "vec", tag = "2")]
    pub body: Vec<u8>,
}

#[derive(Default)]
pub struct GrpcTransportAdapter;

#[async_trait]
impl ClusterTransport for GrpcTransportAdapter {
    async fn send(&self, _node: u64, request: InternodeRequest) -> Result<InternodeResponse> {
        let envelope = RpcEnvelope {
            method: request.method,
            body: request.body.to_vec(),
        };
        let encoded = envelope.encode_to_vec();

        Ok(InternodeResponse {
            status: 200,
            body: Bytes::from(encoded),
        })
    }

    fn name(&self) -> &'static str {
        "grpc-prost"
    }
}
