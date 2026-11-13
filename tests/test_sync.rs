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

#[test]
fn locked_sync_fails_without_installing_when_the_lock_is_missing_or_stale() {
    let (workspace, project) = workspace_with_project();
    let fresh_state = workspace.join("ci-machine");

    let missing = mcpctl(&project, &fresh_state, &["sync", "--locked"]);

    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains(".mcpctl.lock"), "{}", String::from_utf8_lossy(&missing.stderr));
    assert!(!project.join(".mcpctl.lock").exists());
    assert!(!fresh_state.join("packages").exists());

    assert!(mcpctl(&project, &workspace.join("first-machine"), &["sync"]).status.success());
    let committed_lock = fs::read(project.join(".mcpctl.lock")).unwrap();
    write_package(&workspace, "project-mcp", "project-mcp", "0.4.4");

    let stale = mcpctl(&project, &fresh_state, &["sync", "--locked"]);

    let stderr = String::from_utf8_lossy(&stale.stderr);
    assert!(!stale.status.success());
    assert!(stderr.contains("project-mcp"), "{stderr}");
    assert!(stderr.contains("mcpctl sync"), "{stderr}");
    assert_eq!(fs::read(project.join(".mcpctl.lock")).unwrap(), committed_lock);
    assert!(!fresh_state.join("packages").exists());
}

#[test]
fn locked_sync_fails_naming_a_malformed_lock_without_installing_or_rewriting_it() {
    let (workspace, project) = workspace_with_project();
    let fresh_state = workspace.join("ci-machine");
    fs::write(project.join(".mcpctl.lock"), "version = 1\n[[mcp]\nbroken").unwrap();

    let output = mcpctl(&project, &fresh_state, &["sync", "--locked"]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains(&format!("{} is malformed", project.join(".mcpctl.lock").display())), "{stderr}");
    assert_eq!(fs::read_to_string(project.join(".mcpctl.lock")).unwrap(), "version = 1\n[[mcp]\nbroken");
    assert!(!fresh_state.join("packages").exists());
}

#[cfg(unix)]
#[test]
fn sync_fails_naming_a_reused_version_whose_entrypoint_is_not_executable() {
    use std::os::unix::fs::PermissionsExt;
    let (workspace, project) = workspace_with_project();
    let state_home = workspace.join("state");
    assert!(mcpctl(&project, &state_home, &["sync"]).status.success());
    let entrypoint = state_home.join("packages/project-mcp/0.4.3/runtime/bin/bin/project-mcp");
    fs::set_permissions(&entrypoint, fs::Permissions::from_mode(0o644)).unwrap();

    let output = mcpctl(&project, &state_home, &["sync"]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("project-mcp@0.4.3 is installed but broken"), "{stderr}");
    assert!(stderr.contains(&format!("entrypoint {} is not executable", entrypoint.display())), "{stderr}");
}

/// Writes a binary-runtime MCP source at `workspace/dir` whose scaffold copies each `(path, contents)` in
/// `files` to the same project-relative path.
fn write_scaffold_package(workspace: &Path, dir: &str, name: &str, version: &str, files: &[(&str, &str)]) {
    write_package(workspace, dir, name, version);
    let source = workspace.join(dir);
    let mut manifest = fs::read_to_string(source.join("mcpctl.toml")).unwrap() + "\n[scaffold]\n";
    for (path, contents) in files {
        manifest.push_str(&format!("\n[[scaffold.files]]\nfrom = \"templates/{path}\"\nto = \"{path}\"\n"));
        let template = source.join("templates").join(path);
        fs::create_dir_all(template.parent().unwrap()).unwrap();
        fs::write(template, contents).unwrap();
    }
    fs::write(source.join("mcpctl.toml"), manifest).unwrap();
}

/// A project at `workspace/dir` declaring `hooks-mcp` (`^1`) from `../hooks-mcp`.
fn hooks_project(workspace: &Path, dir: &str) -> PathBuf {
    let project = workspace.join(dir);
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join(".mcpctl.toml"),
        format!("[project]\nname = \"{dir}\"\n\n[[mcp]]\nname = \"hooks-mcp\"\nversion = \"^1\"\nsource = \"../hooks-mcp\"\n"),
    )
    .unwrap();
    project
}

fn synced(project: &Path, state_home: &Path, arguments: &[&str]) -> String {
    let output = mcpctl(project, state_home, arguments);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(output.status.success(), "stdout: {stdout}\nstderr: {}", String::from_utf8_lossy(&output.stderr));
    stdout
}

#[test]
fn sync_after_a_version_bump_brings_unmodified_scaffold_files_up_to_date_and_keeps_edited_ones() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_scaffold_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[(".pi/hooks/check.mjs", "check v1"), (".pi/skills/review.md", "review v1")]);
    let project = hooks_project(&workspace, "service");
    synced(&project, &state_home, &["sync"]);
    assert_eq!(fs::read_to_string(project.join(".pi/hooks/check.mjs")).ok().as_deref(), Some("check v1"));
    fs::write(project.join(".pi/skills/review.md"), "our own review").unwrap();
    write_scaffold_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[(".pi/hooks/check.mjs", "check v2"), (".pi/skills/review.md", "review v2")]);

    synced(&project, &state_home, &["sync"]);

    assert_eq!(fs::read_to_string(project.join(".pi/hooks/check.mjs")).ok().as_deref(), Some("check v2"));
    assert_eq!(fs::read_to_string(project.join(".pi/skills/review.md")).ok().as_deref(), Some("our own review"));
}

#[test]
fn locked_sync_brings_scaffold_files_up_to_the_locked_versions() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_scaffold_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[(".pi/hooks/check.mjs", "check v1")]);
    let project = hooks_project(&workspace, "service");
    let teammate = hooks_project(&workspace, "service-teammate");
    synced(&project, &state_home, &["sync"]);
    synced(&teammate, &workspace.join("teammate-state"), &["sync"]);
    write_scaffold_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[(".pi/hooks/check.mjs", "check v2")]);
    synced(&project, &state_home, &["sync"]);
    // The teammate pulls the bumped lock.
    fs::copy(project.join(".mcpctl.lock"), teammate.join(".mcpctl.lock")).unwrap();

    synced(&teammate, &workspace.join("teammate-state"), &["sync", "--locked"]);

    assert_eq!(fs::read_to_string(teammate.join(".pi/hooks/check.mjs")).ok().as_deref(), Some("check v2"));
}

#[test]
fn sync_whose_scaffold_fails_still_locks_and_installs_then_fails_naming_sync_and_the_mcp() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_scaffold_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1")]);
    let manifest = workspace.join("hooks-mcp/mcpctl.toml");
    let failing_merge = "\n[[scaffold.files]]\nfrom = \"templates/docs/hooks.md\"\nto = \"settings.json\"\nmerge = \"json\"\n";
    fs::write(&manifest, fs::read_to_string(&manifest).unwrap() + failing_merge).unwrap();
    let project = hooks_project(&workspace, "service");
    fs::write(project.join("settings.json"), "not json").unwrap();

    let output = mcpctl(&project, &state_home, &["sync"]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "a failed scaffold fails sync");
    assert!(stderr.contains("sync failed for hooks-mcp"), "{stderr}");
    assert!(project.join(".mcpctl.lock").exists(), "the lock is written before scaffolds are applied");
    assert!(state_home.join("packages/hooks-mcp/1.0.0").exists(), "the locked version is installed before scaffolds are applied");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).ok().as_deref(), Some("hooks v1"));
}
