use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use control_plane::{Catalog, InMemoryCatalog};
use data_model::{parse_hyperdex_space, Space};
use data_plane::DataPlane;
use hyperdex_admin_protocol::{AdminRequest, AdminResponse, HyperdexAdminService};
use hyperdex_client_protocol::{ClientRequest, ClientResponse, HyperdexClientService};
use placement_core::{HyperSpacePlacement, PlacementStrategy};
use storage_core::{StorageEngine, WriteResult};

use cluster_config::ClusterConfig;
use engine_memory::MemoryEngine;

pub struct ClusterRuntime {
    catalog: Arc<dyn Catalog>,
    storage: Arc<dyn StorageEngine>,
    data_plane: DataPlane,
}

impl ClusterRuntime {
    pub fn new(
        catalog: Arc<dyn Catalog>,
        storage: Arc<dyn StorageEngine>,
        placement: Arc<dyn PlacementStrategy>,
    ) -> Self {
        let data_plane = DataPlane::new(catalog.clone(), storage.clone(), placement);
        Self {
            catalog,
            storage,
            data_plane,
        }
    }

    pub fn single_node(config: ClusterConfig) -> Self {
        let catalog: Arc<dyn Catalog> =
            Arc::new(InMemoryCatalog::new(config.nodes.clone(), config.replicas));
        let storage: Arc<dyn StorageEngine> = Arc::new(MemoryEngine::new());

        Self::new(catalog, storage, Arc::new(HyperSpacePlacement))
    }

    fn create_space(&self, space: Space) -> Result<()> {
        self.storage.create_space(space.name.clone())?;
        self.catalog.create_space(space)?;
        Ok(())
    }
}

#[async_trait]
impl HyperdexAdminService for ClusterRuntime {
    async fn handle(&self, request: AdminRequest) -> Result<AdminResponse> {
        match request {
            AdminRequest::CreateSpace(space) => {
                self.create_space(space)?;
                Ok(AdminResponse::Unit)
            }
            AdminRequest::CreateSpaceDsl(schema) => {
                self.create_space(parse_hyperdex_space(&schema)?)?;
                Ok(AdminResponse::Unit)
            }
            AdminRequest::DropSpace(space) => {
                self.catalog.drop_space(&space)?;
                self.storage.drop_space(&space)?;
                Ok(AdminResponse::Unit)
            }
            AdminRequest::ListSpaces => Ok(AdminResponse::Spaces(self.catalog.list_spaces()?)),
        }
    }
}

#[async_trait]
impl HyperdexClientService for ClusterRuntime {
    async fn handle(&self, request: ClientRequest) -> Result<ClientResponse> {
        match request {
            ClientRequest::Put {
                space,
                key,
                mutations,
            } => Ok(match self.data_plane.put(&space, key, &mutations)? {
                WriteResult::Written => ClientResponse::Unit,
                WriteResult::ConditionFailed => ClientResponse::ConditionFailed,
                WriteResult::Missing => ClientResponse::Unit,
            }),
            ClientRequest::Get { space, key } => {
                Ok(ClientResponse::Record(self.data_plane.get(&space, &key)?))
            }
            ClientRequest::Delete { space, key } => {
                Ok(match self.data_plane.delete(&space, &key)? {
                    WriteResult::Written | WriteResult::Missing => ClientResponse::Unit,
                    WriteResult::ConditionFailed => ClientResponse::ConditionFailed,
                })
            }
            ClientRequest::ConditionalPut {
                space,
                key,
                checks,
                mutations,
            } => Ok(
                match self
                    .data_plane
                    .conditional_put(&space, key, &checks, &mutations)?
                {
                    WriteResult::Written => ClientResponse::Unit,
                    WriteResult::ConditionFailed => ClientResponse::ConditionFailed,
                    WriteResult::Missing => ClientResponse::Unit,
                },
            ),
            ClientRequest::Search { space, checks } => Ok(ClientResponse::SearchResult(
                self.data_plane.search(&space, &checks)?,
            )),
            ClientRequest::Count { space, checks } => Ok(ClientResponse::Count(
                self.data_plane.count(&space, &checks)?,
            )),
            ClientRequest::DeleteGroup { space, checks } => Ok(ClientResponse::Deleted(
                self.data_plane.delete_matching(&space, &checks)?,
            )),
        }
    }
}

