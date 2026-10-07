use mcp_cli::project_lock::{parse_project_lock, render_project_lock, LockedMcp, ProjectLock};

fn locked(name: &str, version: &str) -> LockedMcp {
    LockedMcp {
        name: name.to_owned(),
        version: version.to_owned(),
        source: format!("../{name}"),
        manifest_digest: format!("sha256:{name}-digest"),
    }
}

#[test]
fn project_lock_records_exact_mcp_versions() {
    let lock = parse_project_lock(
        r#"
version = 1

[[mcp]]
name = "project-mcp"
version = "0.4.3"
source = "../project-mcp"
manifest_digest = "sha256:abc123"
"#,
    )
    .unwrap();

    assert_eq!(lock.version, 1);
    assert_eq!(lock.mcp.len(), 1);
    assert_eq!(lock.mcp[0].name, "project-mcp");
    assert_eq!(lock.mcp[0].version, "0.4.3");
    assert_eq!(lock.mcp[0].source, "../project-mcp");
    assert_eq!(lock.mcp[0].manifest_digest, "sha256:abc123");
}

#[test]
fn project_lock_with_unsupported_version_fails_clearly() {
    let error = parse_project_lock("version = 2\nmcp = []\n").unwrap_err();

    assert!(error.contains("unsupported lock version 2"), "{error}");
}

#[test]
fn project_lock_with_non_exact_mcp_version_fails_naming_the_mcp() {
    let error = parse_project_lock(
        r#"
version = 1

[[mcp]]
name = "project-mcp"
version = "^0.4"
source = "../project-mcp"
manifest_digest = "sha256:abc123"
"#,
    )
    .unwrap_err();

    assert!(error.contains("project-mcp"), "{error}");
    assert!(error.contains("^0.4"), "{error}");
}

#[test]
fn malformed_project_lock_fails_clearly() {
    let error = parse_project_lock("version = 1\n[[mcp]\nname = ").unwrap_err();

    assert!(!error.is_empty());
}

#[test]
fn same_lock_renders_identically_regardless_of_entry_order_and_round_trips() {
    let forward = ProjectLock { version: 1, mcp: vec![locked("design-advisor-mcp", "0.2.1"), locked("project-mcp", "0.4.3")] };
    let reversed = ProjectLock { version: 1, mcp: vec![locked("project-mcp", "0.4.3"), locked("design-advisor-mcp", "0.2.1")] };

    let rendered = render_project_lock(&reversed);

    assert_eq!(rendered, render_project_lock(&forward));
    assert_eq!(parse_project_lock(&rendered).unwrap(), forward);
}

#[test]
fn rendered_lock_has_a_fixed_committable_format() {
    let lock = ProjectLock { version: 1, mcp: vec![locked("project-mcp", "0.4.3")] };

    assert_eq!(
        render_project_lock(&lock),
        r#"version = 1

[[mcp]]
name = "project-mcp"
version = "0.4.3"
source = "../project-mcp"
manifest_digest = "sha256:project-mcp-digest"
"#
    );
}
