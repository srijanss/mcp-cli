use mcp_cli::catalog::{parse_catalog, render_catalog, Catalog, CatalogEntry};

#[test]
fn catalog_renders_one_mcp_table_per_source_in_order_and_parses_back() {
    let catalog = Catalog {
        mcp: vec![
            CatalogEntry { source: "/code/project-mcp".to_owned() },
            CatalogEntry { source: "/code/design-advisor-mcp".to_owned() },
        ],
    };

    let rendered = render_catalog(&catalog);

    assert_eq!(
        rendered,
        "[[mcp]]\nsource = \"/code/project-mcp\"\n\n[[mcp]]\nsource = \"/code/design-advisor-mcp\"\n"
    );
    assert_eq!(parse_catalog(&rendered).unwrap(), catalog);
    assert_eq!(parse_catalog("").unwrap(), Catalog { mcp: vec![] });
}
