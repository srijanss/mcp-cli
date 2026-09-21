use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
pub struct PackageManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub runtime: Runtime,
    pub install: Install,
    pub scaffold: Option<Scaffold>,
}

/// Files a package ships for consumer projects, copied out by `mcpctl init`.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Scaffold {
    #[serde(default)]
    pub files: Vec<ScaffoldFile>,
    #[serde(default)]
    pub dirs: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub hints: Vec<ScaffoldHint>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct ScaffoldFile {
    pub from: String,
    pub to: String,
    /// When the destination already exists, merge the template into it instead of skipping it.
    pub merge: Option<MergeMode>,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum MergeMode {
    Json,
    Toml,
}

/// A message printed after `init`; when `when_exists` is set, only if that path exists in the target.
#[derive(Debug, Deserialize, PartialEq)]
pub struct ScaffoldHint {
    pub when_exists: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Runtime {
    Python { python: String },
    Binary,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Install {
    pub strategy: Option<String>,
    pub entrypoint: String,
}

fn is_valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let starts_ok = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
    starts_ok
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

pub fn parse_manifest(contents: &str) -> Result<PackageManifest, String> {
    let manifest: PackageManifest = toml::from_str(contents).map_err(|error| error.to_string())?;
    if !is_valid_name(&manifest.name) {
        return Err(
            "manifest name must match [a-z0-9][a-z0-9._-]* (lowercase letters, digits, dots, underscores, hyphens)"
                .to_owned(),
        );
    }
    let version = semver::Version::parse(&manifest.version)
        .map_err(|_| "manifest version must be valid semver".to_owned())?;
    if !version.build.is_empty() {
        return Err("manifest version must not include semver build metadata".to_owned());
    }
    if let Runtime::Python { python } = &manifest.runtime {
        if python.trim().is_empty() {
            return Err("python manifests require a runtime.python constraint".to_owned());
        }
        if manifest.install.strategy.as_deref() != Some("uv") {
            return Err("python manifests require install.strategy = \"uv\"".to_owned());
        }
    }
    if matches!(manifest.runtime, Runtime::Binary) && !is_safe_relative_path(&manifest.install.entrypoint) {
        return Err(
            "binary entrypoint must be a non-empty source-relative path without '..' segments or absolute paths"
                .to_owned(),
        );
    }
    if let Some(scaffold) = &manifest.scaffold {
        let paths = scaffold.dirs.iter().chain(&scaffold.exclude)
            .chain(scaffold.files.iter().flat_map(|file| [&file.from, &file.to]))
            .chain(scaffold.hints.iter().filter_map(|hint| hint.when_exists.as_ref()));
        for path in paths {
            if !is_safe_relative_path(path) {
                return Err(format!(
                    "scaffold path {path:?} must be a non-empty relative path without '..' segments or absolute paths"
                ));
            }
        }
    }
    Ok(manifest)
}

fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && path.split('/').all(|segment| !segment.is_empty() && segment != "..")
        && !path.ends_with("/.")
        && path != "."
}

pub fn parse_binary_manifest(contents: &str) -> Result<PackageManifest, String> {
    let manifest = parse_manifest(contents).map_err(|error| error.to_string())?;
    if matches!(manifest.runtime, Runtime::Binary) {
        Ok(manifest)
    } else {
        Err("manifest runtime must be binary".to_owned())
    }
}
