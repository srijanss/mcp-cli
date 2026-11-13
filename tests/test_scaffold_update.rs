use std::{fs, path::{Path, PathBuf}, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_dir() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-scaffold-update-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

/// Writes a binary-runtime MCP source at `workspace/dir` whose scaffold copies each `(path, contents)` in
/// `files` to the same project-relative path, and merges a `.mcp.json` naming the MCP.
fn write_package(workspace: &Path, dir: &str, name: &str, version: &str, files: &[(&str, &str)]) {
    let source = workspace.join(dir);
    let _ = fs::remove_dir_all(&source);
    fs::create_dir_all(source.join("bin")).unwrap();
    fs::create_dir_all(source.join("templates")).unwrap();
    let mut manifest = format!(
        "name = \"{name}\"\nversion = \"{version}\"\n\n[runtime]\ntype = \"binary\"\n\n[install]\nentrypoint = \"bin/{name}\"\n\n\
         [scaffold]\n\n[[scaffold.files]]\nfrom = \"templates/mcp.json\"\nto = \".mcp.json\"\nmerge = \"json\"\n"
    );
    for (path, contents) in files {
        manifest.push_str(&format!("\n[[scaffold.files]]\nfrom = \"templates/{path}\"\nto = \"{path}\"\n"));
        let template = source.join("templates").join(path);
        fs::create_dir_all(template.parent().unwrap()).unwrap();
        fs::write(template, contents).unwrap();
    }
    fs::write(source.join("mcpctl.toml"), manifest).unwrap();
    fs::write(source.join("templates/mcp.json"), format!("{{\"mcpServers\": {{\"{name}\": {{\"command\": \"{name}\"}}}}}}")).unwrap();
    fs::write(source.join("bin").join(name), "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(source.join("bin").join(name), fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// A `service/` project declaring each `(name, constraint)` with source `../<name>`.
fn write_project(workspace: &Path, mcps: &[(&str, &str)]) -> PathBuf {
    let project = workspace.join("service");
    fs::create_dir_all(&project).unwrap();
    let mut manifest = "[project]\nname = \"service\"\n".to_owned();
    for (name, constraint) in mcps {
        manifest.push_str(&format!("\n[[mcp]]\nname = \"{name}\"\nversion = \"{constraint}\"\nsource = \"../{name}\"\n"));
    }
    fs::write(project.join(".mcpctl.toml"), manifest).unwrap();
    project
}

fn mcpctl(directory: &Path, state_home: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mcp-cli"))
        .args(arguments)
        .current_dir(directory)
        .env("MCPCTL_HOME", state_home)
        .output()
        .unwrap()
}

fn succeeded(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(output.status.success(), "stdout: {stdout}\nstderr: {}", String::from_utf8_lossy(&output.stderr));
    stdout
}

fn record(project: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(project.join(".mcpctl.scaffold.json")).unwrap()).unwrap()
}

fn sha256(contents: &str) -> String {
    mcp_cli::metadata::sha256_hex(contents.as_bytes())
}

#[test]
fn init_replaces_an_unmodified_file_with_the_newer_installed_version_and_records_it() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("Updated docs/hooks.md"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v2");
    assert_eq!(
        record(&project)["files"]["docs/hooks.md"],
        serde_json::json!({ "mcp": "hooks-mcp", "version": "1.1.0", "sha256": sha256("hooks v2") })
    );
}

#[test]
fn init_leaves_a_locally_modified_file_alone_and_names_the_new_version_to_merge_by_hand() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    fs::write(project.join("docs/hooks.md"), "hooks v1, edited").unwrap();
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    let installed = state_home.join("packages/hooks-mcp/1.1.0/source/templates/docs/hooks.md");
    assert!(stdout.contains(&format!("Skipping docs/hooks.md (locally modified; merge {} by hand)", installed.display())), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v1, edited");
    assert_eq!(
        record(&project)["files"]["docs/hooks.md"],
        serde_json::json!({ "mcp": "hooks-mcp", "version": "1.0.0", "sha256": sha256("hooks v1") })
    );
}

#[test]
fn updating_a_file_never_touches_a_project_file_named_like_its_temporary_copy() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    fs::write(project.join("docs/hooks.md.mcpctl-new"), "my own notes").unwrap();
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));

    succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v2");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md.mcpctl-new")).unwrap(), "my own notes");
}

