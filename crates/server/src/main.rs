extern crate prost;

use bytes::Bytes;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use hyperdex_admin_protocol::{CoordinatorAdminRequest, CoordinatorReturnCode};
use legacy_frontend::LegacyFrontend;
use server::{
    coordinator_cluster_config, daemon_cluster_config, daemon_registration_node,
    handle_legacy_request, parse_process_mode, request_coordinator_control_once,
    serve_coordinator_public_connection, sync_runtime_with_coordinator, ClusterRuntime,
    ProcessMode, TransportRuntime,
};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tracing::{info, warn};
use transport_core::{ClusterTransport, InternodeRequest, InternodeResponse, RemoteNode};

pub mod grpc_api {
    pub mod v1 {
        tonic::include_proto!("hyperdex.v1");
    }
}

#[derive(Clone)]
struct ProcessInternodeGrpc {
    runtime: Arc<ClusterRuntime>,
}

impl ProcessInternodeGrpc {
    fn new(runtime: Arc<ClusterRuntime>) -> Self {
        Self { runtime }
    }
}

#[tonic::async_trait]
impl grpc_api::v1::internode_transport_server::InternodeTransport for ProcessInternodeGrpc {
    async fn send(
        &self,
        request: tonic::Request<grpc_api::v1::InternodeRpcRequest>,
    ) -> std::result::Result<tonic::Response<grpc_api::v1::InternodeRpcResponse>, tonic::Status>
    {
        let request = request.into_inner();
        let response = self
            .runtime
            .handle_internode_request(InternodeRequest {
                method: request.method,
                body: Bytes::from(request.body),
            })
            .await
            .map_err(|err| tonic::Status::internal(err.to_string()))?;

        Ok(tonic::Response::new(grpc_api::v1::InternodeRpcResponse {
            status: response.status as u32,
            body: response.body.to_vec(),
        }))
    }
}

#[derive(Default)]
struct ProcessGrpcTransportAdapter;

#[async_trait]
impl ClusterTransport for ProcessGrpcTransportAdapter {
    async fn send(
        &self,
        node: &RemoteNode,
        request: InternodeRequest,
    ) -> Result<InternodeResponse> {
        let endpoint = format!("http://{}:{}", node.host, node.port);
        let mut client =
            grpc_api::v1::internode_transport_client::InternodeTransportClient::connect(endpoint)
                .await?;
        let response = client
            .send(grpc_api::v1::InternodeRpcRequest {
                method: request.method,
                body: request.body.to_vec(),
            })
            .await?
            .into_inner();

        Ok(InternodeResponse {
            status: response.status as u16,
            body: Bytes::from(response.body),
        })
    }

    fn name(&self) -> &'static str {
        "grpc-process"
    }
}

async fn serve_internode_grpc(
    runtime: Arc<ClusterRuntime>,
    address: std::net::SocketAddr,
    shutdown_rx: oneshot::Receiver<()>,
) -> Result<()> {
    let listener = TcpListener::bind(address).await?;
    info!(address = %listener.local_addr()?, "daemon internode gRPC listening");

    Server::builder()
        .add_service(
            grpc_api::v1::internode_transport_server::InternodeTransportServer::new(
                ProcessInternodeGrpc::new(runtime),
            ),
        )
        .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
            let _ = shutdown_rx.await;
        })
        .await?;

    Ok(())
}

