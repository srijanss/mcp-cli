use std::{fs, path::{Path, PathBuf}, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

use mcp_cli::registry::load_registry;

fn temporary_dir() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-sync-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

/// Writes a binary-runtime MCP source at `workspace/dir` whose entrypoint prints `name@version`.
fn write_package(workspace: &Path, dir: &str, name: &str, version: &str) {
    let source = workspace.join(dir);
    fs::create_dir_all(source.join("bin")).unwrap();
    fs::write(
        source.join("mcpctl.toml"),
        format!("name = \"{name}\"\nversion = \"{version}\"\n\n[runtime]\ntype = \"binary\"\n\n[install]\nentrypoint = \"bin/{name}\"\n"),
    )
    .unwrap();
    fs::write(source.join("bin").join(name), format!("#!/bin/sh\nprintf '{name}@{version}'\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(source.join("bin").join(name), fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// A `checkout-service/` project beside `project-mcp/` (0.4.3) and `design-advisor-mcp/` (0.2.0) sources.
fn workspace_with_project() -> (PathBuf, PathBuf) {
    let workspace = temporary_dir();
    write_package(&workspace, "project-mcp", "project-mcp", "0.4.3");
    write_package(&workspace, "design-advisor-mcp", "design-advisor-mcp", "0.2.0");
    let project = workspace.join("checkout-service");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join(".mcpctl.toml"),
        r#"[project]
name = "checkout-service"

[[mcp]]
name = "project-mcp"
version = "^0.4"
source = "../project-mcp"

[[mcp]]
name = "design-advisor-mcp"
version = "^0.2"
source = "../design-advisor-mcp"
"#,
    )
    .unwrap();
    (workspace, project)
}

fn mcpctl(directory: &Path, state_home: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(arguments)
        .current_dir(directory)
        .env("MCPCTL_HOME", state_home)
        .output()
        .unwrap()
}

#[test]
fn sync_locks_and_installs_every_project_mcp_without_touching_anything_else() {
    let (workspace, project) = workspace_with_project();
    let state_home = workspace.join("state");
    write_package(&workspace, "other-mcp", "other-mcp", "1.0.0");
    assert!(mcpctl(&workspace, &state_home, &["install", "other-mcp"]).status.success());
    fs::create_dir_all(project.join(".venv")).unwrap();
    fs::write(project.join(".venv/marker"), "project venv").unwrap();
    let nested = project.join("src/handlers");
    fs::create_dir_all(&nested).unwrap();

    let outside = mcpctl(&workspace, &state_home, &["sync"]);
    let synced = mcpctl(&nested, &state_home, &["sync"]);

    assert!(!outside.status.success());
    assert!(String::from_utf8_lossy(&outside.stderr).contains(".mcpctl.toml"), "{}", String::from_utf8_lossy(&outside.stderr));
    assert!(synced.status.success(), "{}", String::from_utf8_lossy(&synced.stderr));
    let lock = mcp_cli::project_lock::parse_project_lock(&fs::read_to_string(project.join(".mcpctl.lock")).unwrap()).unwrap();
    let locked: Vec<_> = lock.mcp.iter().map(|mcp| (mcp.name.as_str(), mcp.version.as_str())).collect();
    assert_eq!(locked, [("design-advisor-mcp", "0.2.0"), ("project-mcp", "0.4.3")]);
    assert_eq!(mcpctl(&nested, &state_home, &["run", "project-mcp"]).stdout, b"project-mcp@0.4.3");
    assert_eq!(mcpctl(&nested, &state_home, &["run", "design-advisor-mcp"]).stdout, b"design-advisor-mcp@0.2.0");
    let registry = load_registry(&state_home.join("registry.json")).unwrap();
    assert_eq!(registry.packages["other-mcp"].active_version, Some("1.0.0".to_owned()));
    assert_eq!(registry.packages["other-mcp"].versions.len(), 1);
    assert_eq!(fs::read_to_string(project.join(".venv/marker")).unwrap(), "project venv");
    assert_eq!(fs::read_dir(project.join(".venv")).unwrap().count(), 1);
}

#[test]
fn repeated_sync_reuses_installed_versions_and_projects_share_them() {
    let (workspace, project) = workspace_with_project();
    let state_home = workspace.join("state");
    let second_project = workspace.join("billing-service");
    fs::create_dir_all(&second_project).unwrap();
    fs::write(
        second_project.join(".mcpctl.toml"),
        "[project]\nname = \"billing-service\"\n\n[[mcp]]\nname = \"project-mcp\"\nversion = \"^0.4\"\nsource = \"../project-mcp\"\n",
    )
    .unwrap();
    assert!(mcpctl(&project, &state_home, &["sync"]).status.success());
    let registry_before = fs::read(state_home.join("registry.json")).unwrap();
    let lock_modified = fs::metadata(project.join(".mcpctl.lock")).unwrap().modified().unwrap();

    let again = mcpctl(&project, &state_home, &["sync"]);
    let shared = mcpctl(&second_project, &state_home, &["sync"]);

    assert!(again.status.success(), "{}", String::from_utf8_lossy(&again.stderr));
    assert!(shared.status.success(), "{}", String::from_utf8_lossy(&shared.stderr));
    assert_eq!(fs::read(state_home.join("registry.json")).unwrap(), registry_before);
    assert_eq!(fs::metadata(project.join(".mcpctl.lock")).unwrap().modified().unwrap(), lock_modified);
    assert_eq!(fs::read_dir(state_home.join("packages/project-mcp")).unwrap().count(), 1);
    assert_eq!(mcpctl(&second_project, &state_home, &["run", "project-mcp"]).stdout, b"project-mcp@0.4.3");
}

#[test]
fn sync_fails_naming_a_reused_version_whose_runtime_is_broken() {
    let (workspace, project) = workspace_with_project();
    let state_home = workspace.join("state");
    assert!(mcpctl(&project, &state_home, &["sync"]).status.success());
    fs::remove_file(state_home.join("packages/project-mcp/0.4.3/runtime/bin/bin/project-mcp")).unwrap();

    let output = mcpctl(&project, &state_home, &["sync"]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("project-mcp@0.4.3"), "{stderr}");
    assert!(stderr.contains("entrypoint"), "{stderr}");
}

#[test]
fn locked_sync_on_a_fresh_store_installs_the_committed_lock_without_rewriting_it() {
    let (workspace, project) = workspace_with_project();
    assert!(mcpctl(&project, &workspace.join("first-machine"), &["sync"]).status.success());
    let committed_lock = fs::read(project.join(".mcpctl.lock")).unwrap();
    let lock_modified = fs::metadata(project.join(".mcpctl.lock")).unwrap().modified().unwrap();
    let fresh_state = workspace.join("ci-machine");

    let output = mcpctl(&project, &fresh_state, &["sync", "--locked"]);

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read(project.join(".mcpctl.lock")).unwrap(), committed_lock);
    assert_eq!(fs::metadata(project.join(".mcpctl.lock")).unwrap().modified().unwrap(), lock_modified);
    assert_eq!(mcpctl(&project, &fresh_state, &["run", "project-mcp"]).stdout, b"project-mcp@0.4.3");
    assert_eq!(mcpctl(&project, &fresh_state, &["run", "design-advisor-mcp"]).stdout, b"design-advisor-mcp@0.2.0");
}
