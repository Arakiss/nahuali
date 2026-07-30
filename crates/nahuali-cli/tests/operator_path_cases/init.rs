#[test]
fn init_mcp_config_uses_the_selected_database() {
    let database = temp_database("init-selected-database");
    let output = run_ok(&database, &["init", "--dry-run"]);
    let selected = database.to_str().expect("database name is UTF-8");

    assert!(output.contains("\"--database\""));
    assert!(
        output.contains(&format!("\"{selected}\"")),
        "init output did not contain selected database {selected}:\n{output}"
    );
}
