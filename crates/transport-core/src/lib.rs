use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use data_model::{Check, Mutation, Record};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub const DATA_PLANE_METHOD: &str = "data-plane";

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteNode {
    pub id: u64,
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataPlaneRequest {
    Put {
        space: String,
        key: Bytes,
        mutations: Vec<Mutation>,
    },
    Get {
        space: String,
        key: Bytes,
    },
    Delete {
        space: String,
        key: Bytes,
    },
    ConditionalPut {
        space: String,
        key: Bytes,
        checks: Vec<Check>,
        mutations: Vec<Mutation>,
    },
    ReplicatedPut {
        space: String,
        key: Bytes,
        mutations: Vec<Mutation>,
    },
    ReplicatedDelete {
        space: String,
        key: Bytes,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataPlaneResponse {
    Unit,
    Record(Option<Record>),
    ConditionFailed,
}

impl InternodeRequest {
    pub fn encode<T: Serialize>(method: impl Into<String>, message: &T) -> Result<Self> {
        Ok(Self {
            method: method.into(),
            body: Bytes::from(serde_json::to_vec(message)?),
        })
    }

    pub fn decode<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_slice(&self.body)?)
    }
}

impl InternodeResponse {
    pub fn encode<T: Serialize>(status: u16, message: &T) -> Result<Self> {
        Ok(Self {
            status,
            body: Bytes::from(serde_json::to_vec(message)?),
        })
    }

    pub fn decode<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_slice(&self.body)?)
    }
}

#[async_trait]
pub trait ClusterTransport: Send + Sync {
    async fn send(&self, node: &RemoteNode, request: InternodeRequest)
        -> Result<InternodeResponse>;
    fn name(&self) -> &'static str;
}

#[derive(Default)]
pub struct InProcessTransport;

#[async_trait]
impl ClusterTransport for InProcessTransport {
    async fn send(
        &self,
        _node: &RemoteNode,
        request: InternodeRequest,
    ) -> Result<InternodeResponse> {
        Ok(InternodeResponse {
            status: 200,
            body: request.body,
        })
    }

    fn name(&self) -> &'static str {
        "in-process"
    }
}
