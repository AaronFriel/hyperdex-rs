use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use hyperdex_admin_protocol::{CoordinatorAdminRequest, CoordinatorReturnCode};
use legacy_frontend::LegacyFrontend;
use server::{
    coordinator_cluster_config, daemon_cluster_config, daemon_registration_node,
    handle_coordinator_control_method, handle_legacy_request, parse_process_mode,
    request_coordinator_control_once, ClusterRuntime, CoordinatorControlService, ProcessMode,
};
use tracing::info;

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

            let control_service = CoordinatorControlService::bind(
                format!("{listen_host}:{listen_port}")
                    .parse()
                    .expect("validated socket address"),
            )
            .await?;

            info!(
                address = %control_service.local_addr()?,
                "coordinator control service listening"
            );

            tokio::select! {
                result = control_service.serve_forever_with(move |method, request| {
                    let runtime = runtime.clone();
                    async move { handle_coordinator_control_method(runtime.as_ref(), &method, request).await }
                }) => result?,
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
            let status = request_coordinator_control_once(
                format!("{coordinator_host}:{coordinator_port}")
                    .parse()
                    .expect("validated socket address"),
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

            let runtime = Arc::new(server::ClusterRuntime::single_node_with_data_dir(
                daemon_config,
                Some(Path::new(&data_dir)),
            )?);
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

            let legacy_frontend = LegacyFrontend::bind(
                format!("{listen_host}:{listen_port}")
                    .parse()
                    .expect("validated socket address"),
            )
            .await?;

            info!(
                address = %legacy_frontend.local_addr()?,
                "legacy HyperDex frontend listening"
            );

            tokio::select! {
                result = legacy_frontend.serve_forever_with(move |header, body| {
                    let runtime = runtime.clone();
                    async move { handle_legacy_request(runtime.as_ref(), header, &body).await }
                }) => result?,
                _ = tokio::signal::ctrl_c() => {}
            }
        }
    }

    Ok(())
}