pub fn bootstrap_runtime() -> ClusterRuntime {
    ClusterRuntime::single_node(ClusterConfig::default())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessMode {
    Coordinator {
        data_dir: String,
        listen_host: String,
        listen_port: u16,
    },
    Daemon {
        threads: usize,
        data_dir: String,
        listen_host: String,
        listen_port: u16,
        coordinator_host: String,
        coordinator_port: u16,
    },
}

pub fn parse_process_mode(args: &[String]) -> Result<ProcessMode> {
    let Some(mode) = args.first() else {
        return Err(anyhow!("expected `coordinator` or `daemon` subcommand"));
    };

    match mode.as_str() {
        "coordinator" => Ok(ProcessMode::Coordinator {
            data_dir: required_option(args, "--data")?,
            listen_host: required_option(args, "--listen")?,
            listen_port: required_option(args, "--listen-port")?.parse()?,
        }),
        "daemon" => Ok(ProcessMode::Daemon {
            threads: required_option(args, "--threads")?.parse()?,
            data_dir: required_option(args, "--data")?,
            listen_host: required_option(args, "--listen")?,
            listen_port: required_option(args, "--listen-port")?.parse()?,
            coordinator_host: required_option(args, "--coordinator")?,
            coordinator_port: required_option(args, "--coordinator-port")?.parse()?,
        }),
        other => Err(anyhow!("unknown subcommand `{other}`")),
    }
}

fn required_option(args: &[String], name: &str) -> Result<String> {
    for arg in args {
        if let Some(value) = arg.strip_prefix(&format!("{name}=")) {
            return Ok(value.to_owned());
        }
    }

    Err(anyhow!("missing required option `{name}=...`"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use data_model::{Attribute, Check, Mutation, Predicate, Value};
    use hyperdex_admin_protocol::HyperdexAdminService;
    use hyperdex_client_protocol::HyperdexClientService;

    #[tokio::test]
    async fn runtime_accepts_hyperdex_dsl_schema() {
        let runtime = bootstrap_runtime();

        let response = HyperdexAdminService::handle(
            &runtime,
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();

        assert_eq!(response, AdminResponse::Unit);
        assert_eq!(
            HyperdexAdminService::handle(&runtime, AdminRequest::ListSpaces)
                .await
                .unwrap(),
            AdminResponse::Spaces(vec!["profiles".to_owned()])
        );
    }

    #[tokio::test]
    async fn runtime_supports_put_get_count_and_delete_group() {
        let runtime = bootstrap_runtime();
        HyperdexAdminService::handle(
            &runtime,
            AdminRequest::CreateSpaceDsl(
                "space profiles\n\
                 key username\n\
                 attributes\n\
                    string first,\n\
                    int profile_views\n\
                 tolerate 0 failures\n"
                    .to_owned(),
            ),
        )
        .await
        .unwrap();

        HyperdexClientService::handle(
            &runtime,
            ClientRequest::Put {
                space: "profiles".to_owned(),
                key: Bytes::from_static(b"ada"),
                mutations: vec![
                    Mutation::Set(Attribute {
                        name: "username".to_owned(),
                        value: Value::Bytes(Bytes::from_static(b"ada")),
                    }),
                    Mutation::Set(Attribute {
                        name: "first".to_owned(),
                        value: Value::String("Ada".to_owned()),
                    }),
                    Mutation::Set(Attribute {
                        name: "profile_views".to_owned(),
                        value: Value::Int(5),
                    }),
                ],
            },
        )
        .await
        .unwrap();

        let record = HyperdexClientService::handle(
            &runtime,
            ClientRequest::Get {
                space: "profiles".to_owned(),
                key: Bytes::from_static(b"ada"),
            },
        )
        .await
        .unwrap();
        assert!(matches!(record, ClientResponse::Record(Some(_))));

        let count = HyperdexClientService::handle(
            &runtime,
            ClientRequest::Count {
                space: "profiles".to_owned(),
                checks: vec![Check {
                    attribute: "profile_views".to_owned(),
                    predicate: Predicate::GreaterThanOrEqual,
                    value: Value::Int(5),
                }],
            },
        )
        .await
        .unwrap();
        assert_eq!(count, ClientResponse::Count(1));

        let deleted = HyperdexClientService::handle(
            &runtime,
            ClientRequest::DeleteGroup {
                space: "profiles".to_owned(),
                checks: vec![Check {
                    attribute: "first".to_owned(),
                    predicate: Predicate::Equal,
                    value: Value::String("Ada".to_owned()),
                }],
            },
        )
        .await
        .unwrap();
        assert_eq!(deleted, ClientResponse::Deleted(1));
    }

    #[test]
    fn parse_coordinator_cli() {
        let args = vec![
            "coordinator".to_owned(),
            "--foreground".to_owned(),
            "--data=/tmp/coordinator".to_owned(),
            "--listen=127.0.0.1".to_owned(),
            "--listen-port=1982".to_owned(),
        ];

        assert_eq!(
            parse_process_mode(&args).unwrap(),
            ProcessMode::Coordinator {
                data_dir: "/tmp/coordinator".to_owned(),
                listen_host: "127.0.0.1".to_owned(),
                listen_port: 1982,
            }
        );
    }

    #[test]
    fn parse_daemon_cli() {
        let args = vec![
            "daemon".to_owned(),
            "--foreground".to_owned(),
            "--threads=1".to_owned(),
            "--data=/tmp/daemon".to_owned(),
            "--listen=127.0.0.1".to_owned(),
            "--listen-port=2012".to_owned(),
            "--coordinator=127.0.0.1".to_owned(),
            "--coordinator-port=1982".to_owned(),
        ];

        assert_eq!(
            parse_process_mode(&args).unwrap(),
            ProcessMode::Daemon {
                threads: 1,
                data_dir: "/tmp/daemon".to_owned(),
                listen_host: "127.0.0.1".to_owned(),
                listen_port: 2012,
                coordinator_host: "127.0.0.1".to_owned(),
                coordinator_port: 1982,
            }
        );
    }
}
