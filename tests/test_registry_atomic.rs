use mcp_cli::registry::{save_registry, Registry};

#[test]
fn save_replaces_the_file_and_leaves_no_temporary_files() {
    let dir = std::env::temp_dir().join(format!("mcpctl-registry-atomic-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("registry.json");
    std::fs::write(&path, "old").unwrap();
    // A hard link shares the inode: an in-place rewrite changes it, a rename-replace does not.
    std::fs::hard_link(&path, dir.join("registry.before")).unwrap();

    save_registry(&path, &Registry::new()).unwrap();

    assert_eq!(std::fs::read_to_string(dir.join("registry.before")).unwrap(), "old");
    assert!(std::fs::read_to_string(&path).unwrap().contains("schema_version"));
    let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["registry.before", "registry.json"], "no temporary file should remain");

    std::fs::remove_dir_all(dir).unwrap();
}
