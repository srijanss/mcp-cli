fn main() {
    match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [flag] if flag == "--version" => println!("mcpctl 0.1.0"),
        [flag] if flag == "--help" || flag == "-h" => println!(
            "Usage: mcpctl <COMMAND>\n\nCommands:\n  install\n  run\n  list\n  info\n  uninstall\n  use\n  update\n  doctor"
        ),
        [] => {
            eprintln!("Usage: mcpctl <COMMAND>");
            std::process::exit(2);
        }
        [command] if command == "install" => {
            eprintln!("install requires a local project path");
            std::process::exit(2);
        }
        [command, project] if command == "install" => {
            let manifest_path = std::path::Path::new(project).join("mcpctl.toml");
            let contents = std::fs::read_to_string(&manifest_path)
                .unwrap_or_else(|error| fatal(format!("cannot read {}: {error}", manifest_path.display())));
            let manifest = mcp_cli::manifest::parse_manifest(&contents)
                .unwrap_or_else(|error| fatal(format!("invalid manifest: {error}")));
            let mcp_cli::manifest::Runtime::Python { python } = manifest.runtime else {
                fatal("install currently supports Python MCPs only".to_owned());
            };
            if std::process::Command::new("uv")
                .arg("--version")
                .status()
                .map(|status| !status.success())
                .unwrap_or(true)
            {
                fatal("uv is required to install Python MCPs".to_owned());
            }
            let state_home = mcp_cli::paths::data_home_from(
                std::env::var_os("MCPCTL_HOME").map(std::path::PathBuf::from),
            )
            .unwrap_or_else(|error| fatal(error));
            let package_name = manifest.name.clone();
            let package_version = manifest.version.clone();
            let destination = state_home
                .join("packages")
                .join(&package_name)
                .join(&package_version)
                .join("source");
            copy_source_snapshot(std::path::Path::new(project), &destination)
                .unwrap_or_else(|error| fatal(format!("cannot snapshot source: {error}")));
            let runtime = destination.parent().unwrap().join("runtime");
            let status = std::process::Command::new("uv")
                .args(["venv", runtime.to_str().unwrap(), "--python", &python])
                .status()
                .unwrap_or_else(|error| fatal(format!("cannot run uv: {error}")));
            if !status.success() {
                let _ = std::fs::remove_dir_all(destination.parent().unwrap());
                fatal("uv failed to create the isolated runtime".to_owned());
            }
            if let Err(error) = record_install(&state_home, &package_name, &package_version) {
                let _ = std::fs::remove_dir_all(destination.parent().unwrap());
                fatal(format!("cannot record installation: {error}"));
            }
        }
        _ => {
            eprintln!("unrecognized argument");
            std::process::exit(2);
        }
    }
}

fn fatal(message: String) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

fn copy_source_snapshot(source: &std::path::Path, destination: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if destination.starts_with(&path)
            || matches!(name.to_str(), Some(".git" | ".venv" | "__pycache__" | "target"))
        {
            continue;
        }
        let target = destination.join(&name);
        if entry.file_type()?.is_dir() {
            copy_source_snapshot(&path, &target)?;
        } else if entry.file_type()?.is_file() {
            std::fs::copy(path, target)?;
        }
    }
    Ok(())
}

fn record_install(state_home: &std::path::Path, name: &str, version: &str) -> Result<(), String> {
    std::fs::create_dir_all(state_home).map_err(|error| error.to_string())?;
    let path = state_home.join("registry.json");
    let mut registry = if path.exists() {
        mcp_cli::registry::load_registry(&path)?
    } else {
        mcp_cli::registry::Registry::new()
    };
    let package = registry.packages.entry(name.to_owned()).or_insert_with(|| mcp_cli::registry::PackageRecord {
        active_version: Some(version.to_owned()),
        versions: Vec::new(),
    });
    if !package.versions.iter().any(|installed| installed.version == version) {
        package.versions.push(mcp_cli::registry::InstalledVersion {
            version: version.to_owned(),
            runtime: "python".to_owned(),
            source: "local".to_owned(),
            installed_at: "installed".to_owned(),
        });
    }
    mcp_cli::registry::save_registry(&path, &registry)
}
