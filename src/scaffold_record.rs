//! `.mcpctl.scaffold.json`: for every scaffold file `init` copied into a project, the MCP and version that
//! copied it and the file's sha256 at the time, so a later scaffold can tell untouched files from local edits.
use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = ".mcpctl.scaffold.json";
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScaffoldRecord {
    pub version: u32,
    /// Keyed by `/`-separated project-relative path; sorted so the file renders deterministically.
    pub files: BTreeMap<String, RecordedFile>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordedFile {
    pub mcp: String,
    pub version: String,
    pub sha256: String,
}

impl Default for ScaffoldRecord {
    fn default() -> Self {
        Self { version: SCHEMA_VERSION, files: BTreeMap::new() }
    }
}

/// The record at `root`, or an empty one when the project has none yet.
pub fn load(root: &Path) -> Result<ScaffoldRecord, String> {
    let path = root.join(FILE_NAME);
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(ScaffoldRecord::default()),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    let record: ScaffoldRecord = serde_json::from_str(&contents).map_err(|error| format!("{} is malformed: {error}", path.display()))?;
    if record.version != SCHEMA_VERSION {
        return Err(format!("{} has unsupported version {}", path.display(), record.version));
    }
    Ok(record)
}

/// Writes `record` to `root` unless the file there already holds exactly that, returning whether it wrote.
pub fn save_if_changed(root: &Path, record: &ScaffoldRecord) -> Result<bool, String> {
    let path = root.join(FILE_NAME);
    let rendered = serde_json::to_string_pretty(record).map_err(|error| error.to_string())? + "\n";
    if std::fs::read(&path).is_ok_and(|existing| existing == rendered.as_bytes()) {
        return Ok(false);
    }
    // Write beside the record and rename over it, so an interrupted write never leaves a truncated record.
    let partial = root.join(format!("{FILE_NAME}.tmp"));
    write_new(&partial, rendered.as_bytes())
        .and_then(|()| std::fs::rename(&partial, &path))
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    Ok(true)
}

/// Writes `bytes` to a freshly created file at `path`. Whatever was left there before (a stale temporary
/// from an interrupted run, or a symlink planted in the project) is unlinked, never written through.
fn write_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    std::fs::OpenOptions::new().write(true).create_new(true).open(path)?.write_all(bytes)
}
