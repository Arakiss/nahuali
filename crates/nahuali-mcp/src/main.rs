use std::path::PathBuf;

use clap::Parser;
use nahuali_mcp::NahualiMcpServer;
use rmcp::{ServiceExt, transport::stdio};

#[derive(Debug, Parser)]
#[command(name = "nahuali-mcp")]
#[command(version)]
#[command(about = "Nahuali MCP stdio server")]
struct Args {
    #[arg(long = "database", value_name = "NAME")]
    database: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // Same single resolver the CLI uses: --database beats NAHUALI_DB_DATABASE
    // beats the default, and a path-like name is refused, not silently mangled.
    let flag = args.database.as_deref().and_then(std::path::Path::to_str);
    let resolved = nahuali_core::resolve_database_name(flag)?;
    let service = NahualiMcpServer::open(PathBuf::from(resolved.value))?
        .serve(stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
