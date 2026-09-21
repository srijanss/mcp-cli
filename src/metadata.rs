/// Lowercase hex SHA-256 digest of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Deterministic SHA-256 over a directory tree: every file (and symlink target) in sorted
/// relative-path order, each as a tag, a length-prefixed `/`-separated path and length-prefixed
/// bytes. Empty directories, mtimes and permissions do not contribute.
pub fn source_tree_sha256(root: &std::path::Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};

    fn collect(root: &std::path::Path, directory: &std::path::Path, entries: &mut Vec<(String, std::path::PathBuf)>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(directory)? {
            let path = entry?.path();
            if std::fs::symlink_metadata(&path)?.is_dir() {
                collect(root, &path, entries)?;
            } else {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                let normalized = relative.components().map(|part| part.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
                entries.push((normalized, path));
            }
        }
        Ok(())
    }

    let mut entries = Vec::new();
    collect(root, root, &mut entries)?;
    entries.sort();

    let mut hasher = Sha256::new();
    for (relative, path) in entries {
        let (tag, content) = if std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
            (b'L', std::fs::read_link(&path)?.to_string_lossy().into_owned().into_bytes())
        } else {
            (b'F', std::fs::read(&path)?)
        };
        hasher.update([tag]);
        hasher.update((relative.len() as u64).to_be_bytes());
        hasher.update(relative.as_bytes());
        hasher.update((content.len() as u64).to_be_bytes());
        hasher.update(&content);
    }
    Ok(hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Formats Unix seconds as an RFC 3339 UTC timestamp, e.g. `2023-11-14T22:13:20Z`.
pub fn format_rfc3339_utc(unix_seconds: u64) -> String {
    let (days, seconds_of_day) = (unix_seconds / 86_400, unix_seconds % 86_400);
    // Civil-from-days (Howard Hinnant), shifted so the era starts on 0000-03-01.
    let shifted = days as i64 + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 { month_index + 3 } else { month_index - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds_of_day / 3_600,
        seconds_of_day % 3_600 / 60,
        seconds_of_day % 60
    )
}
