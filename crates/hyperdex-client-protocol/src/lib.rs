use async_trait::async_trait;
use bytes::Bytes;
use data_model::{Check, Mutation, Record};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientRequest {
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
    Search {
        space: String,
        checks: Vec<Check>,
    },
    Count {
        space: String,
        checks: Vec<Check>,
    },
    DeleteGroup {
        space: String,
        checks: Vec<Check>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientResponse {
    Unit,
    Record(Option<Record>),
    SearchResult(Vec<Record>),
    Count(u64),
    Deleted(u64),
    ConditionFailed,
}

#[async_trait]
pub trait HyperdexClientService: Send + Sync {
    async fn handle(&self, request: ClientRequest) -> anyhow::Result<ClientResponse>;
}
