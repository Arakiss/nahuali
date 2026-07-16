use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;
use nahuali_api::{ApiConfig, router};

#[derive(Debug, Parser)]
#[command(
    name = "nahuali-api",
    version,
    about = "Run the Nahuali HTTP API server."
)]
struct Args {
    #[arg(long)]
    database: Option<PathBuf>,
    #[arg(long, default_value = "127.0.0.1:7070")]
    listen: SocketAddr,
    #[arg(
        long,
        help = "Require the configured semantic index to be current for /v1/ready."
    )]
    require_semantic: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let flag = args.database.as_deref().and_then(std::path::Path::to_str);
    let database = nahuali_core::resolve_database_name(flag)?;
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    axum::serve(
        listener,
        router(ApiConfig::new(database.value).require_semantic(args.require_semantic)),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
