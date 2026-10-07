use std::{fs, path::PathBuf, sync::atomic::{AtomicUsize, Ordering}};

use mcp_cli::{metadata::sha256_hex, project::McpDeclaration, project_lock::resolve_locked_mcp};

fn temporary_dir() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-project-lock-resolve-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

/// A workspace with `checkout-service/` (the project root) beside a `project-mcp/` package.
fn workspace_with_package(package_name: &str, package_version: &str) -> (PathBuf, String) {
    let workspace = temporary_dir();
    fs::create_dir_all(workspace.join("checkout-service")).unwrap();
    fs::create_dir_all(workspace.join("project-mcp")).unwrap();
    let manifest = format!(
        "name = \"{package_name}\"\nversion = \"{package_version}\"\n\n[runtime]\ntype = \"binary\"\n\n[install]\nentrypoint = \"bin/project-mcp\"\n"
    );
    fs::write(workspace.join("project-mcp/mcpctl.toml"), &manifest).unwrap();
    (workspace.join("checkout-service"), manifest)
}

fn declaration(version: &str) -> McpDeclaration {
    McpDeclaration { name: "project-mcp".to_owned(), version: version.to_owned(), source: "../project-mcp".to_owned() }
}

#[test]
fn local_source_resolves_to_exact_version_portable_source_and_manifest_digest() {
    let (root, manifest) = workspace_with_package("project-mcp", "0.4.3");

    let locked = resolve_locked_mcp(&declaration("^0.4"), &root).unwrap();

    assert_eq!(locked.name, "project-mcp");
    assert_eq!(locked.version, "0.4.3");
    assert_eq!(locked.source, "../project-mcp");
    assert_eq!(locked.manifest_digest, format!("sha256:{}", sha256_hex(manifest.as_bytes())));
}

#[test]
fn local_source_version_outside_constraint_fails_naming_both_versions() {
    let (root, _) = workspace_with_package("project-mcp", "0.5.0");

    let error = resolve_locked_mcp(&declaration("^0.4"), &root).unwrap_err();

    assert!(error.contains("project-mcp"), "{error}");
    assert!(error.contains("0.5.0"), "{error}");
    assert!(error.contains("^0.4"), "{error}");
}

#[test]
fn local_source_with_different_package_name_fails_naming_both() {
    let (root, _) = workspace_with_package("other-mcp", "0.4.3");

    let error = resolve_locked_mcp(&declaration("^0.4"), &root).unwrap_err();

    assert!(error.contains("project-mcp"), "{error}");
    assert!(error.contains("other-mcp"), "{error}");
}
