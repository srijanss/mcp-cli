use mcp_cli::project_lock::parse_project_lock;

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
