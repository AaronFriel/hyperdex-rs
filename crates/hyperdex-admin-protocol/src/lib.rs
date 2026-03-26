use anyhow::{anyhow, Result};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum CoordinatorReturnCode {
    Success = 8832,
    Malformed = 8833,
    Duplicate = 8834,
    NotFound = 8835,
    Uninitialized = 8837,
    NoCanDo = 8839,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum LegacyAdminReturnCode {
    Success = 8704,
    CoordFail = 8774,
    BadSpace = 8775,
    Duplicate = 8776,
    NotFound = 8777,
    Internal = 8829,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegacyAdminRequest {
    SpaceAddDsl(String),
    SpaceRm(SpaceName),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinatorAdminRequest {
    SpaceAdd(Space),
    SpaceRm(SpaceName),
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

impl CoordinatorAdminRequest {
    pub fn method_name(&self) -> &'static str {
        match self {
            Self::SpaceAdd(_) => "space_add",
            Self::SpaceRm(_) => "space_rm",
        }
    }
}

impl CoordinatorReturnCode {
    pub fn encode(self) -> [u8; 2] {
        (self as u16).to_be_bytes()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 2 {
            return Err(anyhow!("buffer too short for coordinator return code"));
        }

        match u16::from_be_bytes([bytes[0], bytes[1]]) {
            8832 => Ok(Self::Success),
            8833 => Ok(Self::Malformed),
            8834 => Ok(Self::Duplicate),
            8835 => Ok(Self::NotFound),
            8837 => Ok(Self::Uninitialized),
            8839 => Ok(Self::NoCanDo),
            other => Err(anyhow!("unknown coordinator return code {other}")),
        }
    }

    pub fn legacy_admin_status(self) -> LegacyAdminReturnCode {
        match self {
            Self::Success => LegacyAdminReturnCode::Success,
            Self::Duplicate => LegacyAdminReturnCode::Duplicate,
            Self::NotFound => LegacyAdminReturnCode::NotFound,
            Self::Uninitialized | Self::NoCanDo => LegacyAdminReturnCode::CoordFail,
            Self::Malformed => LegacyAdminReturnCode::Internal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data_model::{
        AttributeDefinition, SchemaFormat, SpaceOptions, Subspace, ValueKind,
    };

    #[test]
    fn coordinator_return_codes_round_trip_through_wire_bytes() {
        let codes = [
            CoordinatorReturnCode::Success,
            CoordinatorReturnCode::Malformed,
            CoordinatorReturnCode::Duplicate,
            CoordinatorReturnCode::NotFound,
            CoordinatorReturnCode::Uninitialized,
            CoordinatorReturnCode::NoCanDo,
        ];

        for code in codes {
            assert_eq!(CoordinatorReturnCode::decode(&code.encode()).unwrap(), code);
        }
    }

    #[test]
    fn coordinator_return_codes_map_to_hyperdex_admin_statuses() {
        assert_eq!(
            CoordinatorReturnCode::Success.legacy_admin_status(),
            LegacyAdminReturnCode::Success
        );
        assert_eq!(
            CoordinatorReturnCode::Duplicate.legacy_admin_status(),
            LegacyAdminReturnCode::Duplicate
        );
        assert_eq!(
            CoordinatorReturnCode::NotFound.legacy_admin_status(),
            LegacyAdminReturnCode::NotFound
        );
        assert_eq!(
            CoordinatorReturnCode::Uninitialized.legacy_admin_status(),
            LegacyAdminReturnCode::CoordFail
        );
        assert_eq!(
            CoordinatorReturnCode::NoCanDo.legacy_admin_status(),
            LegacyAdminReturnCode::CoordFail
        );
        assert_eq!(
            CoordinatorReturnCode::Malformed.legacy_admin_status(),
            LegacyAdminReturnCode::Internal
        );
    }

    #[test]
    fn coordinator_admin_requests_expose_hyperdex_method_names() {
        let space = Space {
            name: "profiles".to_owned(),
            key_attribute: "username".to_owned(),
            attributes: vec![AttributeDefinition {
                name: "first".to_owned(),
                kind: ValueKind::String,
            }],
            subspaces: vec![Subspace {
                dimensions: vec!["username".to_owned()],
            }],
            options: SpaceOptions {
                fault_tolerance: 0,
                partitions: 64,
                schema_format: SchemaFormat::HyperDexDsl,
            },
        };

        assert_eq!(
            CoordinatorAdminRequest::SpaceAdd(space).method_name(),
            "space_add"
        );
        assert_eq!(
            CoordinatorAdminRequest::SpaceRm("profiles".to_owned()).method_name(),
            "space_rm"
        );
    }
}
