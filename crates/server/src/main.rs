use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use legacy_frontend::LegacyFrontend;
use server::{
    daemon_cluster_config, handle_coordinator_control_method, handle_legacy_request,
    parse_process_mode, ClusterRuntime, CoordinatorControlService, ProcessMode,
};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mode = parse_process_mode(&args)?;
    let daemon_config = daemon_cluster_config(&mode);

    match mode {
        ProcessMode::Coordinator {
            data_dir,
            listen_host,
            listen_port,
        } => {
            let runtime = Arc::new(ClusterRuntime::single_node_with_data_dir(
                cluster_config::ClusterConfig::default(),
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
            threads,
            data_dir,
            listen_host,
            listen_port,
            coordinator_host,
            coordinator_port,
            consensus,
            placement,
            storage,
            internode_transport,
        } => {
            let runtime = Arc::new(server::ClusterRuntime::single_node_with_data_dir(
                daemon_config,
                Some(Path::new(&data_dir)),
            )?);
            info!(
                threads,
                data_dir,
                listen_host,
                listen_port,
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
