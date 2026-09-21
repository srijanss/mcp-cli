fn main() {
    use clap::Parser;
    use mcp_cli::cli::{Cli, Command};

    match Cli::parse().command {
        Command::Install { project } => install_package(&project),
        Command::Run { package, arguments } => run_package(&package, &arguments),
        Command::List => list_packages(),
        Command::Info { package } => info_package(&package),
        Command::Uninstall { package } => uninstall_package(&package),
        Command::Use { selector } => use_package(&selector),
        Command::Update { package, source } => update_package(&package, &source),
        Command::Doctor => doctor(),
    }
}

fn install_package(project: &str) {
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
        if !binary.is_file() { fatal(format!("binary entrypoint not found (expected a file at {})", binary.display())); }
        #[cfg(unix)]
        if !std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&binary).unwrap_or_else(|error| fatal(error.to_string())).permissions()).eq(&0) {
            let mode = std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&binary).unwrap_or_else(|error| fatal(error.to_string())).permissions());
            if mode & 0o111 == 0 { fatal("binary entrypoint is not executable".to_owned()); }
        }
        let _state_lock = lock_state(&state_home);
        if state_home.join("packages").join(&package_name).join(&package_version).exists() {
            fatal(format!("{package_name}@{package_version} is already installed; installed versions are immutable"));
        }
        let root = state_home.join("packages").join(&package_name).join(&package_version);
        let runtime_bin = root.join("runtime/bin");
        std::fs::create_dir_all(&runtime_bin).unwrap_or_else(|error| abort_install(&root, format!("cannot create binary runtime: {error}")));
        copy_source_snapshot(std::path::Path::new(project), &root.join("source")).unwrap_or_else(|error| abort_install(&root, format!("cannot snapshot source: {error}")));
        let installed_binary = runtime_bin.join(&manifest.install.entrypoint);
        if let Some(parent) = installed_binary.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| abort_install(&root, format!("cannot create binary runtime: {error}")));
        }
        std::fs::copy(&binary, &installed_binary).unwrap_or_else(|error| abort_install(&root, format!("cannot copy binary: {error}")));
        let executable_sha256 = std::fs::read(&installed_binary).map(|bytes| mcp_cli::metadata::sha256_hex(&bytes)).unwrap_or_else(|error| abort_install(&root, format!("cannot checksum binary: {error}")));
        let extra = serde_json::json!({ "executable_sha256": executable_sha256 });
        if let Err(error) = write_install_metadata(&root, project, &package_name, &package_version, "binary", &manifest.install.entrypoint, extra) { abort_install(&root, format!("cannot write metadata: {error}")); }
        if let Err(error) = record_install(&state_home, &package_name, &package_version, "binary") { abort_install(&root, format!("cannot record installation: {error}")); }
        if let Err(error) = write_active_selection_if_absent(&state_home, &package_name, &package_version) { abort_recorded_install(&state_home, &root, &package_name, &package_version, format!("cannot write active selection: {error}")); }
        return;
    }
    let mcp_cli::manifest::Runtime::Python { python } = manifest.runtime else { unreachable!() };
    let uv_probe = std::process::Command::new("uv").arg("--version").output();
    let Some(uv_probe) = uv_probe.ok().filter(|probe| probe.status.success()) else { fatal("uv is required to install Python MCPs".to_owned()) };
    // `uv --version` prints e.g. "uv 0.5.1 (abc123 2026-01-01)".
    let uv_version = String::from_utf8_lossy(&uv_probe.stdout).split_whitespace().nth(1).unwrap_or("unknown").to_owned();
    let _state_lock = lock_state(&state_home);
    if state_home.join("packages").join(&package_name).join(&package_version).exists() {
        fatal(format!("{package_name}@{package_version} is already installed; installed versions are immutable"));
    }
    let destination = state_home
        .join("packages")
        .join(&package_name)
        .join(&package_version)
        .join("source");
    let version_root = destination.parent().unwrap().to_path_buf();
    copy_source_snapshot(std::path::Path::new(project), &destination)
        .unwrap_or_else(|error| abort_install(&version_root, format!("cannot snapshot source: {error}")));
    let runtime = version_root.join("runtime").join(".venv");
    let status = std::process::Command::new("uv")
        .args(["sync", "--frozen", "--no-dev", "--python", &python])
        .current_dir(&destination)
        .env("UV_PROJECT_ENVIRONMENT", &runtime)
        .status()
        .unwrap_or_else(|error| abort_install(&version_root, format!("cannot run uv: {error}")));
    if !status.success() {
        abort_install(&version_root, "uv sync failed to install Python MCP dependencies".to_owned());
    }
    if let Err(error) = write_install_metadata(&version_root, project, &package_name, &package_version, "python", &manifest.install.entrypoint, serde_json::json!({ "uv_version": uv_version, "python_constraint": python })) {
        abort_install(&version_root, format!("cannot write metadata: {error}"));
    }
    if let Err(error) = record_install(&state_home, &package_name, &package_version, "python") {
        abort_install(&version_root, format!("cannot record installation: {error}"));
    }
    if let Err(error) = write_active_selection_if_absent(&state_home, &package_name, &package_version) {
        abort_recorded_install(&state_home, &version_root, &package_name, &package_version, format!("cannot write active selection: {error}"));
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
    let mut warnings = 0;
    let probe = state_home.join(format!(".doctor-write-probe-{}", std::process::id()));
    match std::fs::write(&probe, b"") {
        Ok(()) => { let _ = std::fs::remove_file(&probe); }
        Err(error) => { println!("ERROR data directory {} is not writable: {error}", state_home.display()); errors += 1; }
    }
    for (name, package) in &registry.packages {
        if let Some(active) = &package.active_version {
            if !package.versions.iter().any(|version| &version.version == active) { println!("ERROR {name}: active version {active} is not installed"); errors += 1; }
            // Older installs have no active file, so absence is only a warning; disagreement is an error.
            match std::fs::read_to_string(state_home.join("active").join(format!("{name}.json"))) {
                Err(_) => { println!("WARN {name}: no active file"); warnings += 1; }
                Ok(contents) => match serde_json::from_str::<serde_json::Value>(&contents) {
                    Ok(value) if value["version"].as_str() == Some(active.as_str()) => {}
                    Ok(value) => { println!("ERROR {name}: active file selects {} but registry selects {active}", value["version"].as_str().unwrap_or("nothing")); errors += 1; }
                    Err(error) => { println!("ERROR {name}: active file is unreadable: {error}"); errors += 1; }
                },
            }
        }
        for version in &package.versions {
            let root = state_home.join("packages").join(name).join(&version.version);
            match std::fs::read_to_string(root.join("metadata.json")) {
                Err(_) => { println!("WARN {name}@{}: no metadata.json", version.version); warnings += 1; }
                Ok(contents) => if let Err(error) = serde_json::from_str::<serde_json::Value>(&contents) { println!("ERROR {name}@{}: metadata.json is unreadable: {error}", version.version); errors += 1; },
            }
            let manifest_path = root.join("source/mcpctl.toml");
            let manifest = std::fs::read_to_string(&manifest_path).ok().and_then(|contents| mcp_cli::manifest::parse_manifest(&contents).ok());
            let Some(manifest) = manifest else { println!("ERROR {name}@{}: missing or invalid manifest", version.version); errors += 1; continue };
            let entrypoint = match manifest.runtime {
                mcp_cli::manifest::Runtime::Python { .. } => root.join("runtime/.venv/bin/python"),
                mcp_cli::manifest::Runtime::Binary => root.join("runtime/bin").join(manifest.install.entrypoint),
            };
            if !entrypoint.is_file() { println!("ERROR {name}@{}: missing entrypoint", version.version); errors += 1; continue }
            #[cfg(unix)]
            match std::fs::metadata(&entrypoint) {
                Ok(metadata) if std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o111 == 0 => { println!("ERROR {name}@{}: entrypoint is not executable", version.version); errors += 1; }
                Ok(_) => {}
                Err(error) => { println!("ERROR {name}@{}: cannot read entrypoint: {error}", version.version); errors += 1; }
            }
        }
    }
    // Directories under packages/ that the registry does not know about.
    for name_entry in std::fs::read_dir(state_home.join("packages")).into_iter().flatten().flatten() {
        let name = name_entry.file_name().to_string_lossy().into_owned();
        for version_entry in std::fs::read_dir(name_entry.path()).into_iter().flatten().flatten() {
            let version = version_entry.file_name().to_string_lossy().into_owned();
            if !registry.packages.get(&name).is_some_and(|package| package.versions.iter().any(|installed| installed.version == version)) {
                println!("WARN {name}@{version}: orphaned directory not in the registry"); warnings += 1;
            }
        }
    }
    if errors == 0 { println!("OK: registry and installed entrypoints are healthy{}", if warnings > 0 { format!(" ({warnings} warning(s))") } else { String::new() }); } else { std::process::exit(1); }
}

