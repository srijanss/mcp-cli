use std::process::Command;

#[test]
fn uninstall_fails_fast_while_another_process_holds_the_state_lock() {
    let state_home = std::env::temp_dir().join(format!("mcpctl-main-lock-{}", std::process::id()));
    std::fs::create_dir_all(&state_home).unwrap();
    let lock = std::fs::File::create(state_home.join("mcpctl.lock")).unwrap();
    lock.lock().unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(["uninstall", "example-mcp@1.0.0"])
        .env("MCPCTL_HOME", &state_home)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("locked"), "stderr was: {stderr}");

    drop(lock);
    std::fs::remove_dir_all(state_home).unwrap();
}
