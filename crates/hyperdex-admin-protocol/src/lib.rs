use async_trait::async_trait;
use data_model::{Space, SpaceName};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdminRequest {
    CreateSpace(Space),
    DropSpace(SpaceName),
    ListSpaces,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdminResponse {
    Unit,
    Spaces(Vec<SpaceName>),
}

#[async_trait]
pub trait HyperdexAdminService: Send + Sync {
    async fn handle(&self, request: AdminRequest) -> anyhow::Result<AdminResponse>;
}
