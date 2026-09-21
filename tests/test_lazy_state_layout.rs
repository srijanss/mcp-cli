use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use mcp_cli::paths::{data_home_from, ensure_state_layout};

fn temporary_state_home() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    std::env::temp_dir().join(format!(
        "mcpctl-state-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn state_layout_is_created_only_when_explicitly_requested() {
    let state_home = temporary_state_home();
    let _ = fs::remove_dir_all(&state_home);

    assert_eq!(data_home_from(Some(state_home.clone())).unwrap(), state_home);
    assert!(!state_home.exists());

    ensure_state_layout(&state_home).unwrap();

    assert!(state_home.join("registry.json").is_file());
    for directory in ["packages", "active", "cache"] {
        assert!(state_home.join(directory).is_dir());
    }

    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn state_initialization_creates_registry_and_directories() {
    let state_home = temporary_state_home();
    let _ = fs::remove_dir_all(&state_home);

    ensure_state_layout(&state_home).unwrap();

    assert!(state_home.join("registry.json").is_file());
    assert!(state_home.join("packages").is_dir());

    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn state_initialization_writes_loadable_empty_registry() {
    let state_home = temporary_state_home();
    let _ = fs::remove_dir_all(&state_home);

    ensure_state_layout(&state_home).unwrap();

    let registry = mcp_cli::registry::load_registry(&state_home.join("registry.json")).unwrap();
    assert_eq!(registry, mcp_cli::registry::Registry::new());

    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn state_initialization_preserves_existing_registry() {
    let state_home = temporary_state_home();
    let _ = fs::remove_dir_all(&state_home);
    fs::create_dir_all(&state_home).unwrap();
    let existing = r#"{"schema_version":1,"packages":{"x":{"active_version":null,"versions":[]}}}"#;
    fs::write(state_home.join("registry.json"), existing).unwrap();

    ensure_state_layout(&state_home).unwrap();

    assert_eq!(fs::read_to_string(state_home.join("registry.json")).unwrap(), existing);

    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn state_initialization_rejects_registry_directory() {
    let state_home = temporary_state_home();
    let _ = fs::remove_dir_all(&state_home);
    fs::create_dir_all(state_home.join("registry.json")).unwrap();

    assert!(ensure_state_layout(&state_home).is_err());

    fs::remove_dir_all(state_home).unwrap();
}

#[test]
fn state_initialization_rejects_registry_directory_after_directories_exist() {
    let state_home = temporary_state_home();
    let _ = fs::remove_dir_all(&state_home);
    ensure_state_layout(&state_home).unwrap();
    fs::remove_file(state_home.join("registry.json")).unwrap();
    fs::create_dir(state_home.join("registry.json")).unwrap();

    assert!(ensure_state_layout(&state_home).is_err());

    fs::remove_dir_all(state_home).unwrap();
}