fn legacy_request_needs_config_refresh(err: &anyhow::Error) -> bool {
    let message = err.to_string();
    message.contains("legacy request handling requires one created space")
        || message.contains("catalog lost space definition")
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mode = parse_process_mode(&args)?;

    match mode {
        ProcessMode::Coordinator {
            data_dir,
            listen_host,
            listen_port,
        } => {
            let runtime = Arc::new(ClusterRuntime::single_node_with_data_dir(
                coordinator_cluster_config(),
                Some(Path::new(&data_dir)),
            )?);
            info!(
                data_dir,
                listen_host, listen_port, "hyperdex-rs coordinator bootstrapped"
            );

            let listener = TcpListener::bind(
                format!("{listen_host}:{listen_port}")
                    .parse::<std::net::SocketAddr>()
                    .expect("validated socket address"),
            )
            .await?;

            info!(
                address = %listener.local_addr()?,
                "coordinator public service listening"
            );

            tokio::select! {
                result = async {
                    loop {
                        let (stream, _) = listener.accept().await?;
                        let runtime = runtime.clone();
                        tokio::spawn(async move {
                            if let Err(err) = serve_coordinator_public_connection(stream, runtime).await {
                                warn!(error = %err, "coordinator public connection failed");
                            }
                        });
                    }
                    #[allow(unreachable_code)]
                    Ok::<(), anyhow::Error>(())
                } => result?,
                _ = tokio::signal::ctrl_c() => {}
            }
        }
        ProcessMode::Daemon {
            node_id,
            threads,
            data_dir,
            listen_host,
            listen_port,
            control_port,
            coordinator_host,
            coordinator_port,
            consensus,
            placement,
            storage,
            internode_transport,
        } => {
            let daemon_mode = ProcessMode::Daemon {
                node_id,
                threads,
                data_dir: data_dir.clone(),
                listen_host: listen_host.clone(),
                listen_port,
                control_port,
                coordinator_host: coordinator_host.clone(),
                coordinator_port,
                consensus: consensus.clone(),
                placement: placement.clone(),
                storage: storage.clone(),
                internode_transport: internode_transport.clone(),
            };
            let daemon_config = daemon_cluster_config(&daemon_mode);
            let daemon_node =
                daemon_registration_node(&daemon_mode).expect("daemon mode has a node identity");
            let coordinator_address = format!("{coordinator_host}:{coordinator_port}")
                .parse()
                .expect("validated socket address");
            let status = request_coordinator_control_once(
                coordinator_address,
                "daemon_register",
                &CoordinatorAdminRequest::DaemonRegister(daemon_node.clone()),
            )
            .await?;
            let status = CoordinatorReturnCode::decode(&status)?;
            if status != CoordinatorReturnCode::Success {
                anyhow::bail!(
                    "coordinator rejected daemon registration for node {} with {:?}",
                    daemon_node.id,
                    status
                );
            }

            let mut runtime = server::ClusterRuntime::single_node_with_data_dir(
                daemon_config,
                Some(Path::new(&data_dir)),
            )?;
            if internode_transport == cluster_config::TransportBackend::Grpc {
                runtime.install_cluster_transport(
                    Arc::new(ProcessGrpcTransportAdapter),
                    TransportRuntime::Grpc,
                );
            }
            let runtime = Arc::new(runtime);
            info!(
                threads,
                data_dir,
                listen_host,
                listen_port,
                control_port,
                coordinator_host,
                coordinator_port,
                consensus = ?consensus,
                placement = ?placement,
                storage = ?storage,
                internode_transport = ?internode_transport,
                "hyperdex-rs daemon bootstrapped"
            );

            match sync_runtime_with_coordinator(runtime.as_ref(), coordinator_address).await {
                Ok(view) => info!(
                    version = view.version,
                    stable_through = view.stable_through,
                    spaces = view.spaces.len(),
                    "daemon synchronized coordinator config"
                ),
                Err(err) => warn!(
                    error = %err,
                    "daemon could not fetch coordinator config during startup"
                ),
            }

            let mut grpc_shutdown_tx = None;
            let mut grpc_task = None;
            if internode_transport == cluster_config::TransportBackend::Grpc {
                let (shutdown_tx, shutdown_rx) = oneshot::channel();
                let grpc_runtime = runtime.clone();
                let grpc_address = format!("{listen_host}:{control_port}")
                    .parse()
                    .expect("validated socket address");
                grpc_shutdown_tx = Some(shutdown_tx);
                grpc_task = Some(tokio::spawn(async move {
                    serve_internode_grpc(grpc_runtime, grpc_address, shutdown_rx).await
                }));
            }

            let legacy_frontend = LegacyFrontend::bind_with_server_id(
                format!("{listen_host}:{listen_port}")
                    .parse()
                    .expect("validated socket address"),
                node_id,
            )
            .await?;

            info!(
                address = %legacy_frontend.local_addr()?,
                "legacy HyperDex frontend listening"
            );

            let sync_runtime = runtime.clone();
            let sync_task = tokio::spawn(async move {
                let mut last_synced_version = None;
                let mut interval = tokio::time::interval(Duration::from_millis(50));

                loop {
                    interval.tick().await;
                    match sync_runtime_with_coordinator(sync_runtime.as_ref(), coordinator_address)
                        .await
                    {
                        Ok(view) => {
                            if last_synced_version != Some(view.version) {
                                info!(
                                    version = view.version,
                                    stable_through = view.stable_through,
                                    spaces = view.spaces.len(),
                                    "daemon synchronized coordinator config"
                                );
                                last_synced_version = Some(view.version);
                            }
                        }
                        Err(err) => warn!(
                            error = %err,
                            "daemon failed to refresh coordinator config"
                        ),
                    }
                }
            });

            tokio::select! {
                result = legacy_frontend.serve_forever_with(move |header, body| {
                    let runtime = runtime.clone();
                    async move {
                        match handle_legacy_request(runtime.as_ref(), header, &body).await {
                            Ok(response) => Ok(response),
                            Err(err) if legacy_request_needs_config_refresh(&err) => {
                                sync_runtime_with_coordinator(runtime.as_ref(), coordinator_address)
                                    .await?;
                                handle_legacy_request(runtime.as_ref(), header, &body).await
                            }
                            Err(err) => Err(err),
                        }
                    }
                }) => result?,
                _ = tokio::signal::ctrl_c() => {}
            }

            if let Some(shutdown_tx) = grpc_shutdown_tx.take() {
                let _ = shutdown_tx.send(());
            }
            if let Some(task) = grpc_task.take() {
                task.await??;
            }
            sync_task.abort();
            let _ = sync_task.await;
        }
    }

    Ok(())
}
