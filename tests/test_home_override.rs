use std::path::PathBuf;

use mcp_cli::paths::data_home_from;

#[test]
fn absolute_home_override_is_used_and_relative_override_is_rejected() {
    let override_home = PathBuf::from("/tmp/mcpctl-state");

    assert_eq!(data_home_from(Some(override_home.clone())).unwrap(), override_home);
    assert!(data_home_from(Some(PathBuf::from("relative-state"))).is_err());
}

#[test]
fn home_override_returns_the_exact_absolute_path() {
    let override_home = PathBuf::from("/var/tmp/mcpctl-state");

    assert_eq!(data_home_from(Some(override_home.clone())).unwrap(), override_home);
}
