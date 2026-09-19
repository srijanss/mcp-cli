use std::process::Command;

#[test]
fn help_lists_initial_command_surface() {
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .arg("--help")
        .output()
        .expect("mcpctl binary should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help output should be UTF-8");

    for command in [
        "install", "run", "list", "info", "uninstall", "use", "update", "doctor",
    ] {
        assert!(stdout.contains(command), "help should list {command}");
    }
}

#[test]
fn help_has_usage_banner() {
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .arg("--help")
        .output()
        .expect("mcpctl binary should run");

    assert!(String::from_utf8(output.stdout)
        .expect("help output should be UTF-8")
        .contains("Usage:"));
}

#[test]
fn invalid_cli_usage_returns_an_error() {
    for arguments in [Vec::<&str>::new(), vec!["--bogus"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
            .args(arguments)
            .output()
            .expect("mcpctl binary should run");

        assert!(!output.status.success());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn invalid_cli_usage_writes_to_stderr() {
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .arg("--bogus")
        .output()
        .expect("mcpctl binary should run");

    assert!(!output.stderr.is_empty());
}
