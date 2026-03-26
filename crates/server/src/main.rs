use anyhow::Result;
use server::bootstrap_runtime;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let _runtime = bootstrap_runtime();

    info!("hyperdex-rs server bootstrapped");
    Ok(())
}
