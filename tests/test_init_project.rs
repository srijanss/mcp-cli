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