fn use_package(selector: &str) {
    let Some((name, version)) = selector.split_once('@') else { fatal("use requires name@version".to_owned()) };
    if name.is_empty() || version.is_empty() || name.contains(['/', '\\']) || version.contains(['/', '\\']) {
        fatal("invalid package selector".to_owned());
    }
    let state_home = mcp_cli::paths::data_home_from(std::env::var_os("MCPCTL_HOME").map(std::path::PathBuf::from)).unwrap_or_else(|error| fatal(error));
    let _state_lock = lock_state(&state_home);
    let registry_path = state_home.join("registry.json");
    let mut registry = mcp_cli::registry::load_registry(&registry_path).unwrap_or_else(|error| fatal(format!("cannot read registry: {error}")));
    let package = registry.packages.get_mut(name).unwrap_or_else(|| fatal(format!("{name} is not installed")));
    if !package.versions.iter().any(|installed| installed.version == version) { fatal(format!("{name}@{version} is not installed")); }
    let previous = package.active_version.replace(version.to_owned());
    // The active file is authoritative, so write it first; a failure then leaves both files untouched.
    write_active_selection(&state_home, name, version).unwrap_or_else(|error| fatal(format!("cannot write active selection: {error}")));
    if let Err(error) = mcp_cli::registry::save_registry(&registry_path, &registry) {
        if let Some(previous) = previous { let _ = write_active_selection(&state_home, name, &previous); }
        fatal(error);
    }
}

