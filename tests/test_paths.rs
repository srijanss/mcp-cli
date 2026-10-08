use std::path::{Path, PathBuf};

use mcp_cli::paths::absolute_lexically;

#[test]
fn absolute_lexically_joins_a_relative_path_to_the_base_and_resolves_dot_components_without_the_filesystem() {
    let base = Path::new("/no/such/workspace/project");

    assert_eq!(absolute_lexically(base, Path::new("../removed-mcp")), PathBuf::from("/no/such/workspace/removed-mcp"));
    assert_eq!(absolute_lexically(base, Path::new("./a/./b/../c")), PathBuf::from("/no/such/workspace/project/a/c"));
    assert_eq!(absolute_lexically(base, Path::new("/elsewhere/../x")), PathBuf::from("/x"));
    assert_eq!(absolute_lexically(base, Path::new("../../../../..")), PathBuf::from("/"));
}
