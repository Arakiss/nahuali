mod catalog;
mod protocol;
mod server;
mod tools;

pub use catalog::{McpPrompt, McpResource, McpTool, SERVER_NAME};
pub use catalog::{prompt_catalog, resource_catalog, tool_catalog};
pub use server::NahualiMcpServer;
