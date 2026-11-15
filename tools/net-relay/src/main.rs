//! The `nfs-relay` binary: read the configuration, then serve until a signal.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let config = nfs_relay::config::from_args(std::env::args().skip(1))?;
    nfs_relay::server::run(config).await
}
