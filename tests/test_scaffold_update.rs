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

/// Installs and activates the current version of every MCP the project at `project` declares from its
/// `../<name>` source, as `sync` would select it, but without applying scaffolds (which `sync` also does), so
/// `init`'s own handling can be observed. A version the store already holds is left as it is.
fn install_declared(project: &Path, state_home: &Path) {
    let manifest = fs::read_to_string(project.join(".mcpctl.toml")).unwrap();
    for source in manifest.lines().filter_map(|line| line.strip_prefix("source = \"")?.strip_suffix('"')) {
        let output = mcpctl(project, state_home, &["install", source]);
        assert!(
            output.status.success() || String::from_utf8_lossy(&output.stderr).contains("is already installed"),
            "installing {source}: {}", String::from_utf8_lossy(&output.stderr)
        );
        let package: toml::Value = toml::from_str(&fs::read_to_string(project.join(source).join("mcpctl.toml")).unwrap()).unwrap();
        let selector = format!("{}@{}", package["name"].as_str().unwrap(), package["version"].as_str().unwrap());
        succeeded(&mcpctl(project, state_home, &["use", &selector]));
    }
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
    install_declared(&project, &state_home);
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    install_declared(&project, &state_home);

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
    install_declared(&project, &state_home);
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    fs::write(project.join("docs/hooks.md"), "hooks v1, edited").unwrap();
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    install_declared(&project, &state_home);

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
    install_declared(&project, &state_home);
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    fs::write(project.join("docs/hooks.md.mcpctl-new"), "my own notes").unwrap();
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    install_declared(&project, &state_home);

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
    install_declared(&project, &state_home);
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    let shared = workspace.join("shared-hooks.md");
    fs::write(&shared, "hooks v1").unwrap();
    fs::remove_file(project.join("docs/hooks.md")).unwrap();
    std::os::unix::fs::symlink(&shared, project.join("docs/hooks.md")).unwrap();
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    install_declared(&project, &state_home);

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
    install_declared(&project, &state_home);
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    fs::remove_file(project.join("docs/hooks.md")).unwrap();
    std::os::unix::fs::symlink(workspace.join("nowhere.md"), project.join("docs/hooks.md")).unwrap();
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    install_declared(&project, &state_home);

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
    install_declared(&project, &state_home);
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    install_declared(&project, &state_home);

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
    install_declared(&project, &state_home);
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    fs::write(project.join("docs/legacy.md"), "legacy v1, edited").unwrap();
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    install_declared(&project, &state_home);

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
    install_declared(&project, &state_home);
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    fs::remove_file(project.join("docs/legacy.md")).unwrap();
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    install_declared(&project, &state_home);

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
    install_declared(&project, &state_home);
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    let manifest = workspace.join("hooks-mcp/mcpctl.toml");
    let failing_merge = "\n[[scaffold.files]]\nfrom = \"templates/mcp.json\"\nto = \"settings.json\"\nmerge = \"json\"\n";
    fs::write(&manifest, fs::read_to_string(&manifest).unwrap() + failing_merge).unwrap();
    fs::write(project.join("settings.json"), "not json").unwrap();
    install_declared(&project, &state_home);

    let output = mcpctl(&project, &state_home, &["init"]);

    assert!(!output.status.success(), "the failing merge fails init");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("no longer shipped"));
    assert_eq!(record(&project)["files"]["docs/legacy.md"]["version"], "1.0.0");
    // The copy that succeeded before the failure is on disk, so it is recorded at the new version.
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v2");
    assert_eq!(
        record(&project)["files"]["docs/hooks.md"],
        serde_json::json!({ "mcp": "hooks-mcp", "version": "1.1.0", "sha256": sha256("hooks v2") })
    );
}

#[test]
fn init_reports_every_recorded_file_when_a_new_version_drops_its_scaffold_entirely() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    install_declared(&project, &state_home);
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[]);
    let manifest = workspace.join("hooks-mcp/mcpctl.toml");
    let without_scaffold = fs::read_to_string(&manifest).unwrap().split("[scaffold]").next().unwrap().to_owned();
    fs::write(&manifest, without_scaffold).unwrap();
    install_declared(&project, &state_home);

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("docs/hooks.md is no longer shipped by hooks-mcp@1.1.0 and is unchanged; it is safe to delete"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v1");
    assert!(record(&project)["files"].get("docs/hooks.md").is_none(), "{}", record(&project));
}

