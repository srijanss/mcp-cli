use std::{fs, path::PathBuf, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

const MANIFEST: &str = r#"name = "scaf-mcp"
version = "1.0.0"

[runtime]
type = "binary"

[install]
entrypoint = "scaf-mcp"

[scaffold]
dirs = [".agents"]
exclude = [".agents/private.txt", "templates/mcp.json"]

[[scaffold.files]]
from = "templates/mcp.json"
to = ".mcp.json"

[[scaffold.files]]
from = "templates/settings.json"
to = ".claude/settings.json"
merge = "json"

[[scaffold.files]]
from = "templates/config.toml"
to = ".codex/config.toml"
merge = "toml"

[[scaffold.hints]]
message = "Defaults to pytest."

[[scaffold.hints]]
when_exists = "Cargo.toml"
message = "Rust project detected: use cargo-adapter-runner."

[[scaffold.hints]]
when_exists = "package.json"
message = "JS/TS project detected: use vitest-adapter-runner."
"#;

fn temporary_dir(label: &str) -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!("mcpctl-init-{label}-{}-{}", std::process::id(), NEXT_ID.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&path).unwrap();
    path
}

/// A binary MCP project that ships a scaffold, installed into a fresh `MCPCTL_HOME`.
#[cfg(unix)]
fn installed_scaffold_mcp() -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let project = temporary_dir("project");
    fs::write(project.join("mcpctl.toml"), MANIFEST).unwrap();
    fs::write(project.join("scaf-mcp"), "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(project.join("scaf-mcp"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::create_dir_all(project.join("templates")).unwrap();
    fs::write(project.join("templates/mcp.json"), "{\"from\":\"template\"}\n").unwrap();
    fs::write(
        project.join("templates/config.toml"),
        "[mcp_servers.scaf]\ncommand = \"scaf\"\n\n[[hooks.PreToolUse]]\nmatcher = \"Bash\"\n",
    )
    .unwrap();
    fs::write(
        project.join("templates/settings.json"),
        r#"{"keep":2,"mcpServers":{"scaf":{"command":"scaf"}},"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"command":"a"}]}]}}"#,
    )
    .unwrap();
    fs::create_dir_all(project.join(".agents/skills")).unwrap();
    fs::write(project.join(".agents/skills/tdd.md"), "skill\n").unwrap();
    fs::write(project.join(".agents/private.txt"), "secret\n").unwrap();
    let state_home = project.join("state");
    let installed = Command::new(env!("CARGO_BIN_EXE_mcp-cli")).args(["install", project.to_str().unwrap()]).env("MCPCTL_HOME", &state_home).output().unwrap();
    assert!(installed.status.success(), "install failed: {}", String::from_utf8_lossy(&installed.stderr));
    (project, state_home)
}

fn init(state_home: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mcp-cli")).arg("init").args(args).env("MCPCTL_HOME", state_home).output().unwrap()
}

#[cfg(unix)]
#[test]
fn init_copies_scaffold_files_and_dirs_from_the_installed_package_into_the_target() {
    let (project, state_home) = installed_scaffold_mcp();
    let target = temporary_dir("target");

    let output = init(&state_home, &["scaf-mcp", target.to_str().unwrap()]);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read_to_string(target.join(".mcp.json")).unwrap(), "{\"from\":\"template\"}\n");
    assert_eq!(fs::read_to_string(target.join(".agents/skills/tdd.md")).unwrap(), "skill\n");
    assert!(!target.join(".agents/private.txt").exists(), "excluded files must not be copied");
    assert!(!target.join("templates").exists(), "only declared scaffold entries are copied");

    fs::remove_dir_all(project).unwrap();
    fs::remove_dir_all(target).unwrap();
}

#[cfg(unix)]
#[test]
fn init_never_overwrites_existing_files_and_reports_what_it_skipped_and_copied() {
    let (project, state_home) = installed_scaffold_mcp();
    let target = temporary_dir("target");
    fs::write(target.join(".mcp.json"), "mine\n").unwrap();

    let output = init(&state_home, &["scaf-mcp", target.to_str().unwrap()]);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read_to_string(target.join(".mcp.json")).unwrap(), "mine\n", "existing file must be kept");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Skipping .mcp.json (already exists)"), "stdout was: {stdout}");
    assert!(stdout.contains("Copied .agents/skills/tdd.md"), "stdout was: {stdout}");
    assert_eq!(fs::read_to_string(target.join(".agents/skills/tdd.md")).unwrap(), "skill\n");

    fs::remove_dir_all(project).unwrap();
    fs::remove_dir_all(target).unwrap();
}

#[cfg(unix)]
#[test]
fn init_prints_only_the_hints_whose_marker_file_exists_in_the_target() {
    let (project, state_home) = installed_scaffold_mcp();
    let target = temporary_dir("target");
    fs::write(target.join("Cargo.toml"), "[package]\n").unwrap();

    let output = init(&state_home, &["scaf-mcp", target.to_str().unwrap()]);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Defaults to pytest."), "unconditional hints are always printed, stdout was: {stdout}");
    assert!(stdout.contains("Rust project detected: use cargo-adapter-runner."), "stdout was: {stdout}");
    assert!(!stdout.contains("JS/TS project detected"), "package.json is absent, stdout was: {stdout}");

    fs::remove_dir_all(project).unwrap();
    fs::remove_dir_all(target).unwrap();
}