#[cfg(unix)]
#[test]
fn init_leaves_a_recorded_file_the_project_replaced_with_a_symlink_alone() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    let shared = workspace.join("shared-hooks.md");
    fs::write(&shared, "hooks v1").unwrap();
    fs::remove_file(project.join("docs/hooks.md")).unwrap();
    std::os::unix::fs::symlink(&shared, project.join("docs/hooks.md")).unwrap();
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("Skipping docs/hooks.md (locally modified"), "{stdout}");
    assert!(fs::symlink_metadata(project.join("docs/hooks.md")).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(&shared).unwrap(), "hooks v1");
    assert_eq!(record(&project)["files"]["docs/hooks.md"]["version"], "1.0.0");
}

#[cfg(unix)]
#[test]
fn init_leaves_a_recorded_file_the_project_replaced_with_a_dangling_symlink_alone() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    fs::remove_file(project.join("docs/hooks.md")).unwrap();
    std::os::unix::fs::symlink(workspace.join("nowhere.md"), project.join("docs/hooks.md")).unwrap();
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("Skipping docs/hooks.md (locally modified"), "{stdout}");
    assert!(fs::symlink_metadata(project.join("docs/hooks.md")).unwrap().file_type().is_symlink());
    assert_eq!(record(&project)["files"]["docs/hooks.md"]["version"], "1.0.0");
}

#[test]
fn init_reports_an_unmodified_file_no_longer_shipped_as_safe_to_delete_and_stops_recording_it() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1"), ("docs/legacy.md", "legacy v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("docs/legacy.md is no longer shipped by hooks-mcp@1.1.0 and is unchanged; it is safe to delete"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/legacy.md")).unwrap(), "legacy v1");
    assert!(record(&project)["files"].get("docs/legacy.md").is_none(), "{}", record(&project));
}

#[test]
fn init_keeps_a_locally_modified_file_no_longer_shipped_and_stops_recording_it() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1"), ("docs/legacy.md", "legacy v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    fs::write(project.join("docs/legacy.md"), "legacy v1, edited").unwrap();
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("docs/legacy.md is no longer shipped by hooks-mcp@1.1.0 and is locally modified; keeping it"), "{stdout}");
    assert!(!stdout.contains("safe to delete"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/legacy.md")).unwrap(), "legacy v1, edited");
    assert!(record(&project)["files"].get("docs/legacy.md").is_none(), "{}", record(&project));
}

#[test]
fn init_quietly_stops_recording_a_file_no_longer_shipped_that_the_project_already_deleted() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1"), ("docs/legacy.md", "legacy v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    fs::remove_file(project.join("docs/legacy.md")).unwrap();
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(!stdout.contains("docs/legacy.md"), "{stdout}");
    assert!(!project.join("docs/legacy.md").exists());
    assert!(record(&project)["files"].get("docs/legacy.md").is_none(), "{}", record(&project));
}

#[test]
fn init_keeps_recording_files_when_the_new_scaffold_fails_before_it_has_seen_them_all() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1"), ("docs/legacy.md", "legacy v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    let manifest = workspace.join("hooks-mcp/mcpctl.toml");
    let failing_merge = "\n[[scaffold.files]]\nfrom = \"templates/mcp.json\"\nto = \"settings.json\"\nmerge = \"json\"\n";
    fs::write(&manifest, fs::read_to_string(&manifest).unwrap() + failing_merge).unwrap();
    fs::write(project.join("settings.json"), "not json").unwrap();
    succeeded(&mcpctl(&project, &state_home, &["sync"]));

    let output = mcpctl(&project, &state_home, &["init"]);

    assert!(!output.status.success(), "the failing merge fails init");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("no longer shipped"));
    assert_eq!(record(&project)["files"]["docs/legacy.md"]["version"], "1.0.0");
}

#[test]
fn init_reports_every_recorded_file_when_a_new_version_drops_its_scaffold_entirely() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[]);
    let manifest = workspace.join("hooks-mcp/mcpctl.toml");
    let without_scaffold = fs::read_to_string(&manifest).unwrap().split("[scaffold]").next().unwrap().to_owned();
    fs::write(&manifest, without_scaffold).unwrap();
    succeeded(&mcpctl(&project, &state_home, &["sync"]));

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("docs/hooks.md is no longer shipped by hooks-mcp@1.1.0 and is unchanged; it is safe to delete"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v1");
    assert!(record(&project)["files"].get("docs/hooks.md").is_none(), "{}", record(&project));
}
