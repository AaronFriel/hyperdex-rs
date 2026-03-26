use std::sync::Arc;

use anyhow::Result;
use cluster_config::ClusterConfig;
use control_plane::InMemoryCatalog;
use data_plane::DataPlane;
use engine_memory::MemoryEngine;
use placement_core::HyperSpacePlacement;
use storage_core::StorageEngine;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = ClusterConfig::default();
    let storage: Arc<dyn StorageEngine> = Arc::new(MemoryEngine::new());
    let catalog = Arc::new(InMemoryCatalog::new(config.nodes.clone(), config.replicas));
    let _data_plane = DataPlane::new(catalog, storage, Arc::new(HyperSpacePlacement));

    info!("hyperdex-rs server bootstrapped");
    Ok(())
}
