use mcp_cli::run::terminal_notice;

#[test]
fn terminal_notice_names_the_server_and_version_and_how_to_stop_it() {
    assert_eq!(
        terminal_notice("project-mcp", "1.0.1"),
        "project-mcp@1.0.1 running on stdio, waiting for an MCP client (Ctrl-C to stop)"
    );
}