#[cfg(unix)]
#[test]
fn init_copies_an_explicitly_mapped_file_even_when_exclude_lists_its_source_path() {
    // `exclude` only filters the `dirs` copies; a file named in `files` is always copied.
    let (project, state_home) = installed_scaffold_mcp();
    let target = temporary_dir("target");

    let output = init(&state_home, &["scaf-mcp", target.to_str().unwrap()]);

    assert!(output.status.success(), "stderr was: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read_to_string(target.join(".mcp.json")).unwrap(), "{\"from\":\"template\"}\n");
    assert!(!target.join(".agents/private.txt").exists(), "dirs copies still honour exclude");

    fs::remove_dir_all(project).unwrap();
    fs::remove_dir_all(target).unwrap();
}

#[cfg(unix)]
#[test]
fn init_merges_a_json_template_into_an_existing_file_without_losing_or_duplicating_anything() {
    let (project, state_home) = installed_scaffold_mcp();
    let target = temporary_dir("target");
    fs::create_dir_all(target.join(".claude")).unwrap();
    fs::write(
        target.join(".claude/settings.json"),
        r#"{"keep":1,"mcpServers":{"other":{"command":"x"}},"hooks":{"PreToolUse":[{"matcher":"Edit","hooks":[{"command":"b"}]}]}}"#,
    )
    .unwrap();

    let first = init(&state_home, &["scaf-mcp", target.to_str().unwrap()]);

    assert!(first.status.success(), "stderr was: {}", String::from_utf8_lossy(&first.stderr));
    let merged: serde_json::Value = serde_json::from_str(&fs::read_to_string(target.join(".claude/settings.json")).unwrap()).unwrap();
    assert_eq!(merged["keep"], 1, "existing scalars win over the template");
    assert_eq!(merged["mcpServers"]["other"]["command"], "x", "existing keys are kept");
    assert_eq!(merged["mcpServers"]["scaf"]["command"], "scaf", "template keys are added");
    let pre_tool_use = merged["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre_tool_use.len(), 2, "template array items are appended: {pre_tool_use:?}");
    assert!(String::from_utf8_lossy(&first.stdout).contains("Merged .claude/settings.json"), "stdout was: {}", String::from_utf8_lossy(&first.stdout));

    fs::remove_dir_all(project).unwrap();
    fs::remove_dir_all(target).unwrap();
}

#[cfg(unix)]
#[test]
fn init_merges_a_toml_template_into_an_existing_file_without_losing_or_duplicating_anything() {
    let (project, state_home) = installed_scaffold_mcp();
    let target = temporary_dir("target");
    fs::create_dir_all(target.join(".codex")).unwrap();
    fs::write(
        target.join(".codex/config.toml"),
        "model = \"mine\"\n\n[mcp_servers.other]\ncommand = \"x\"\n\n[[hooks.PreToolUse]]\nmatcher = \"Edit\"\n",
    )
    .unwrap();

    let first = init(&state_home, &["scaf-mcp", target.to_str().unwrap()]);

    assert!(first.status.success(), "stderr was: {}", String::from_utf8_lossy(&first.stderr));
    let merged: toml::Value = fs::read_to_string(target.join(".codex/config.toml")).unwrap().parse().unwrap();
    assert_eq!(merged["model"].as_str(), Some("mine"));
    assert_eq!(merged["mcp_servers"]["other"]["command"].as_str(), Some("x"));
    assert_eq!(merged["mcp_servers"]["scaf"]["command"].as_str(), Some("scaf"));
    assert_eq!(merged["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
    assert!(String::from_utf8_lossy(&first.stdout).contains("Merged .codex/config.toml"), "stdout was: {}", String::from_utf8_lossy(&first.stdout));

    let second = init(&state_home, &["scaf-mcp", target.to_str().unwrap()]);
    let again: toml::Value = fs::read_to_string(target.join(".codex/config.toml")).unwrap().parse().unwrap();
    assert_eq!(again, merged, "a second init changes nothing");
    assert!(String::from_utf8_lossy(&second.stdout).contains("Unchanged .codex/config.toml"), "stdout was: {}", String::from_utf8_lossy(&second.stdout));

    fs::remove_dir_all(project).unwrap();
    fs::remove_dir_all(target).unwrap();
}

#[cfg(unix)]
#[test]
fn init_refuses_to_merge_into_an_unparseable_file_and_leaves_it_untouched() {
    let (project, state_home) = installed_scaffold_mcp();
    let target = temporary_dir("target");
    fs::create_dir_all(target.join(".claude")).unwrap();
    fs::write(target.join(".claude/settings.json"), "{ not json").unwrap();

    let output = init(&state_home, &["scaf-mcp", target.to_str().unwrap()]);

    assert!(!output.status.success(), "a corrupt destination must fail the command");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(".claude/settings.json") && stderr.contains("not valid JSON"), "stderr was: {stderr}");
    assert_eq!(fs::read_to_string(target.join(".claude/settings.json")).unwrap(), "{ not json", "the file must not be modified");

    fs::remove_dir_all(project).unwrap();
    fs::remove_dir_all(target).unwrap();
}
