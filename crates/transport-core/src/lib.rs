use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternodeRequest {
    pub method: String,
    pub body: Bytes,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InternodeResponse {
    pub status: u16,
    pub body: Bytes,
}

#[async_trait]
pub trait ClusterTransport: Send + Sync {
    async fn send(&self, node: u64, request: InternodeRequest) -> Result<InternodeResponse>;
    fn name(&self) -> &'static str;
}

#[derive(Default)]
pub struct InProcessTransport;

#[async_trait]
impl ClusterTransport for InProcessTransport {
    async fn send(&self, _node: u64, request: InternodeRequest) -> Result<InternodeResponse> {
        Ok(InternodeResponse {
            status: 200,
            body: request.body,
        })
    }

    fn name(&self) -> &'static str {
        "in-process"
    }
}