fn uninstall_package(selector: &str) {
    let Some((name, version)) = selector.split_once('@') else { fatal("uninstall requires name@version".to_owned()) };
    if name.is_empty() || version.is_empty() || name.contains(['/', '\\']) || version.contains(['/', '\\']) {
        fatal("invalid package selector".to_owned());
    }
    let state_home = mcp_cli::paths::data_home_from(std::env::var_os("MCPCTL_HOME").map(std::path::PathBuf::from)).unwrap_or_else(|error| fatal(error));
    let _state_lock = lock_state(&state_home);
    let registry_path = state_home.join("registry.json");
    let mut registry = mcp_cli::registry::load_registry(&registry_path).unwrap_or_else(|error| fatal(format!("cannot read registry: {error}")));
    let package = registry.packages.get_mut(name).unwrap_or_else(|| fatal(format!("{name} is not installed")));
    if package.active_version.as_deref() == Some(version) && package.versions.len() > 1 { fatal(format!("{name}@{version} is active; select another version first")); }
    let before = package.versions.len(); package.versions.retain(|installed| installed.version != version);
    if package.versions.len() == before { fatal(format!("{name}@{version} is not installed")); }
    if package.active_version.as_deref() == Some(version) { package.active_version = None; }
    let original_registry = std::fs::read_to_string(&registry_path).unwrap_or_else(|error| fatal(format!("cannot read registry: {error}")));
    mcp_cli::registry::save_registry(&registry_path, &registry).unwrap_or_else(|error| fatal(error));
    if let Err(error) = std::fs::remove_dir_all(state_home.join("packages").join(name).join(version)) {
        // Put the entry back so registry and files stay consistent and a retry can succeed.
        let _ = std::fs::write(&registry_path, original_registry);
        fatal(format!("cannot remove {name}@{version}: {error}"));
    }
    if registry.packages.get(name).is_some_and(|package| package.active_version.is_none()) {
        match std::fs::remove_file(state_home.join("active").join(format!("{name}.json"))) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => fatal(format!("cannot remove active selection for {name}: {error}")),
        }
    }
}