#[cfg(unix)]
#[test]
fn init_fails_and_keeps_recording_a_file_no_longer_shipped_that_it_cannot_inspect() {
    use std::os::unix::fs::PermissionsExt;
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1"), ("legacy/notes.md", "legacy v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    install_declared(&project, &state_home);
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    install_declared(&project, &state_home);
    // The file is still there, but init can't even tell whether it exists.
    fs::set_permissions(project.join("legacy"), fs::Permissions::from_mode(0o000)).unwrap();
    if fs::symlink_metadata(project.join("legacy/notes.md")).is_ok() {
        // Running with permission checks bypassed (e.g. as root): the error this test needs can't be provoked.
        fs::set_permissions(project.join("legacy"), fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("skipped: permission checks are bypassed for this user");
        return;
    }

    let output = mcpctl(&project, &state_home, &["init"]);

    fs::set_permissions(project.join("legacy"), fs::Permissions::from_mode(0o755)).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "stdout: {}", String::from_utf8_lossy(&output.stdout));
    assert!(stderr.contains("hooks-mcp@1.1.0") && stderr.contains("legacy/notes.md"), "{stderr}");
    assert_eq!(
        record(&project)["files"]["legacy/notes.md"],
        serde_json::json!({ "mcp": "hooks-mcp", "version": "1.0.0", "sha256": sha256("legacy v1") })
    );
}

