use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use hyperdex_admin_protocol::{CoordinatorAdminRequest, CoordinatorReturnCode};
use legacy_frontend::LegacyFrontend;
use server::{
    coordinator_cluster_config, daemon_cluster_config, daemon_registration_node,
    handle_coordinator_control_method, handle_legacy_request, parse_process_mode,
    request_coordinator_control_once, sync_runtime_with_coordinator, ClusterRuntime,
    CoordinatorControlService, ProcessMode,
};
use tracing::{info, warn};

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
                    async move { handle_legacy_request(runtime.as_ref(), header, &body).await }
                }) => result?,
                _ = tokio::signal::ctrl_c() => {}
            }

            sync_task.abort();
            let _ = sync_task.await;
        }
    }

    Ok(())
}
