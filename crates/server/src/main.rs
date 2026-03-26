use anyhow::Result;
use legacy_frontend::LegacyFrontend;
use server::{bootstrap_runtime, parse_process_mode, ProcessMode};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mode = parse_process_mode(&args)?;
    let _runtime = bootstrap_runtime();

    match mode {
        ProcessMode::Coordinator {
            data_dir,
            listen_host,
            listen_port,
        } => {
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
        } => {
            info!(
                threads,
                data_dir,
                listen_host,
                listen_port,
                coordinator_host,
                coordinator_port,
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
                result = legacy_frontend.serve_forever() => result?,
                _ = tokio::signal::ctrl_c() => {}
            }
        }
    }

    Ok(())
}
