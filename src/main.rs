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
            let state_home = mcp_cli::paths::data_home_from(
                std::env::var_os("MCPCTL_HOME").map(std::path::PathBuf::from),
            )
            .unwrap_or_else(|error| fatal(error));
            let package_name = manifest.name.clone();
            let package_version = manifest.version.clone();
            if state_home.join("packages").join(&package_name).join(&package_version).exists() {
                fatal(format!("{package_name}@{package_version} is already installed; installed versions are immutable"));
            }
            if matches!(manifest.runtime, mcp_cli::manifest::Runtime::Binary) {
                let binary = std::path::Path::new(project).join(&manifest.install.entrypoint);
                if !binary.is_file() { fatal(format!("Python MCPs and binary MCPs require an entrypoint: {}", binary.display())); }
                #[cfg(unix)]
                if !std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&binary).unwrap_or_else(|error| fatal(error.to_string())).permissions()).eq(&0) {
                    let mode = std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&binary).unwrap_or_else(|error| fatal(error.to_string())).permissions());
                    if mode & 0o111 == 0 { fatal("binary entrypoint is not executable".to_owned()); }
                }
                let root = state_home.join("packages").join(&package_name).join(&package_version);
                let runtime_bin = root.join("runtime/bin");
                std::fs::create_dir_all(&runtime_bin).unwrap_or_else(|error| fatal(format!("cannot create binary runtime: {error}")));
                copy_source_snapshot(std::path::Path::new(project), &root.join("source")).unwrap_or_else(|error| fatal(format!("cannot snapshot source: {error}")));
                let installed_binary = runtime_bin.join(&manifest.install.entrypoint);
                if let Some(parent) = installed_binary.parent() {
                    std::fs::create_dir_all(parent).unwrap_or_else(|error| fatal(format!("cannot create binary runtime: {error}")));
                }
                std::fs::copy(&binary, &installed_binary).unwrap_or_else(|error| fatal(format!("cannot copy binary: {error}")));
                if let Err(error) = record_install(&state_home, &package_name, &package_version, "binary") { let _ = std::fs::remove_dir_all(&root); fatal(format!("cannot record installation: {error}")); }
                return;
            }
            let mcp_cli::manifest::Runtime::Python { python } = manifest.runtime else { unreachable!() };
            if std::process::Command::new("uv").arg("--version").status().map(|status| !status.success()).unwrap_or(true) { fatal("uv is required to install Python MCPs".to_owned()); }
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
            let status = std::process::Command::new("uv")
                .args(["pip", "install", "--python", runtime.join("bin/python").to_str().unwrap(), destination.to_str().unwrap()])
                .status()
                .unwrap_or_else(|error| fatal(format!("cannot run uv: {error}")));
            if !status.success() {
                let _ = std::fs::remove_dir_all(destination.parent().unwrap());
                let package_dir = destination.parent().unwrap().parent().unwrap();
                if std::fs::read_dir(package_dir).ok().is_some_and(|mut entries| entries.next().is_none()) {
                    let _ = std::fs::remove_dir(package_dir);
                }
                fatal("uv failed to install Python MCP dependencies".to_owned());
            }
            if let Err(error) = record_install(&state_home, &package_name, &package_version, "python") {
                let _ = std::fs::remove_dir_all(destination.parent().unwrap());
                fatal(format!("cannot record installation: {error}"));
            }
        }
        [command, package, arguments @ ..] if command == "run" => run_package(package, arguments),
        [command] if command == "list" => list_packages(),
        [command, package] if command == "info" => info_package(package),
        [command, package] if command == "uninstall" => uninstall_package(package),
        [command, package] if command == "use" => use_package(package),
        [command, package, flag, source] if command == "update" && flag == "--source" => update_package(package, source),
        [command] if command == "doctor" => doctor(),
        _ => {
            eprintln!("unrecognized argument");
            std::process::exit(2);
        }
    }
}