/// A project scaffolded by `hooks-mcp@1.0.0` (shipping `docs/hooks.md` as "hooks v1") before hashes were
/// recorded, with 1.1.0 (shipping `hooks_v2`) installed since.
fn pre_record_project_upgraded_to(workspace: &Path, state_home: &Path, hooks_v2: &str) -> PathBuf {
    write_package(workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1")]);
    let project = write_project(workspace, &[("hooks-mcp", "^1")]);
    install_declared(&project, state_home);
    succeeded(&mcpctl(&project, state_home, &["init"]));
    fs::remove_file(project.join(".mcpctl.scaffold.json")).unwrap();
    write_package(workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", hooks_v2)]);
    install_declared(&project, state_home);
    project
}

#[test]
fn init_updates_and_records_an_unrecorded_file_identical_to_an_earlier_installed_version() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    let project = pre_record_project_upgraded_to(&workspace, &state_home, "hooks v2");

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("Updated docs/hooks.md"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v2");
    assert_eq!(
        record(&project)["files"]["docs/hooks.md"],
        serde_json::json!({ "mcp": "hooks-mcp", "version": "1.1.0", "sha256": sha256("hooks v2") })
    );
}

#[test]
fn init_records_an_unrecorded_file_already_identical_to_the_current_version_without_rewriting_it() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    let project = pre_record_project_upgraded_to(&workspace, &state_home, "hooks v1");

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(!stdout.contains("Updated docs/hooks.md") && !stdout.contains("Skipping docs/hooks.md"), "{stdout}");
    assert_eq!(
        record(&project)["files"]["docs/hooks.md"],
        serde_json::json!({ "mcp": "hooks-mcp", "version": "1.1.0", "sha256": sha256("hooks v1") })
    );
}

#[test]
fn init_skips_an_unrecorded_file_matching_no_installed_version_exactly_as_before() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    let project = pre_record_project_upgraded_to(&workspace, &state_home, "hooks v2");
    fs::write(project.join("docs/hooks.md"), "hooks v1, edited").unwrap();

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("Skipping docs/hooks.md (already exists)"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v1, edited");
    assert!(record(&project)["files"].get("docs/hooks.md").is_none(), "{}", record(&project));
}

#[cfg(unix)]
#[test]
fn init_of_a_single_mcp_skips_an_existing_file_without_reading_it() {
    use std::os::unix::fs::PermissionsExt;
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    install_declared(&project, &state_home);
    fs::create_dir_all(project.join("docs")).unwrap();
    fs::write(project.join("docs/hooks.md"), "the project's own").unwrap();
    fs::set_permissions(project.join("docs/hooks.md"), fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(project.join("docs/hooks.md")).is_ok() {
        // Running with permission checks bypassed (e.g. as root): an unreadable file can't be made.
        eprintln!("skipped: permission checks are bypassed for this user");
        return;
    }

    let output = mcpctl(&project, &state_home, &["init", "hooks-mcp"]);

    fs::set_permissions(project.join("docs/hooks.md"), fs::Permissions::from_mode(0o644)).unwrap();
    let stdout = succeeded(&output);
    assert!(stdout.contains("Skipping docs/hooks.md (already exists)"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "the project's own");
}

#[test]
fn init_never_adopts_a_project_file_matching_a_source_file_an_installed_version_did_not_copy_there() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    // 1.0.0 has templates/docs/hooks.md in its source, and copies a templates dir with it excluded, but its
    // scaffold never copies that file to docs/hooks.md.
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[]);
    let source = workspace.join("hooks-mcp");
    fs::create_dir_all(source.join("templates/docs")).unwrap();
    fs::write(source.join("templates/docs/hooks.md"), "the project's own").unwrap();
    let manifest = fs::read_to_string(source.join("mcpctl.toml")).unwrap()
        .replace("[scaffold]\n", "[scaffold]\ndirs = [\"templates\"]\nexclude = [\"templates/docs\"]\n");
    fs::write(source.join("mcpctl.toml"), manifest).unwrap();
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    install_declared(&project, &state_home);
    fs::create_dir_all(project.join("docs")).unwrap();
    fs::write(project.join("docs/hooks.md"), "the project's own").unwrap();
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    install_declared(&project, &state_home);

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("Skipping docs/hooks.md (already exists)"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "the project's own");
    assert!(record(&project)["files"].get("docs/hooks.md").is_none(), "{}", record(&project));
}

#[test]
fn init_adopts_an_unrecorded_file_an_earlier_version_copied_from_a_since_renamed_source() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    // 1.0.0 copies templates/old.md to docs/hooks.md; 1.1.0 copies templates/docs/hooks.md there instead.
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("old.md", "hooks v1")]);
    let manifest = workspace.join("hooks-mcp/mcpctl.toml");
    fs::write(&manifest, fs::read_to_string(&manifest).unwrap().replace("to = \"old.md\"", "to = \"docs/hooks.md\"")).unwrap();
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    install_declared(&project, &state_home);
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    fs::remove_file(project.join(".mcpctl.scaffold.json")).unwrap();
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    install_declared(&project, &state_home);

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("Updated docs/hooks.md"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v2");
    assert_eq!(
        record(&project)["files"]["docs/hooks.md"],
        serde_json::json!({ "mcp": "hooks-mcp", "version": "1.1.0", "sha256": sha256("hooks v2") })
    );
}

#[test]
fn init_never_adopts_an_identical_file_another_mcp_already_records() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    // other-mcp copies docs/hooks.md first and records it; hooks-mcp 1.0.0 shipped the very same bytes there.
    write_package(&workspace, "other-mcp", "other-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1")]);
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1")]);
    let project = write_project(&workspace, &[("other-mcp", "^1"), ("hooks-mcp", "^1")]);
    install_declared(&project, &state_home);
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    install_declared(&project, &state_home);

    succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v1");
    assert_eq!(
        record(&project)["files"]["docs/hooks.md"],
        serde_json::json!({ "mcp": "other-mcp", "version": "1.0.0", "sha256": sha256("hooks v1") })
    );
}

#[cfg(unix)]
#[test]
fn init_never_adopts_an_unrecorded_symlink_even_to_identical_content() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    let project = pre_record_project_upgraded_to(&workspace, &state_home, "hooks v2");
    let target = workspace.join("shared-hooks.md");
    fs::write(&target, "hooks v1").unwrap();
    fs::remove_file(project.join("docs/hooks.md")).unwrap();
    std::os::unix::fs::symlink(&target, project.join("docs/hooks.md")).unwrap();

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("Skipping docs/hooks.md (already exists)"), "{stdout}");
    assert!(fs::symlink_metadata(project.join("docs/hooks.md")).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(&target).unwrap(), "hooks v1");
    assert!(record(&project)["files"].get("docs/hooks.md").is_none(), "{}", record(&project));
}

