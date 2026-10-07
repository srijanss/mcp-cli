use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::project::McpDeclaration;

const LOCK_VERSION: u32 = 1;

/// A project's `.mcpctl.lock`: the exact MCP versions resolved from `.mcpctl.toml`.
#[derive(Debug, Deserialize, PartialEq)]
pub struct ProjectLock {
    pub version: u32,
    pub mcp: Vec<LockedMcp>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
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

#[derive(Serialize)]
struct SortedLock<'a> {
    version: u32,
    mcp: Vec<&'a LockedMcp>,
}

/// Renders the lock with entries sorted by name, so equal locks always produce identical text.
pub fn render_project_lock(lock: &ProjectLock) -> String {
    let mut mcp: Vec<&LockedMcp> = lock.mcp.iter().collect();
    mcp.sort_by(|left, right| left.name.cmp(&right.name));
    toml::to_string(&SortedLock { version: lock.version, mcp }).expect("lock entries are plain strings")
}

/// Pins a local-source declaration to the exact package version found at its source.
pub fn resolve_locked_mcp(declaration: &McpDeclaration, project_root: &Path) -> Result<LockedMcp, String> {
    let source = declaration.resolve_local_source(project_root)?;
    let manifest_path = source.join("mcpctl.toml");
    let manifest_bytes = std::fs::read(&manifest_path)
        .map_err(|error| format!("cannot read {}: {error}", manifest_path.display()))?;
    let manifest = crate::manifest::parse_manifest(&String::from_utf8_lossy(&manifest_bytes))
        .map_err(|error| format!("mcp '{}' source manifest is invalid: {error}", declaration.name))?;
    if manifest.name != declaration.name {
        return Err(format!(
            "mcp '{}' source {} contains package '{}'",
            declaration.name,
            source.display(),
            manifest.name
        ));
    }
    let constraint = semver::VersionReq::parse(&declaration.version).map_err(|error| error.to_string())?;
    let version = semver::Version::parse(&manifest.version).map_err(|error| error.to_string())?;
    if !constraint.matches(&version) {
        return Err(format!(
            "mcp '{}' source version {version} does not satisfy constraint '{}'",
            declaration.name, declaration.version
        ));
    }
    Ok(LockedMcp {
        name: declaration.name.clone(),
        version: manifest.version,
        source: declaration.source.clone(),
        manifest_digest: format!("sha256:{}", crate::metadata::sha256_hex(&manifest_bytes)),
    })
}
