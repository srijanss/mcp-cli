use std::process::Command;

#[test]
fn unknown_subcommand_gets_clap_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .arg("bogus")
        .output()
        .expect("mcpctl binary should run");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("unrecognized subcommand"), "stderr was: {stderr}");
}