#[cfg(unix)]
#[test]
fn init_never_adopts_an_unrecorded_dangling_symlink() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    let project = pre_record_project_upgraded_to(&workspace, &state_home, "hooks v2");
    let target = workspace.join("missing-hooks.md");
    fs::remove_file(project.join("docs/hooks.md")).unwrap();
    std::os::unix::fs::symlink(&target, project.join("docs/hooks.md")).unwrap();

    succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(fs::symlink_metadata(project.join("docs/hooks.md")).unwrap().file_type().is_symlink());
    assert!(!target.exists(), "nothing was written through the dangling symlink");
    assert!(record(&project)["files"].get("docs/hooks.md").is_none(), "{}", record(&project));
}

/// Breaks the installed manifest of `hooks-mcp@1.0.0` and drops an unrelated entry beside its versions.
fn break_inactive_snapshot(state_home: &Path) {
    let package = state_home.join("packages/hooks-mcp");
    fs::write(package.join("1.0.0/source/mcpctl.toml"), "not a manifest [").unwrap();
    fs::write(package.join("notes.txt"), "not a version").unwrap();
}

#[test]
fn init_of_a_fresh_project_ignores_unreadable_inactive_snapshots() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    install_declared(&project, &state_home);
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    install_declared(&project, &state_home);
    break_inactive_snapshot(&state_home);

    let output = mcpctl(&project, &state_home, &["init"]);

    let stdout = succeeded(&output);
    assert!(stdout.contains("Copied docs/hooks.md"), "{stdout}");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("1.0.0"), "the inactive snapshot is never read");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v2");
}

#[test]
fn init_skips_an_unreadable_inactive_snapshot_with_a_warning_when_looking_for_pre_record_copies() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    let project = pre_record_project_upgraded_to(&workspace, &state_home, "hooks v2");
    break_inactive_snapshot(&state_home);

    let output = mcpctl(&project, &state_home, &["init"]);

    let stdout = succeeded(&output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("Skipping docs/hooks.md (already exists)"), "{stdout}");
    assert!(stderr.contains("hooks-mcp@1.0.0") && stderr.contains("cannot read installed manifest"), "{stderr}");
    assert!(!stderr.contains("notes.txt"), "{stderr}");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v1");
}

#[test]
fn init_never_adopts_a_file_an_installed_version_excludes_from_the_dir_that_reaches_it() {
    // The excluded path is the file itself, or a directory above it.
    for (excluded, path) in [("docs/hooks.md", "docs/hooks.md"), ("docs/inner", "docs/inner/hooks.md")] {
        let workspace = temporary_dir();
        let state_home = workspace.join("state");
        write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[]);
        let source = workspace.join("hooks-mcp");
        fs::create_dir_all(source.join(path).parent().unwrap()).unwrap();
        fs::write(source.join(path), "the project's own").unwrap();
        let manifest = fs::read_to_string(source.join("mcpctl.toml")).unwrap()
            .replace("[scaffold]\n", &format!("[scaffold]\ndirs = [\"docs\"]\nexclude = [\"{excluded}\"]\n"));
        fs::write(source.join("mcpctl.toml"), manifest).unwrap();
        let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
        install_declared(&project, &state_home);
        fs::create_dir_all(project.join(path).parent().unwrap()).unwrap();
        fs::write(project.join(path), "the project's own").unwrap();
        write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[(path, "hooks v2")]);
        install_declared(&project, &state_home);

        let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

        assert!(stdout.contains(&format!("Skipping {path} (already exists)")), "{excluded}: {stdout}");
        assert_eq!(fs::read_to_string(project.join(path)).unwrap(), "the project's own", "{excluded}");
        assert!(record(&project)["files"].get(path).is_none(), "{excluded}: {}", record(&project));
    }
}

