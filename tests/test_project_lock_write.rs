use std::{fs, path::PathBuf, sync::atomic::{AtomicUsize, Ordering}};

use mcp_cli::project_lock::{parse_project_lock, write_project_lock, LockedMcp, ProjectLock};

fn temporary_dir() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-project-lock-write-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn lock(version: &str) -> ProjectLock {
    ProjectLock {
        version: 1,
        mcp: vec![LockedMcp {
            name: "project-mcp".to_owned(),
            version: version.to_owned(),
            source: "../project-mcp".to_owned(),
            manifest_digest: "sha256:abc123".to_owned(),
        }],
    }
}

#[test]
fn lock_is_written_once_and_unchanged_inputs_do_not_rewrite_it() {
    let root = temporary_dir();
    let path = root.join(".mcpctl.lock");

    assert!(write_project_lock(&root, &lock("0.4.3")).unwrap());
    let first_modified = fs::metadata(&path).unwrap().modified().unwrap();

    assert!(!write_project_lock(&root, &lock("0.4.3")).unwrap());
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), first_modified);

    assert!(write_project_lock(&root, &lock("0.4.4")).unwrap());
    assert_eq!(parse_project_lock(&fs::read_to_string(&path).unwrap()).unwrap(), lock("0.4.4"));
}

#[test]
fn malformed_existing_lock_fails_and_is_not_replaced() {
    let root = temporary_dir();
    let path = root.join(".mcpctl.lock");
    fs::write(&path, "version = 1\n[[mcp]\nbroken").unwrap();

    let error = write_project_lock(&root, &lock("0.4.3")).unwrap_err();

    assert!(error.contains(".mcpctl.lock"), "{error}");
    assert_eq!(fs::read_to_string(&path).unwrap(), "version = 1\n[[mcp]\nbroken");
}
