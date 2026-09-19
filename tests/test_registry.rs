use std::{fs, path::PathBuf, sync::atomic::{AtomicUsize, Ordering}};

use mcp_cli::registry::{load_registry, save_registry, InstalledVersion, PackageRecord, Registry};

fn registry_path() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "mcpctl-registry-{}-{}.json",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn registry_persists_and_reloads_installed_package_metadata() {
    let path = registry_path();
    let _ = fs::remove_file(&path);
    let registry = Registry::with_package(
        "project-mcp",
        PackageRecord {
            active_version: Some("0.4.0".to_owned()),
            versions: vec![InstalledVersion {
                version: "0.4.0".to_owned(),
                runtime: "python".to_owned(),
                source: "local".to_owned(),
                installed_at: "2026-09-19T00:00:00Z".to_owned(),
            }],
        },
    );

    save_registry(&path, &registry).unwrap();
    assert_eq!(load_registry(&path).unwrap(), registry);

    fs::remove_file(path).unwrap();
}

#[test]
fn registry_uses_schema_version_one() {
    assert_eq!(Registry::new().schema_version, 1);
}

#[test]
fn corrupt_registry_returns_clear_diagnostic_without_overwrite() {
    let path = registry_path();
    fs::write(&path, "{ invalid json").unwrap();

    let error = load_registry(&path).unwrap_err();
    assert!(error.contains("invalid registry"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "{ invalid json");

    fs::remove_file(path).unwrap();
}

#[test]
fn corrupt_registry_error_identifies_registry_path() {
    let path = registry_path();
    fs::write(&path, "not json").unwrap();

    assert!(load_registry(&path).unwrap_err().contains("invalid registry"));

    fs::remove_file(path).unwrap();
}

#[test]
fn registry_rejects_unsupported_schema_version() {
    let path = registry_path();
    fs::write(&path, r#"{"schema_version":999,"packages":{}}"#).unwrap();

    assert!(load_registry(&path).unwrap_err().contains("unsupported registry schema"));

    fs::remove_file(path).unwrap();
}

#[test]
fn registry_rejects_older_schema_version() {
    let path = registry_path();
    fs::write(&path, r#"{"schema_version":0,"packages":{}}"#).unwrap();

    assert!(load_registry(&path).unwrap_err().contains("unsupported registry schema"));

    fs::remove_file(path).unwrap();
}