fn info_package(selector: &str) {
    let (name, requested_version) = selector.split_once('@').map_or((selector, None), |(name, version)| (name, Some(version)));
    let state_home = mcp_cli::paths::data_home_from(std::env::var_os("MCPCTL_HOME").map(std::path::PathBuf::from)).unwrap_or_else(|error| fatal(error));
    let registry = mcp_cli::registry::load_registry(&state_home.join("registry.json")).unwrap_or_else(|error| fatal(format!("cannot read registry: {error}")));
    let package = registry.packages.get(name).unwrap_or_else(|| fatal(format!("{name} is not installed")));
    let selected: Vec<_> = package.versions.iter().filter(|version| requested_version.is_none_or(|requested| version.version == requested)).collect();
    if selected.is_empty() { fatal(format!("{selector} is not installed")); }
    for version in selected {
        let install_path = state_home.join("packages").join(name).join(&version.version);
        // Only fields recorded by mcpctl itself are printed, never anything from the process environment.
        let metadata = std::fs::read_to_string(install_path.join("metadata.json")).ok().and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok());
        let field = |key: &str| metadata.as_ref().and_then(|metadata| metadata[key].as_str()).map(str::to_owned);
        println!("name: {name}");
        println!("version: {}", version.version);
        println!("runtime: {}", version.runtime);
        println!("active: {}", if package.active_version.as_deref() == Some(&version.version) { "yes" } else { "no" });
        println!("install_path: {}", install_path.display());
        if let Some(source) = field("source_path") { println!("source: {source}"); }
        if let Some(entrypoint) = field("entrypoint_relative_path") { println!("entrypoint: {entrypoint}"); }
        if let Some(python) = field("python_constraint") { println!("python: {python}"); }
        println!("installed_at: {}", field("installed_at").unwrap_or_else(|| version.installed_at.clone()));
    }
}

