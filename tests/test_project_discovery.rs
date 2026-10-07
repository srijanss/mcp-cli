use std::{fs, path::PathBuf, sync::atomic::{AtomicUsize, Ordering}};

use mcp_cli::project::find_project_root;

fn temporary_dir() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-project-discovery-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn nested_directory_discovers_project_root() {
    let root = temporary_dir();
    fs::write(root.join(".mcpctl.toml"), "").unwrap();
    let nested = root.join("src/foo");
    fs::create_dir_all(&nested).unwrap();

    assert_eq!(find_project_root(&nested), Some(root));
}

#[test]
fn directory_outside_any_project_finds_no_root() {
    let outside = temporary_dir().join("not-a-project/deeper");
    fs::create_dir_all(&outside).unwrap();

    assert_eq!(find_project_root(&outside), None);
}
