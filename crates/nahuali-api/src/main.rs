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
    #[arg(long, default_value = "memory")]
    database: PathBuf,
    #[arg(long, default_value = "127.0.0.1:7070")]
    listen: SocketAddr,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    axum::serve(listener, router(ApiConfig::new(args.database))).await?;
    Ok(())
}
