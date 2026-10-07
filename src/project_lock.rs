use serde::Deserialize;

const LOCK_VERSION: u32 = 1;

/// A project's `.mcpctl.lock`: the exact MCP versions resolved from `.mcpctl.toml`.
#[derive(Debug, Deserialize, PartialEq)]
pub struct ProjectLock {
    pub version: u32,
    pub mcp: Vec<LockedMcp>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct LockedMcp {
    pub name: String,
    pub version: String,
    pub source: String,
    pub manifest_digest: String,
}

pub fn parse_project_lock(contents: &str) -> Result<ProjectLock, String> {
    let lock: ProjectLock = toml::from_str(contents).map_err(|error| error.to_string())?;
    if lock.version != LOCK_VERSION {
        return Err(format!("unsupported lock version {} (expected {LOCK_VERSION})", lock.version));
    }
    for mcp in &lock.mcp {
        semver::Version::parse(&mcp.version).map_err(|error| {
            format!("locked mcp '{}' has non-exact version '{}': {error}", mcp.name, mcp.version)
        })?;
    }
    Ok(lock)
}
