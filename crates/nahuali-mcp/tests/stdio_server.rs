mod mcp_stdio;

#[test]
fn stdio_server_supports_current_mcp_tool_workflow() {
    mcp_stdio::workflow::run();
}

#[test]
fn stdio_server_supports_memory_hook() {
    mcp_stdio::hooks::run();
}

#[test]
fn stdio_server_supports_consolidation_plan() {
    mcp_stdio::consolidation_plan::run();
}

#[test]
fn stdio_server_supports_operator_loop_tools() {
    mcp_stdio::operator_loop::run();
}

#[test]
fn stdio_server_supports_scoped_memory_tools() {
    mcp_stdio::scopes::run();
}