fn list_packages() {
    let state_home = mcp_cli::paths::data_home_from(std::env::var_os("MCPCTL_HOME").map(std::path::PathBuf::from))
        .unwrap_or_else(|error| fatal(error));
    let registry = mcp_cli::registry::load_registry(&state_home.join("registry.json"))
        .unwrap_or_else(|error| fatal(format!("cannot read registry: {error}")));
    let mut rows = vec![["NAME".to_owned(), "ACTIVE".to_owned(), "VERSIONS".to_owned(), "RUNTIME".to_owned()]];
    for (name, package) in registry.packages {
        let versions = package.versions.iter().map(|version| version.version.as_str()).collect::<Vec<_>>().join(", ");
        let runtime = package.active_version.as_ref().and_then(|active| package.versions.iter().find(|version| &version.version == active)).map(|version| version.runtime.clone()).unwrap_or_else(|| "unknown".to_owned());
        rows.push([name, package.active_version.unwrap_or_else(|| "-".to_owned()), versions, runtime]);
    }
    let widths: Vec<usize> = (0..4).map(|column| rows.iter().map(|row| row[column].len()).max().unwrap_or(0)).collect();
    for row in rows {
        let line = row.iter().zip(&widths).map(|(cell, width)| format!("{cell:<width$}")).collect::<Vec<_>>().join("  ");
        println!("{}", line.trim_end());
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
    let runtime_bin = match manifest.runtime {
        mcp_cli::manifest::Runtime::Python { .. } => install_root.join("runtime/.venv/bin"),
        mcp_cli::manifest::Runtime::Binary => install_root.join("runtime/bin"),
    };
    let mut path_entries = vec![runtime_bin.clone()];
    if let Some(path) = std::env::var_os("PATH") {
        path_entries.extend(std::env::split_paths(&path));
    }
    // `metadata.json` is authoritative; the manifest copy only covers installs that predate it.
    let entrypoint = std::fs::read_to_string(install_root.join("metadata.json")).ok()
        .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
        .and_then(|metadata| metadata["entrypoint_relative_path"].as_str().map(str::to_owned))
        .unwrap_or(manifest.install.entrypoint);
    let mut command = std::process::Command::new(runtime_bin.join(entrypoint));
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

/// Removes a partially created version directory (and its package directory if
/// now empty) so a failed install never blocks a retry, then exits with `message`.
/// Like `abort_install`, but for a failure after `record_install`: also drops the registry entry so
/// a retry is not rejected as "already installed".
fn abort_recorded_install(state_home: &std::path::Path, version_root: &std::path::Path, name: &str, version: &str, message: String) -> ! {
    let path = state_home.join("registry.json");
    if let Ok(mut registry) = mcp_cli::registry::load_registry(&path) {
        if let Some(package) = registry.packages.get_mut(name) {
            package.versions.retain(|installed| installed.version != version);
            if package.active_version.as_deref() == Some(version) { package.active_version = None; }
            if package.versions.is_empty() { registry.packages.remove(name); }
        }
        let _ = mcp_cli::registry::save_registry(&path, &registry);
    }
    abort_install(version_root, message)
}

fn abort_install(version_root: &std::path::Path, message: String) -> ! {
    let _ = std::fs::remove_dir_all(version_root);
    if let Some(package_dir) = version_root.parent() {
        if std::fs::read_dir(package_dir).ok().is_some_and(|mut entries| entries.next().is_none()) {
            let _ = std::fs::remove_dir(package_dir);
        }
    }
    fatal(message)
}

fn lock_state(state_home: &std::path::Path) -> mcp_cli::lock::StateLock {
    mcp_cli::lock::StateLock::exclusive(state_home).unwrap_or_else(|error| fatal(error))
}

fn fatal(message: String) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

/// VCS metadata, virtualenvs, caches, ordinary build output and local `.env` files stay out of
/// installed snapshots; `.env` templates are kept. `dist/` is kept because binary entrypoints may live there.
fn is_excluded_from_snapshot(name: &str) -> bool {
    let is_env_file = name == ".env" || name.strip_prefix(".env.").is_some_and(|suffix| !matches!(suffix, "example" | "sample" | "template"));
    is_env_file
        || matches!(name, ".git" | ".venv" | "__pycache__" | "target" | "build")
        || name.ends_with(".pyc")
        || name.ends_with(".egg-info")
}

fn copy_source_snapshot(source: &std::path::Path, destination: &std::path::Path) -> std::io::Result<()> {
    copy_snapshot_dir(&source.canonicalize()?, source, destination)
}

/// Relative symlinks are preserved only when they resolve inside `root`; anything else is rejected.
#[cfg(unix)]
fn copy_snapshot_symlink(root: &std::path::Path, path: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    let reject = |reason: &str| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("symlink {} {reason}", path.display()));
    let link = std::fs::read_link(path)?;
    if link.is_absolute() {
        return Err(reject("is absolute; only relative symlinks are allowed"));
    }
    let resolved = path.canonicalize().map_err(|_| reject("does not resolve to an existing file"))?;
    if !resolved.starts_with(root) {
        return Err(reject("escapes the source root"));
    }
    std::os::unix::fs::symlink(link, target)
}

fn copy_snapshot_dir(root: &std::path::Path, source: &std::path::Path, destination: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if destination.starts_with(&path) || is_excluded_from_snapshot(&name.to_string_lossy()) {
            continue;
        }
        let target = destination.join(&name);
        #[cfg(unix)]
        if entry.file_type()?.is_symlink() {
            copy_snapshot_symlink(root, &path, &target)?;
            continue;
        }
        if entry.file_type()?.is_dir() {
            copy_snapshot_dir(root, &path, &target)?;
        } else if entry.file_type()?.is_file() {
            std::fs::copy(path, target)?;
        }
    }
    Ok(())
}

