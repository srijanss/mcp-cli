use std::{fs, path::{Path, PathBuf}, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_dir() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-scaffold-record-test-{}-{}",
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
fn init_records_each_copied_file_once_and_leaves_the_record_alone_on_rerun() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[(".pi/hooks/check.mjs", "check v1"), ("docs/hooks.md", "hooks v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));

    succeeded(&mcpctl(&project, &state_home, &["init"]));

    let recorded = record(&project);
    let files = recorded["files"].as_object().unwrap();
    assert_eq!(files.keys().collect::<Vec<_>>(), [".pi/hooks/check.mjs", "docs/hooks.md"], "merged .mcp.json is not recorded: {recorded}");
    assert_eq!(files[".pi/hooks/check.mjs"], serde_json::json!({ "mcp": "hooks-mcp", "version": "1.0.0", "sha256": sha256("check v1") }));
    assert_eq!(files["docs/hooks.md"], serde_json::json!({ "mcp": "hooks-mcp", "version": "1.0.0", "sha256": sha256("hooks v1") }));

    let before = fs::read(project.join(".mcpctl.scaffold.json")).unwrap();
    succeeded(&mcpctl(&project, &state_home, &["init"]));
    assert_eq!(fs::read(project.join(".mcpctl.scaffold.json")).unwrap(), before);
}

#[cfg(unix)]
#[test]
fn init_never_writes_through_a_symlink_planted_at_the_record_temp_path() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[("docs/hooks.md", "hooks v1")]);
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));
    let victim = workspace.join("victim.txt");
    fs::write(&victim, "precious").unwrap();
    std::os::unix::fs::symlink(&victim, project.join(".mcpctl.scaffold.json.tmp")).unwrap();

    succeeded(&mcpctl(&project, &state_home, &["init"]));

    assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
    assert!(record(&project)["files"]["docs/hooks.md"].is_object());
}

#[cfg(unix)]
#[test]
fn init_records_files_copied_from_scaffold_dirs_and_keeps_their_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    write_package(&workspace, "hooks-mcp", "hooks-mcp", "1.0.0", &[]);
    let source = workspace.join("hooks-mcp");
    fs::create_dir_all(source.join("skills/review")).unwrap();
    fs::write(source.join("skills/review/run.sh"), "#!/bin/sh\necho review\n").unwrap();
    fs::set_permissions(source.join("skills/review/run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    let manifest = fs::read_to_string(source.join("mcpctl.toml")).unwrap().replace("[scaffold]\n", "[scaffold]\ndirs = [\"skills\"]\n");
    fs::write(source.join("mcpctl.toml"), manifest).unwrap();
    let project = write_project(&workspace, &[("hooks-mcp", "^1")]);
    succeeded(&mcpctl(&project, &state_home, &["sync"]));

    succeeded(&mcpctl(&project, &state_home, &["init"]));

    let copied = project.join("skills/review/run.sh");
    assert_eq!(fs::metadata(&copied).unwrap().permissions().mode() & 0o777, 0o755);
    assert_eq!(
        record(&project)["files"]["skills/review/run.sh"],
        serde_json::json!({ "mcp": "hooks-mcp", "version": "1.0.0", "sha256": sha256("#!/bin/sh\necho review\n") })
    );
}
