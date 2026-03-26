use data_model::NodeId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterNode {
    pub id: NodeId,
    pub host: String,
    pub control_port: u16,
    pub data_port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusBackend {
    SingleNode,
    Mirror,
    OmniPaxos,
    OpenRaft,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementBackend {
    Hyperspace,
    Rendezvous,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageBackend {
    Memory,
    RocksDb,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportBackend {
    InProcess,
    Grpc,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PublicProtocol {
    LegacyHyperdex,
    Grpc,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub nodes: Vec<ClusterNode>,
    pub replicas: usize,
    pub consensus: ConsensusBackend,
    pub placement: PlacementBackend,
    pub storage: StorageBackend,
    pub internode_transport: TransportBackend,
    pub public_protocols: Vec<PublicProtocol>,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            nodes: vec![ClusterNode {
                id: 1,
                host: "127.0.0.1".to_owned(),
                control_port: 1982,
                data_port: 2012,
            }],
            replicas: 1,
            consensus: ConsensusBackend::SingleNode,
            placement: PlacementBackend::Hyperspace,
            storage: StorageBackend::Memory,
            internode_transport: TransportBackend::InProcess,
            public_protocols: vec![PublicProtocol::LegacyHyperdex, PublicProtocol::Grpc],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_exposes_legacy_and_grpc_frontends() {
        let config = ClusterConfig::default();

        assert_eq!(config.consensus, ConsensusBackend::SingleNode);
        assert_eq!(
            config.public_protocols,
            vec![PublicProtocol::LegacyHyperdex, PublicProtocol::Grpc]
        );
        assert_eq!(config.internode_transport, TransportBackend::InProcess);
    }
}
