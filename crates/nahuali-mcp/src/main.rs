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
    let database = args.database.unwrap_or_else(default_database_name);
    let service = NahualiMcpServer::open(database)?.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

fn default_database_name() -> PathBuf {
    std::env::var("NAHUALI_DB_DATABASE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("memory"))
}