#[test]
fn init_never_adopts_a_file_an_installed_version_merges_into() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    // 1.0.0 merges templates/settings.json into config/settings.json; 1.1.0 copies a settings file there.
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("settings.json", "{\"a\": 1}")]);
    let manifest = workspace.join("hooks-mcp/mcpctl.toml");
    let merged = fs::read_to_string(&manifest).unwrap()
        .replace("to = \"settings.json\"\n", "to = \"config/settings.json\"\nmerge = \"json\"\n");
    fs::write(&manifest, merged).unwrap();
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    install_declared(&project, &state_home);
    fs::create_dir_all(project.join("config")).unwrap();
    fs::write(project.join("config/settings.json"), "{\"a\": 1}").unwrap();
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("config/settings.json", "{\"b\": 2}")]);
    install_declared(&project, &state_home);

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("Skipping config/settings.json (already exists)"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("config/settings.json")).unwrap(), "{\"a\": 1}");
    assert!(record(&project)["files"].get("config/settings.json").is_none(), "{}", record(&project));
}

/// A project where `hooks-mcp@1.0.0` copied `docs/hooks.md` ("hooks v1") and so owns it, then `other-mcp@1.0.0`
/// (shipping `other_copy` there) was added, listed first.
fn project_with_late_second_shipper(workspace: &Path, state_home: &Path, other_copy: &str) -> PathBuf {
    write_package(workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1")]);
    let project = write_project(workspace, &[("hooks-mcp", "^1")]);
    install_declared(&project, state_home);
    succeeded(&mcpctl(&project, state_home, &["init"]));
    write_package(workspace, "other-mcp", "other-mcp", "1.0.0", &[("docs/hooks.md", other_copy)]);
    let project = write_project(workspace, &[("other-mcp", "^1"), ("hooks-mcp", "^1")]);
    install_declared(&project, state_home);
    project
}

#[test]
fn a_shared_path_stays_with_the_mcp_that_copied_it_first_and_a_differing_copy_is_reported_as_a_conflict() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    let project = project_with_late_second_shipper(&workspace, &state_home, "other v1");

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("WARNING: docs/hooks.md is shipped by both other-mcp and hooks-mcp; keeping hooks-mcp's (it copied it first)"), "{stdout}");
    assert!(stdout.contains("Skipping docs/hooks.md (conflict: other-mcp@1.0.0 ships a different copy; keeping hooks-mcp@1.0.0's, which copied it first)"), "{stdout}");
    assert!(!stdout.contains("locally modified") && !stdout.contains("already exists"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v1");
    assert_eq!(
        record(&project)["files"]["docs/hooks.md"],
        serde_json::json!({ "mcp": "hooks-mcp", "version": "1.0.0", "sha256": sha256("hooks v1") })
    );

    // The owner still updates its file, and nothing changes hands on later runs.
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[("docs/hooks.md", "hooks v2")]);
    install_declared(&project, &state_home);
    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));
    assert!(stdout.contains("Updated docs/hooks.md"), "{stdout}");
    assert!(stdout.contains("Skipping docs/hooks.md (conflict: other-mcp@1.0.0 ships a different copy; keeping hooks-mcp@1.1.0's, which copied it first)"), "{stdout}");
    let before = fs::read(project.join(".mcpctl.scaffold.json")).unwrap();
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v2");
    assert_eq!(fs::read(project.join(".mcpctl.scaffold.json")).unwrap(), before);
    assert_eq!(record(&project)["files"]["docs/hooks.md"]["mcp"], "hooks-mcp");
}

#[test]
fn a_non_owner_shipping_the_owners_exact_copy_of_a_shared_path_reports_nothing_about_it() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    let project = project_with_late_second_shipper(&workspace, &state_home, "hooks v1");

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(!stdout.contains("Skipping docs/hooks.md") && !stdout.contains("Copied docs/hooks.md"), "{stdout}");
    assert_eq!(record(&project)["files"]["docs/hooks.md"]["mcp"], "hooks-mcp");
}

#[test]
fn a_non_owner_never_recreates_a_shared_path_the_project_deleted() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    let project = project_with_late_second_shipper(&workspace, &state_home, "other v1");
    fs::remove_file(project.join("docs/hooks.md")).unwrap();

    succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v1", "only the owner copies it again");
    assert_eq!(
        record(&project)["files"]["docs/hooks.md"],
        serde_json::json!({ "mcp": "hooks-mcp", "version": "1.0.0", "sha256": sha256("hooks v1") })
    );
}

