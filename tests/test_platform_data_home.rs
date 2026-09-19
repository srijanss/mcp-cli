use std::path::PathBuf;

use mcp_cli::paths::{non_macos_data_home_from, platform_data_home_from};

#[test]
fn platform_resolver_uses_standard_data_directory() {
    let home = PathBuf::from("/Users/alice");
    let data_home = platform_data_home_from(Some(home.clone()), None).unwrap();

    #[cfg(target_os = "macos")]
    assert_eq!(data_home, home.join("Library/Application Support/mcpctl"));

    #[cfg(not(target_os = "macos"))]
    assert_eq!(data_home, home.join(".local/share/mcpctl"));
}

#[cfg(not(target_os = "macos"))]
#[test]
fn platform_resolver_uses_xdg_data_home_when_provided() {
    assert_eq!(
        platform_data_home_from(Some(PathBuf::from("/home/alice")), Some(PathBuf::from("/srv/data")))
            .unwrap(),
        PathBuf::from("/srv/data/mcpctl")
    );
}

#[test]
fn non_macos_resolver_uses_xdg_data_home_without_home() {
    assert_eq!(
        non_macos_data_home_from(None, Some(PathBuf::from("/srv/data"))).unwrap(),
        PathBuf::from("/srv/data/mcpctl")
    );
}

#[test]
fn non_macos_resolver_falls_back_to_home_when_xdg_is_absent() {
    assert_eq!(
        non_macos_data_home_from(Some(PathBuf::from("/home/alice")), None).unwrap(),
        PathBuf::from("/home/alice/.local/share/mcpctl")
    );
}

#[test]
fn non_macos_resolver_rejects_relative_xdg_data_home() {
    assert!(non_macos_data_home_from(Some(PathBuf::from("/home/alice")), Some(PathBuf::from("data")))
        .is_err());
}

#[test]
fn non_macos_resolver_rejects_relative_xdg_without_home() {
    assert!(non_macos_data_home_from(None, Some(PathBuf::from("data"))).is_err());
}
