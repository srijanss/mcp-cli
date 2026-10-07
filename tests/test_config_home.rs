use std::path::PathBuf;

use mcp_cli::paths::config_home_from;

#[test]
fn config_home_prefers_mcpctl_config_home_then_the_platform_config_directory() {
    let home = Some(PathBuf::from("/home/alice"));
    let xdg = Some(PathBuf::from("/srv/config"));

    assert_eq!(
        config_home_from(Some(PathBuf::from("/opt/mcpctl-config")), home.clone(), xdg.clone()).unwrap(),
        PathBuf::from("/opt/mcpctl-config")
    );
    assert!(config_home_from(Some(PathBuf::from("relative")), home.clone(), xdg.clone())
        .unwrap_err()
        .contains("MCPCTL_CONFIG_HOME must be an absolute path"));

    #[cfg(target_os = "macos")]
    assert_eq!(
        config_home_from(None, home.clone(), xdg.clone()).unwrap(),
        PathBuf::from("/home/alice/Library/Application Support/mcpctl")
    );

    #[cfg(not(target_os = "macos"))]
    {
        assert_eq!(config_home_from(None, home.clone(), xdg).unwrap(), PathBuf::from("/srv/config/mcpctl"));
        assert_eq!(config_home_from(None, home.clone(), None).unwrap(), PathBuf::from("/home/alice/.config/mcpctl"));
        assert!(config_home_from(None, home, Some(PathBuf::from("config")))
            .unwrap_err()
            .contains("XDG_CONFIG_HOME must be an absolute path"));
    }

    assert!(config_home_from(None, None, None).unwrap_err().contains("HOME is not set"));
}
