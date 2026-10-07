use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::project::{McpDeclaration, ProjectManifest};

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

/// The name of the first MCP whose entry differs between two locks, or that only one of them has.
pub fn first_lock_mismatch(locked: &ProjectLock, resolved: &ProjectLock) -> Option<String> {
    fn entry<'a>(lock: &'a ProjectLock, name: &str) -> Option<&'a LockedMcp> {
        lock.mcp.iter().find(|mcp| mcp.name == name)
    }
    let mut names: Vec<&str> = locked.mcp.iter().chain(&resolved.mcp).map(|mcp| mcp.name.as_str()).collect();
    names.sort_unstable();
    names.into_iter().find(|name| entry(locked, name) != entry(resolved, name)).map(str::to_owned)
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

/// Resolves every MCP the project declares; local sources are re-read so the lock always reflects them.
pub fn resolve_project_lock(manifest: &ProjectManifest, project_root: &Path) -> Result<ProjectLock, String> {
    let mcp = manifest
        .mcp
        .iter()
        .map(|declaration| resolve_locked_mcp(declaration, project_root))
        .collect::<Result<_, _>>()?;
    Ok(ProjectLock { version: LOCK_VERSION, mcp })
}

/// Writes `.mcpctl.lock` under `project_root` unless it already holds an equivalent lock.
/// Returns whether the file was written; a malformed existing lock is an error, never overwritten.
pub fn write_project_lock(project_root: &Path, lock: &ProjectLock) -> Result<bool, String> {
    let path = project_root.join(".mcpctl.lock");
    let rendered = render_project_lock(lock);
    match std::fs::read_to_string(&path) {
        Ok(existing) => {
            let existing = parse_project_lock(&existing).map_err(|error| format!("{} is malformed: {error}", path.display()))?;
            if render_project_lock(&existing) == rendered {
                return Ok(false);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(".tmp-{}", std::process::id()));
    let temporary = std::path::PathBuf::from(temporary);
    let written = std::fs::write(&temporary, &rendered).and_then(|()| std::fs::rename(&temporary, &path));
    written.map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        format!("cannot write {}: {error}", path.display())
    })?;
    Ok(true)
}