#[test]
fn a_shared_path_owner_named_in_the_record_is_never_looked_up_outside_the_installed_packages() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    let project = project_with_late_second_shipper(&workspace, &state_home, "other v1");
    // A record naming an owner whose name walks out of the state home, where a manifest ships other-mcp's copy.
    let planted = workspace.join("planted/x/source");
    fs::create_dir_all(planted.join("templates/docs")).unwrap();
    fs::write(planted.join("templates/docs/hooks.md"), "other v1").unwrap();
    fs::write(
        planted.join("mcpctl.toml"),
        "name = \"planted\"\nversion = \"1.0.0\"\n\n[runtime]\ntype = \"binary\"\n\n[install]\nentrypoint = \"bin/planted\"\n\n\
         [scaffold]\n\n[[scaffold.files]]\nfrom = \"templates/docs/hooks.md\"\nto = \"docs/hooks.md\"\n",
    ).unwrap();
    let mut recorded = record(&project);
    recorded["files"]["docs/hooks.md"]["mcp"] = serde_json::json!("../../planted");
    recorded["files"]["docs/hooks.md"]["version"] = serde_json::json!("x");
    fs::write(project.join(".mcpctl.scaffold.json"), serde_json::to_string(&recorded).unwrap()).unwrap();

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    // Compared with the recorded copy ("hooks v1"), not the planted one.
    assert!(stdout.contains("Skipping docs/hooks.md (conflict: other-mcp@1.0.0 ships a different copy; keeping ../../planted@x's, which copied it first)"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v1");
}

#[test]
fn a_shared_path_whose_owner_left_the_project_is_compared_with_the_recorded_copy_not_the_owners_snapshot() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    project_with_late_second_shipper(&workspace, &state_home, "other v1");
    // hooks-mcp leaves the project; its installed snapshot now ships other-mcp's copy, unlike the one it recorded.
    fs::write(state_home.join("packages/hooks-mcp/1.0.0/source/templates/docs/hooks.md"), "other v1").unwrap();
    let project = write_project(&workspace, &[("other-mcp", "^1")]);

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("Skipping docs/hooks.md (conflict: other-mcp@1.0.0 ships a different copy; keeping hooks-mcp@1.0.0's, which copied it first)"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v1");
    assert_eq!(record(&project)["files"]["docs/hooks.md"]["mcp"], "hooks-mcp");
}

#[test]
fn a_shared_path_its_owner_stopped_shipping_is_compared_with_the_copy_the_owner_recorded() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    let project = project_with_late_second_shipper(&workspace, &state_home, "other v1");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.1.0", &[]);
    install_declared(&project, &state_home);

    let stdout = succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert!(stdout.contains("Skipping docs/hooks.md (conflict: other-mcp@1.0.0 ships a different copy; keeping hooks-mcp@1.0.0's, which copied it first)"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v1");
}

#[cfg(unix)]
#[test]
fn an_unreadable_owners_copy_of_a_shared_path_is_warned_about_without_stopping_the_non_owners_other_files() {
    use std::os::unix::fs::PermissionsExt;
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    project_with_late_second_shipper(&workspace, &state_home, "other v1");
    write_package(&workspace, "other-mcp", "other-mcp", "1.0.1", &[("docs/hooks.md", "other v1"), ("docs/other.md", "other notes")]);
    let project = write_project(&workspace, &[("other-mcp", "^1"), ("hooks-mcp", "^1")]);
    install_declared(&project, &state_home);
    let owners_copy = state_home.join("packages/hooks-mcp/1.0.0/source/templates/docs/hooks.md");
    fs::set_permissions(&owners_copy, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&owners_copy).is_ok() {
        // Running with permission checks bypassed (e.g. as root): an unreadable file can't be made.
        eprintln!("skipped: permission checks are bypassed for this user");
        return;
    }

    let output = mcpctl(&project, &state_home, &["init"]);

    fs::set_permissions(&owners_copy, fs::Permissions::from_mode(0o644)).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning: cannot compare docs/hooks.md with hooks-mcp@1.0.0's copy"), "{stderr}");
    assert_eq!(fs::read_to_string(project.join("docs/other.md")).ok().as_deref(), Some("other notes"), "stdout: {}\nstderr: {stderr}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(fs::read_to_string(project.join("docs/hooks.md")).unwrap(), "hooks v1");
    assert_eq!(record(&project)["files"]["docs/hooks.md"]["mcp"], "hooks-mcp");
}