fn update_package(name: &str, source: &str) {
    let manifest_path = std::path::Path::new(source).join("mcpctl.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .map_err(|error| error.to_string())
        .and_then(|contents| mcp_cli::manifest::parse_manifest(&contents))
        .unwrap_or_else(|error| fatal(format!("cannot read update source: {error}")));
    if manifest.name != name { fatal(format!("update source package {} does not match {name}", manifest.name)); }
    let status = std::process::Command::new(std::env::current_exe().unwrap_or_else(|error| fatal(error.to_string())))
        .args(["install", source]).status().unwrap_or_else(|error| fatal(format!("cannot install update: {error}")));
    if !status.success() { std::process::exit(status.code().unwrap_or(1)); }
    use_package(&format!("{name}@{}", manifest.version));
}

fn doctor() {
    let state_home = match mcp_cli::paths::data_home_from(std::env::var_os("MCPCTL_HOME").map(std::path::PathBuf::from)) { Ok(path) => path, Err(error) => fatal(error) };
    let registry_path = state_home.join("registry.json");
    let registry = match mcp_cli::registry::load_registry(&registry_path) { Ok(registry) => registry, Err(error) => { println!("ERROR registry: {error}"); std::process::exit(1) } };
    let mut errors = 0;
    for (name, package) in &registry.packages {
        if let Some(active) = &package.active_version {
            if !package.versions.iter().any(|version| &version.version == active) { println!("ERROR {name}: active version {active} is not installed"); errors += 1; }
        }
        for version in &package.versions {
            let root = state_home.join("packages").join(name).join(&version.version);
            let manifest_path = root.join("source/mcpctl.toml");
            let manifest = std::fs::read_to_string(&manifest_path).ok().and_then(|contents| mcp_cli::manifest::parse_manifest(&contents).ok());
            let Some(manifest) = manifest else { println!("ERROR {name}@{}: missing or invalid manifest", version.version); errors += 1; continue };
            let entrypoint = match manifest.runtime {
                mcp_cli::manifest::Runtime::Python { .. } => root.join("runtime/bin/python"),
                mcp_cli::manifest::Runtime::Binary => root.join("runtime/bin").join(manifest.install.entrypoint),
            };
            if !entrypoint.is_file() { println!("ERROR {name}@{}: missing entrypoint", version.version); errors += 1; continue }
            #[cfg(unix)]
            if std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&entrypoint).unwrap().permissions()) & 0o111 == 0 { println!("ERROR {name}@{}: entrypoint is not executable", version.version); errors += 1; }
        }
    }
    if errors == 0 { println!("OK: registry and installed entrypoints are healthy"); } else { std::process::exit(1); }
}

fn use_package(selector: &str) {
    let Some((name, version)) = selector.split_once('@') else { fatal("use requires name@version".to_owned()) };
    if name.is_empty() || version.is_empty() || name.contains(['/', '\\']) || version.contains(['/', '\\']) {
        fatal("invalid package selector".to_owned());
    }
    let state_home = mcp_cli::paths::data_home_from(std::env::var_os("MCPCTL_HOME").map(std::path::PathBuf::from)).unwrap_or_else(|error| fatal(error));
    let registry_path = state_home.join("registry.json");
    let mut registry = mcp_cli::registry::load_registry(&registry_path).unwrap_or_else(|error| fatal(format!("cannot read registry: {error}")));
    let package = registry.packages.get_mut(name).unwrap_or_else(|| fatal(format!("{name} is not installed")));
    if !package.versions.iter().any(|installed| installed.version == version) { fatal(format!("{name}@{version} is not installed")); }
    package.active_version = Some(version.to_owned());
    mcp_cli::registry::save_registry(&registry_path, &registry).unwrap_or_else(|error| fatal(error));
}

fn uninstall_package(selector: &str) {
    let Some((name, version)) = selector.split_once('@') else { fatal("uninstall requires name@version".to_owned()) };
    if name.is_empty() || version.is_empty() || name.contains(['/', '\\']) || version.contains(['/', '\\']) {
        fatal("invalid package selector".to_owned());
    }
    let state_home = mcp_cli::paths::data_home_from(std::env::var_os("MCPCTL_HOME").map(std::path::PathBuf::from)).unwrap_or_else(|error| fatal(error));
    let registry_path = state_home.join("registry.json");
    let mut registry = mcp_cli::registry::load_registry(&registry_path).unwrap_or_else(|error| fatal(format!("cannot read registry: {error}")));
    let package = registry.packages.get_mut(name).unwrap_or_else(|| fatal(format!("{name} is not installed")));
    if package.active_version.as_deref() == Some(version) && package.versions.len() > 1 { fatal(format!("{name}@{version} is active; select another version first")); }
    let before = package.versions.len(); package.versions.retain(|installed| installed.version != version);
    if package.versions.len() == before { fatal(format!("{name}@{version} is not installed")); }
    if package.active_version.as_deref() == Some(version) { package.active_version = None; }
    mcp_cli::registry::save_registry(&registry_path, &registry).unwrap_or_else(|error| fatal(error));
    std::fs::remove_dir_all(state_home.join("packages").join(name).join(version)).unwrap_or_else(|error| fatal(format!("cannot remove {name}@{version}: {error}")));
}

