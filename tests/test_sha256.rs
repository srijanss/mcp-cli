use mcp_cli::metadata::sha256_hex;

#[test]
fn sha256_hex_matches_the_known_vector() {
    assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

use mcp_cli::metadata::source_tree_sha256;

fn tree(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("mcpctl-treehash-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (path, contents) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
    root
}

#[test]
fn source_tree_hash_is_deterministic_and_sensitive_to_paths_and_contents() {
    let base = tree("base", &[("a.py", "1"), ("pkg/b.py", "2")]);
    // Same files created in a different order must hash the same.
    let reordered = tree("reordered", &[("pkg/b.py", "2"), ("a.py", "1")]);
    let changed_contents = tree("contents", &[("a.py", "1"), ("pkg/b.py", "3")]);
    let renamed = tree("renamed", &[("a.py", "1"), ("pkg/c.py", "2")]);
    // Moving bytes between adjacent files must not collide: "ab"+"" vs "a"+"b".
    let split_one = tree("split-one", &[("x", "ab"), ("y", "")]);
    let split_two = tree("split-two", &[("x", "a"), ("y", "b")]);

    let hash = source_tree_sha256(&base).unwrap();

    assert_eq!(hash.len(), 64);
    assert_eq!(hash, source_tree_sha256(&reordered).unwrap());
    assert_ne!(hash, source_tree_sha256(&changed_contents).unwrap());
    assert_ne!(hash, source_tree_sha256(&renamed).unwrap());
    assert_ne!(source_tree_sha256(&split_one).unwrap(), source_tree_sha256(&split_two).unwrap());

    for dir in [base, reordered, changed_contents, renamed, split_one, split_two] {
        std::fs::remove_dir_all(dir).unwrap();
    }
}
