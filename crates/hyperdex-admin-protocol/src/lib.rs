use async_trait::async_trait;
use cluster_config::ClusterConfig;
use data_model::{Space, SpaceName};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigView {
    pub version: u64,
    pub stable_through: u64,
    pub cluster: ClusterConfig,
    pub spaces: Vec<Space>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdminRequest {
    CreateSpace(Space),
    CreateSpaceDsl(String),
    DropSpace(SpaceName),
    ListSpaces,
    DumpConfig,
    WaitUntilStable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdminResponse {
    Unit,
    Spaces(Vec<SpaceName>),
    Config(ConfigView),
    Stable { version: u64 },
}

#[async_trait]
pub trait HyperdexAdminService: Send + Sync {
    async fn handle(&self, request: AdminRequest) -> anyhow::Result<AdminResponse>;
}
