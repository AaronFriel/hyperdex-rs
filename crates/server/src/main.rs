use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use hyperdex_client_protocol::{ClientRequest, ClientResponse, HyperdexClientService};
use legacy_frontend::LegacyFrontend;
use legacy_protocol::{
    config_mismatch_response, CountRequest, CountResponse, LegacyMessageType, ResponseHeader,
};
use server::{bootstrap_runtime, daemon_cluster_config, parse_process_mode, ProcessMode};
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
            let _runtime = bootstrap_runtime();
            info!(
                data_dir,
                listen_host, listen_port, "hyperdex-rs coordinator bootstrapped"
            );
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
                    async move {
                        match header.message_type {
                            LegacyMessageType::ReqCount => {
                                let request = CountRequest::decode_body(&body)?;
                                let response = HyperdexClientService::handle(
                                    runtime.as_ref(),
                                    ClientRequest::Count {
                                        space: request.space,
                                        checks: Vec::new(),
                                    },
                                )
                                .await?;

                                let ClientResponse::Count(count) = response else {
                                    anyhow::bail!("unexpected runtime response to count request");
                                };

                                Ok((
                                    ResponseHeader {
                                        message_type: LegacyMessageType::RespCount,
                                        target_virtual_server: header.target_virtual_server,
                                        nonce: header.nonce,
                                    },
                                    CountResponse { count }.encode_body().to_vec(),
                                ))
                            }
                            _ => Ok((config_mismatch_response(header), Vec::new())),
                        }
                    }
                }) => result?,
                _ = tokio::signal::ctrl_c() => {}
            }
        }
    }

    Ok(())
}
