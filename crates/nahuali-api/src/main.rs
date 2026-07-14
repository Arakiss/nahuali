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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let flag = args.database.as_deref().and_then(std::path::Path::to_str);
    let database = nahuali_core::resolve_database_name(flag)?;
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    axum::serve(listener, router(ApiConfig::new(database.value))).await?;
    Ok(())
}