fn info_package(name: &str) {
    let state_home = mcp_cli::paths::data_home_from(std::env::var_os("MCPCTL_HOME").map(std::path::PathBuf::from)).unwrap_or_else(|error| fatal(error));
    let registry = mcp_cli::registry::load_registry(&state_home.join("registry.json")).unwrap_or_else(|error| fatal(format!("cannot read registry: {error}")));
    let package = registry.packages.get(name).unwrap_or_else(|| fatal(format!("{name} is not installed")));
    for version in &package.versions {
        println!("{name}\t{}\t{}\t{}", version.version, version.runtime, if package.active_version.as_deref() == Some(&version.version) { "active" } else { "inactive" });
    }
}

fn list_packages() {
    let state_home = mcp_cli::paths::data_home_from(std::env::var_os("MCPCTL_HOME").map(std::path::PathBuf::from))
        .unwrap_or_else(|error| fatal(error));
    let registry = mcp_cli::registry::load_registry(&state_home.join("registry.json"))
        .unwrap_or_else(|error| fatal(format!("cannot read registry: {error}")));
    for (name, package) in registry.packages {
        let versions = package.versions.iter().map(|version| version.version.as_str()).collect::<Vec<_>>().join(", ");
        let runtime = package.active_version.as_ref().and_then(|active| package.versions.iter().find(|version| &version.version == active)).map(|version| version.runtime.as_str()).unwrap_or("unknown");
        println!("{name}\t{}\t{versions}\t{runtime}", package.active_version.unwrap_or_else(|| "-".to_owned()));
    }
}

fn run_package(package: &str, arguments: &[String]) {
    let (name, requested_version) = package.split_once('@').map_or((package, None), |(name, version)| (name, Some(version)));
    if requested_version == Some("") {
        fatal("run version is required after @".to_owned());
    }
    if name.is_empty() || name.contains('/') || name.contains('\\') || requested_version.is_some_and(|version| version.contains('/') || version.contains('\\')) {
        fatal("invalid package selector".to_owned());
    }
    let state_home = mcp_cli::paths::data_home_from(
        std::env::var_os("MCPCTL_HOME").map(std::path::PathBuf::from),
    )
    .unwrap_or_else(|error| fatal(error));
    let registry_path = state_home.join("registry.json");
    if !registry_path.exists() {
        fatal(format!("{name} is not installed"));
    }
    let registry = mcp_cli::registry::load_registry(&registry_path)
        .unwrap_or_else(|error| fatal(format!("cannot read registry: {error}")));
    let package = registry.packages.get(name).unwrap_or_else(|| fatal(format!("{name} is not installed")));
    let version = requested_version
        .map(str::to_owned)
        .or_else(|| package.active_version.clone())
        .unwrap_or_else(|| fatal(format!("{name} has no active version")));
    if !package.versions.iter().any(|installed| installed.version == version) {
        fatal(format!("{name}@{version} is not installed"));
    }
    let install_root = state_home.join("packages").join(name).join(&version);
    let manifest = std::fs::read_to_string(install_root.join("source/mcpctl.toml"))
        .map_err(|error| error.to_string())
        .and_then(|contents| mcp_cli::manifest::parse_manifest(&contents))
        .unwrap_or_else(|error| fatal(format!("cannot read installed manifest: {error}")));
    let runtime_bin = install_root.join("runtime/bin");
    let mut path_entries = vec![runtime_bin.clone()];
    if let Some(path) = std::env::var_os("PATH") {
        path_entries.extend(std::env::split_paths(&path));
    }
    let mut command = std::process::Command::new(runtime_bin.join(manifest.install.entrypoint));
    command.args(arguments).env("PATH", std::env::join_paths(path_entries).unwrap()).env_remove("VIRTUAL_ENV").env_remove("PYTHONHOME");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = command.exec();
        fatal(format!("cannot run {name}@{version}: {error}"));
    }
    #[cfg(not(unix))]
    {
        let status = command.status().unwrap_or_else(|error| fatal(format!("cannot run {name}@{version}: {error}")));
        std::process::exit(status.code().unwrap_or(1));
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

fn record_install(state_home: &std::path::Path, name: &str, version: &str, runtime: &str) -> Result<(), String> {
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
            runtime: runtime.to_owned(),
            source: "local".to_owned(),
            installed_at: "installed".to_owned(),
        });
    }
    mcp_cli::registry::save_registry(&path, &registry)
}
