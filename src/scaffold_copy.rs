//! Copying one scaffold file into a project: the destination appears complete or not at all, and its
//! sha256 is computed from the bytes as they stream past.
use std::io::{Read, Write};
use std::path::Path;

use sha2::Digest;

/// Streams `reader` into a new file at `destination` (never an existing one), applies `permissions`, and
/// returns the sha256 of the bytes copied. On any failure after the destination is created it is removed
/// again, so a later run doesn't skip a truncated file as already present.
pub fn copy_new_hashing(reader: &mut impl Read, destination: &Path, permissions: std::fs::Permissions) -> std::io::Result<String> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    // Owner-only until the copy is complete, so a private source is never briefly readable by others.
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut writer = options.open(destination)?;
    let copied = stream_hashing(reader, &mut writer).and_then(|sha256| writer.set_permissions(permissions).map(|()| sha256));
    if copied.is_err() {
        drop(writer);
        let _ = std::fs::remove_file(destination);
    }
    copied
}

fn stream_hashing(reader: &mut impl Read, writer: &mut impl Write) -> std::io::Result<String> {
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 { break; }
        hasher.update(&buffer[..read]);
        writer.write_all(&buffer[..read])?;
    }
    Ok(hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect())
}

/// The sha256 of the file at `path`, streamed rather than read whole.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    stream_hashing(&mut std::fs::File::open(path)?, &mut std::io::sink())
}

/// Replaces the file at `destination` with a copy of `source` (keeping `source`'s permissions), returning the
/// copy's sha256. The copy is written beside `destination` and renamed over it, so an interrupted update leaves
/// the old file in place rather than a truncated one.
pub fn replace_hashing(source: &Path, destination: &Path) -> std::io::Result<String> {
    let mut reader = std::fs::File::open(source)?;
    let permissions = reader.metadata()?.permissions();
    let mut name = destination.file_name().unwrap_or_default().to_os_string();
    name.push(".mcpctl-new");
    let partial = destination.with_file_name(name);
    // A leftover from an interrupted run (or anything planted there) is unlinked, never written through.
    match std::fs::remove_file(&partial) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let sha256 = copy_new_hashing(&mut reader, &partial, permissions)?;
    std::fs::rename(&partial, destination).inspect_err(|_| { let _ = std::fs::remove_file(&partial); })?;
    Ok(sha256)
}
