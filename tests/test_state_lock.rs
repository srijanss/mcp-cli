use mcp_cli::lock::StateLock;

fn state_home(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("mcpctl-lock-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    path
}

#[test]
fn exclusive_lock_is_refused_while_held_and_released_on_drop() {
    let home = state_home("exclusive");

    let held = StateLock::exclusive(&home).expect("first lock should succeed");
    let error = StateLock::exclusive(&home).err().expect("second lock should be refused");
    assert!(error.contains("locked"), "error was: {error}");

    drop(held);
    StateLock::exclusive(&home).expect("lock should be free after drop");

    std::fs::remove_dir_all(home).unwrap();
}
