use std::path::PathBuf;

pub fn data_home_from(home_override: Option<PathBuf>) -> Result<PathBuf, String> {
    match home_override {
        Some(path) if path.is_absolute() => Ok(path),
        Some(_) => Err("MCPCTL_HOME must be an absolute path".to_owned()),
        None => platform_data_home_from(
            std::env::var_os("HOME").map(PathBuf::from),
            std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
        ),
    }
}

pub fn platform_data_home_from(
    home: Option<PathBuf>,
    xdg_data_home: Option<PathBuf>,
) -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        let _ = xdg_data_home;
        let home = home.ok_or_else(|| "HOME is not set".to_owned())?;
        Ok(home.join("Library/Application Support/mcpctl"))
    }

    #[cfg(not(target_os = "macos"))]
    {
        non_macos_data_home_from(home, xdg_data_home)
    }
}

pub fn non_macos_data_home_from(
    home: Option<PathBuf>,
    xdg_data_home: Option<PathBuf>,
) -> Result<PathBuf, String> {
    match xdg_data_home {
        Some(path) if path.is_absolute() => Ok(path.join("mcpctl")),
        Some(_) => Err("XDG_DATA_HOME must be an absolute path".to_owned()),
        None => Ok(home
            .ok_or_else(|| "HOME is not set".to_owned())?
            .join(".local/share/mcpctl")),
    }
}

/// Where user configuration such as `catalog.toml` lives; separate from the data home.
pub fn config_home_from(
    home_override: Option<PathBuf>,
    home: Option<PathBuf>,
    xdg_config_home: Option<PathBuf>,
) -> Result<PathBuf, String> {
    match home_override {
        Some(path) if path.is_absolute() => return Ok(path),
        Some(_) => return Err("MCPCTL_CONFIG_HOME must be an absolute path".to_owned()),
        None => {}
    }

    #[cfg(target_os = "macos")]
    {
        let _ = xdg_config_home;
        let home = home.ok_or_else(|| "HOME is not set".to_owned())?;
        Ok(home.join("Library/Application Support/mcpctl"))
    }

    #[cfg(not(target_os = "macos"))]
    match xdg_config_home {
        Some(path) if path.is_absolute() => Ok(path.join("mcpctl")),
        Some(_) => Err("XDG_CONFIG_HOME must be an absolute path".to_owned()),
        None => Ok(home.ok_or_else(|| "HOME is not set".to_owned())?.join(".config/mcpctl")),
    }
}

pub fn ensure_state_layout(state_home: &std::path::Path) -> std::io::Result<()> {
    for directory in ["packages", "active", "cache"] {
        std::fs::create_dir_all(state_home.join(directory))?;
    }

    let registry_path = state_home.join("registry.json");
    if registry_path.exists() && !registry_path.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "registry.json must be a regular file",
        ));
    }
    if !registry_path.exists() {
        crate::registry::save_registry(&registry_path, &crate::registry::Registry::new())
            .map_err(std::io::Error::other)?;
    }

    Ok(())
}
