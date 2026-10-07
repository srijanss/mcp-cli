use std::{fs, path::{Path, PathBuf}, process::{Command, Output}, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_dir() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-init-project-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

/// Writes a binary-runtime MCP source at `workspace/dir` whose scaffold copies
/// `docs/<name>.md` (holding `name@version`) and the `.agents/<name>/` directory.
fn write_package(workspace: &Path, dir: &str, name: &str, version: &str) {
    let source = workspace.join(dir);
    fs::create_dir_all(source.join("bin")).unwrap();
    fs::create_dir_all(source.join("templates")).unwrap();
    fs::create_dir_all(source.join(".agents").join(name)).unwrap();
    fs::write(
        source.join("mcpctl.toml"),
        format!(
            "name = \"{name}\"\nversion = \"{version}\"\n\n[runtime]\ntype = \"binary\"\n\n[install]\nentrypoint = \"bin/{name}\"\n\n\
             [scaffold]\ndirs = [\".agents/{name}\"]\n\n[[scaffold.files]]\nfrom = \"templates/{name}.md\"\nto = \"docs/{name}.md\"\n"
        ),
    )
    .unwrap();
    fs::write(source.join("templates").join(format!("{name}.md")), format!("{name}@{version}")).unwrap();
    fs::write(source.join(".agents").join(name).join("skill.md"), format!("{name} skill")).unwrap();
    fs::write(source.join("bin").join(name), "#!/bin/sh\nexit 0\n").unwrap();
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
fn init_without_an_mcp_scaffolds_every_locked_project_mcp_into_the_project_root() {
    let (workspace, project) = workspace_with_project();
    let state_home = workspace.join("state");
    assert!(mcpctl(&project, &state_home, &["sync"]).status.success());
    write_package(&workspace, "project-mcp-next", "project-mcp", "0.5.0");
    assert!(mcpctl(&workspace, &state_home, &["install", "project-mcp-next"]).status.success());
    assert!(mcpctl(&workspace, &state_home, &["use", "project-mcp@0.5.0"]).status.success());
    let nested = project.join("src/handlers");
    fs::create_dir_all(&nested).unwrap();

    let outside = mcpctl(&workspace, &state_home, &["init"]);
    let output = mcpctl(&nested, &state_home, &["init"]);

    assert!(!outside.status.success());
    assert!(String::from_utf8_lossy(&outside.stderr).contains(".mcpctl.toml"), "{}", String::from_utf8_lossy(&outside.stderr));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read_to_string(project.join("docs/project-mcp.md")).unwrap(), "project-mcp@0.4.3");
    assert_eq!(fs::read_to_string(project.join("docs/design-advisor-mcp.md")).unwrap(), "design-advisor-mcp@0.2.0");
    assert_eq!(fs::read_to_string(project.join(".agents/project-mcp/skill.md")).unwrap(), "project-mcp skill");
    assert_eq!(fs::read_to_string(project.join(".agents/design-advisor-mcp/skill.md")).unwrap(), "design-advisor-mcp skill");
    assert!(!nested.join("docs").exists(), "scaffolds land in the project root, not the current directory");
    assert!(!workspace.join("docs").exists());
}

#[test]
fn init_uses_a_satisfying_installed_version_for_unlocked_mcps_and_names_any_it_cannot_resolve() {
    let (workspace, project) = workspace_with_project();
    let state_home = workspace.join("state");
    assert!(mcpctl(&workspace, &state_home, &["install", "project-mcp"]).status.success());
    assert!(mcpctl(&workspace, &state_home, &["install", "design-advisor-mcp"]).status.success());

    let unlocked = mcpctl(&project, &state_home, &["init"]);

    assert!(unlocked.status.success(), "{}", String::from_utf8_lossy(&unlocked.stderr));
    assert!(!project.join(".mcpctl.lock").exists(), "init never writes the lock");
    assert_eq!(fs::read_to_string(project.join("docs/project-mcp.md")).unwrap(), "project-mcp@0.4.3");
    assert_eq!(fs::read_to_string(project.join("docs/design-advisor-mcp.md")).unwrap(), "design-advisor-mcp@0.2.0");

    write_package(&workspace, "project-mcp-next", "project-mcp", "0.5.0");
    assert!(mcpctl(&workspace, &state_home, &["install", "project-mcp-next"]).status.success());
    assert!(mcpctl(&workspace, &state_home, &["use", "project-mcp@0.5.0"]).status.success());

    let unsatisfied = mcpctl(&project, &state_home, &["init"]);

    let stderr = String::from_utf8_lossy(&unsatisfied.stderr);
    assert!(!unsatisfied.status.success());
    assert!(stderr.contains("project-mcp") && stderr.contains("^0.4") && stderr.contains("mcpctl sync"), "{stderr}");

    fs::write(
        project.join(".mcpctl.lock"),
        "version = 1\n\n[[mcp]]\nname = \"project-mcp\"\nversion = \"0.4.9\"\nsource = \"../project-mcp\"\nmanifest_digest = \"sha256:abc123\"\n",
    )
    .unwrap();

    let not_installed = mcpctl(&project, &state_home, &["init"]);

    let stderr = String::from_utf8_lossy(&not_installed.stderr);
    assert!(!not_installed.status.success());
    assert!(stderr.contains("project-mcp@0.4.9"), "{stderr}");
    assert!(stderr.contains(&project.join(".mcpctl.lock").display().to_string()), "{stderr}");
}

#[test]
fn init_skips_a_project_mcp_without_a_scaffold_and_still_initializes_the_others() {
    let (workspace, project) = workspace_with_project();
    let state_home = workspace.join("state");
    let plain = workspace.join("plain-mcp");
    fs::create_dir_all(&plain).unwrap();
    fs::write(plain.join("mcpctl.toml"), "name = \"plain-mcp\"\nversion = \"1.0.0\"\n\n[runtime]\ntype = \"binary\"\n\n[install]\nentrypoint = \"plain-mcp\"\n").unwrap();
    fs::write(plain.join("plain-mcp"), "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(plain.join("plain-mcp"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    let mut manifest = fs::read_to_string(project.join(".mcpctl.toml")).unwrap();
    manifest = manifest.replacen("[[mcp]]", "[[mcp]]\nname = \"plain-mcp\"\nversion = \"^1\"\nsource = \"../plain-mcp\"\n\n[[mcp]]", 1);
    fs::write(project.join(".mcpctl.toml"), manifest).unwrap();
    assert!(mcpctl(&project, &state_home, &["sync"]).status.success());

    let output = mcpctl(&project, &state_home, &["init"]);
    let single = mcpctl(&project, &state_home, &["init", "plain-mcp"]);

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Skipping plain-mcp@1.0.0 (no [scaffold])"), "{stdout}");
    assert_eq!(fs::read_to_string(project.join("docs/project-mcp.md")).unwrap(), "project-mcp@0.4.3");
    assert_eq!(fs::read_to_string(project.join("docs/design-advisor-mcp.md")).unwrap(), "design-advisor-mcp@0.2.0");
    assert!(!single.status.success(), "naming an MCP without a scaffold is still an error");
    assert!(String::from_utf8_lossy(&single.stderr).contains("does not ship a scaffold"), "{}", String::from_utf8_lossy(&single.stderr));
}

/// Adds templates to an MCP source that merge its server entry into the project's shared `.mcp.json` and `.codex/config.toml`.
fn add_shared_config(workspace: &Path, dir: &str, name: &str) {
    let source = workspace.join(dir);
    fs::write(source.join("templates/mcp.json"), format!(r#"{{"mcpServers":{{"{name}":{{"command":"mcpctl","args":["run","{name}"]}}}}}}"#)).unwrap();
    fs::write(source.join("templates/config.toml"), format!("[mcp_servers.{name}]\ncommand = \"mcpctl\"\nargs = [\"run\", \"{name}\"]\n")).unwrap();
    let mut manifest = fs::read_to_string(source.join("mcpctl.toml")).unwrap();
    manifest.push_str(
        "\n[[scaffold.files]]\nfrom = \"templates/mcp.json\"\nto = \".mcp.json\"\nmerge = \"json\"\n\n\
         [[scaffold.files]]\nfrom = \"templates/config.toml\"\nto = \".codex/config.toml\"\nmerge = \"toml\"\n",
    );
    fs::write(source.join("mcpctl.toml"), manifest).unwrap();
}

/// Every file under `root`, keyed by relative path, with its contents.
fn snapshot(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut files = std::collections::BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.insert(path.strip_prefix(root).unwrap().to_path_buf(), fs::read(&path).unwrap());
            }
        }
    }
    files
}

#[test]
fn init_merges_every_mcps_entries_into_shared_config_and_a_second_run_changes_nothing() {
    let (workspace, project) = workspace_with_project();
    let state_home = workspace.join("state");
    add_shared_config(&workspace, "project-mcp", "project-mcp");
    add_shared_config(&workspace, "design-advisor-mcp", "design-advisor-mcp");
    fs::write(project.join(".mcp.json"), r#"{"mcpServers":{"mine":{"command":"my-server"}}}"#).unwrap();
    assert!(mcpctl(&project, &state_home, &["sync"]).status.success());

    let first = mcpctl(&project, &state_home, &["init"]);

    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));
    let json: serde_json::Value = serde_json::from_str(&fs::read_to_string(project.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(json["mcpServers"]["mine"]["command"], "my-server", "the user's own entry is kept");
    assert_eq!(json["mcpServers"]["project-mcp"]["args"], serde_json::json!(["run", "project-mcp"]));
    assert_eq!(json["mcpServers"]["design-advisor-mcp"]["args"], serde_json::json!(["run", "design-advisor-mcp"]));
    let toml: toml::Value = fs::read_to_string(project.join(".codex/config.toml")).unwrap().parse().unwrap();
    assert_eq!(toml["mcp_servers"]["project-mcp"]["command"].as_str(), Some("mcpctl"));
    assert_eq!(toml["mcp_servers"]["design-advisor-mcp"]["command"].as_str(), Some("mcpctl"));
    let after_first = snapshot(&project);

    let second = mcpctl(&project, &state_home, &["init"]);

    assert!(second.status.success(), "{}", String::from_utf8_lossy(&second.stderr));
    assert_eq!(snapshot(&project), after_first, "a repeated init changes no file");
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(!stdout.contains("Merged") && !stdout.contains("Copied"), "{stdout}");
}

#[test]
fn init_names_the_mcp_that_failed_and_still_initializes_the_rest() {
    let (workspace, project) = workspace_with_project();
    let state_home = workspace.join("state");
    add_shared_config(&workspace, "project-mcp", "project-mcp");
    fs::write(project.join(".mcp.json"), "{ not json").unwrap();
    assert!(mcpctl(&project, &state_home, &["sync"]).status.success());

    let output = mcpctl(&project, &state_home, &["init"]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("project-mcp@0.4.3") && stderr.contains(".mcp.json"), "{stderr}");
    assert!(!stderr.contains("design-advisor-mcp"), "only the failing MCP is blamed: {stderr}");
    assert_eq!(fs::read_to_string(project.join(".mcp.json")).unwrap(), "{ not json", "the unparseable file is left untouched");
    assert_eq!(
        fs::read_to_string(project.join("docs/design-advisor-mcp.md")).unwrap(),
        "design-advisor-mcp@0.2.0",
        "MCPs after the failing one are still initialized"
    );
}

#[test]
fn init_on_a_state_home_with_nothing_installed_names_each_project_mcp_and_suggests_sync() {
    let (workspace, project) = workspace_with_project();
    let state_home = workspace.join("fresh-state");

    let output = mcpctl(&project, &state_home, &["init"]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(!stderr.contains("cannot read registry"), "{stderr}");
    assert!(stderr.contains("mcp 'project-mcp' is not installed; run `mcpctl sync`"), "{stderr}");
    assert!(stderr.contains("mcp 'design-advisor-mcp' is not installed; run `mcpctl sync`"), "{stderr}");

    fs::write(project.join(".mcpctl.toml"), "[project]\nname = \"checkout-service\"\n").unwrap();

    let empty = mcpctl(&project, &state_home, &["init"]);

    assert!(empty.status.success(), "a project with no MCPs has nothing to initialize: {}", String::from_utf8_lossy(&empty.stderr));
}

#[test]
fn init_fails_naming_the_lock_file_when_the_project_lock_cannot_be_read() {
    let (workspace, project) = workspace_with_project();
    let state_home = workspace.join("state");
    assert!(mcpctl(&workspace, &state_home, &["install", "project-mcp"]).status.success());
    assert!(mcpctl(&workspace, &state_home, &["install", "design-advisor-mcp"]).status.success());
    fs::create_dir(project.join(".mcpctl.lock")).unwrap();

    let output = mcpctl(&project, &state_home, &["init"]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains(&format!("cannot read {}", project.join(".mcpctl.lock").display())), "{stderr}");
    assert!(!project.join("docs").exists(), "an unreadable lock must not fall back to the active versions");
}

#[test]
fn a_project_that_declares_no_mcps_syncs_and_initializes_without_error() {
    let workspace = temporary_dir();
    let state_home = workspace.join("state");
    let project = workspace.join("empty-service");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join(".mcpctl.toml"), "[project]\nname = \"empty-service\"\n").unwrap();

    let sync = mcpctl(&project, &state_home, &["sync"]);
    let init = mcpctl(&project, &state_home, &["init"]);

    assert!(sync.status.success(), "{}", String::from_utf8_lossy(&sync.stderr));
    assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));
    assert!(project.join(".mcpctl.lock").is_file(), "sync still records an (empty) lock");
}
