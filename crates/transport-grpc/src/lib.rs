use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use prost::Message;
use transport_core::{ClusterTransport, InternodeRequest, InternodeResponse};

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
