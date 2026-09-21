use std::process::Command;

fn mcpctl(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(args)
        .output()
        .expect("mcpctl binary should run")
}

#[test]
fn subcommand_help_is_generated_by_clap() {
    let output = mcpctl(&["install", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help output should be UTF-8");
    assert!(stdout.contains("Usage:"), "subcommand help should have a usage banner");
    assert!(stdout.contains("install"), "subcommand help should name the command");
}
