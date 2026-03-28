use anyhow::Result;
use bytes::Bytes;
use data_model::{Check, Mutation, Record};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

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
    Search {
        space: String,
        checks: Vec<Check>,
    },
    ConditionalPut {
        space: String,
        key: Bytes,
        checks: Vec<Check>,
        mutations: Vec<Mutation>,
    },
    ValidatePrimary {
        space: String,
        key: Bytes,
        expected_primary: u64,
        expected_cluster_size: u64,
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
    ReplicatedDeleteGroup {
        space: String,
        checks: Vec<Check>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataPlaneResponse {
    Unit,
    Record(Option<Record>),
    SearchResult(Vec<Record>),
    Deleted(u64),
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

pub trait ClusterTransport: Send + Sync {
    fn send<'a>(
        &'a self,
        node: &'a RemoteNode,
        request: InternodeRequest,
    ) -> Pin<Box<dyn Future<Output = Result<InternodeResponse>> + Send + 'a>>;
    fn name(&self) -> &'static str;
}

#[derive(Default)]
pub struct InProcessTransport;

impl ClusterTransport for InProcessTransport {
    fn send<'a>(
        &'a self,
        _node: &'a RemoteNode,
        request: InternodeRequest,
    ) -> Pin<Box<dyn Future<Output = Result<InternodeResponse>> + Send + 'a>> {
        Box::pin(async move {
            Ok(InternodeResponse {
                status: 200,
                body: request.body,
            })
        })
    }

    fn name(&self) -> &'static str {
        "in-process"
    }
}
