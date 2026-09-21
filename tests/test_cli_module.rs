use clap::Parser;
use mcp_cli::cli::{Cli, Command};

#[test]
fn parses_install_with_project_path() {
    let cli = Cli::try_parse_from(["mcpctl", "install", "./project"]).expect("should parse");

    match cli.command {
        Command::Install { project } => assert_eq!(project, "./project"),
        other => panic!("expected install, got {other:?}"),
    }
}
