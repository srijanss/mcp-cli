use mcp_cli::metadata::format_rfc3339_utc;

#[test]
fn formats_unix_seconds_as_rfc3339_utc() {
    assert_eq!(format_rfc3339_utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(format_rfc3339_utc(951_782_400), "2000-02-29T00:00:00Z"); // leap day
    assert_eq!(format_rfc3339_utc(1_700_000_000), "2023-11-14T22:13:20Z");
    assert_eq!(format_rfc3339_utc(4_102_444_799), "2099-12-31T23:59:59Z");
}