/// Writes `metadata.json`, the authoritative record of one installed version.
fn write_install_metadata(version_root: &std::path::Path, project: &str, name: &str, version: &str, runtime_type: &str, entrypoint: &str, extra: serde_json::Value) -> Result<(), String> {
    let installed_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs();
    let source_path = std::fs::canonicalize(project).map_err(|error| error.to_string())?;
    let manifest_bytes = std::fs::read(std::path::Path::new(project).join("mcpctl.toml")).map_err(|error| error.to_string())?;
    let source_tree_sha256 = mcp_cli::metadata::source_tree_sha256(&version_root.join("source")).map_err(|error| error.to_string())?;
    let mut metadata = serde_json::json!({
        "schema_version": 1,
        "name": name,
        "version": version,
        "runtime_type": runtime_type,
        "installed_at": mcp_cli::metadata::format_rfc3339_utc(installed_at),
        "mcpctl_version": env!("CARGO_PKG_VERSION"),
        "source_path": source_path,
        "entrypoint_relative_path": entrypoint,
        "manifest_sha256": mcp_cli::metadata::sha256_hex(&manifest_bytes),
        "source_tree_sha256": source_tree_sha256,
    });
    if let serde_json::Value::Object(fields) = extra {
        metadata.as_object_mut().expect("metadata is an object").extend(fields);
    }
    if runtime_type == "python" {
        if let Ok(lockfile) = std::fs::read(std::path::Path::new(project).join("uv.lock")) {
            metadata["lockfile_sha256"] = mcp_cli::metadata::sha256_hex(&lockfile).into();
        }
    }
    let contents = serde_json::to_string_pretty(&metadata).map_err(|error| error.to_string())?;
    std::fs::write(version_root.join("metadata.json"), contents).map_err(|error| error.to_string())
}

/// Installing a version activates it only when the MCP has no active selection yet; `create_new`
/// makes "only when absent" atomic, so an existing selection is never overwritten.
fn write_active_selection_if_absent(state_home: &std::path::Path, name: &str, version: &str) -> Result<(), String> {
    let directory = state_home.join("active");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let file = std::fs::OpenOptions::new().write(true).create_new(true).open(directory.join(format!("{name}.json")));
    match file {
        Ok(mut file) => {
            let contents = serde_json::to_string_pretty(&serde_json::json!({ "schema_version": 1, "name": name, "version": version }))
                .map_err(|error| error.to_string())?;
            std::io::Write::write_all(&mut file, contents.as_bytes()).map_err(|error| error.to_string())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

/// Replaces `active/<name>.json` atomically: write a temp file in the same directory, then rename over it.
fn write_active_selection(state_home: &std::path::Path, name: &str, version: &str) -> Result<(), String> {
    let directory = state_home.join("active");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let contents = serde_json::to_string_pretty(&serde_json::json!({ "schema_version": 1, "name": name, "version": version }))
        .map_err(|error| error.to_string())?;
    let temporary = directory.join(format!(".{name}.json.tmp-{}", std::process::id()));
    std::fs::write(&temporary, contents).map_err(|error| error.to_string())?;
    std::fs::rename(&temporary, directory.join(format!("{name}.json"))).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        error.to_string()
    })
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
