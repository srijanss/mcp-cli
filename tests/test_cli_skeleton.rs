use std::process::Command;

#[test]
fn binary_supports_version_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .arg("--version")
        .output()
        .expect("mcpctl binary should run");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("version output should be UTF-8"),
        "mcpctl 0.1.0\n"
    );
}
