use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
pub struct PackageManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub runtime: Runtime,
    pub install: Install,
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

pub fn parse_manifest(contents: &str) -> Result<PackageManifest, String> {
    let manifest: PackageManifest = toml::from_str(contents).map_err(|error| error.to_string())?;
    if manifest.name.is_empty()
        || !manifest
            .name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("manifest name must use only letters, digits, hyphens, or underscores".to_owned());
    }
    semver::Version::parse(&manifest.version)
        .map_err(|_| "manifest version must be valid semver".to_owned())?;
    if let Runtime::Python { python } = &manifest.runtime {
        if python.trim().is_empty() {
            return Err("python manifests require a runtime.python constraint".to_owned());
        }
        if manifest.install.strategy.as_deref() != Some("uv") {
            return Err("python manifests require install.strategy = \"uv\"".to_owned());
        }
    }
    Ok(manifest)
}

pub fn parse_binary_manifest(contents: &str) -> Result<PackageManifest, String> {
    let manifest = parse_manifest(contents).map_err(|error| error.to_string())?;
    if matches!(manifest.runtime, Runtime::Binary) {
        Ok(manifest)
    } else {
        Err("manifest runtime must be binary".to_owned())
    }
}
