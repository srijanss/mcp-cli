//! Copying one scaffold file into a project: the destination appears complete or not at all, and its
//! sha256 is computed from the bytes as they stream past.
use std::io::{Read, Write};
use std::path::Path;

use sha2::Digest;

/// Streams `reader` into a new file at `destination` (never an existing one), applies `permissions`, and
/// returns the sha256 of the bytes copied. On any failure after the destination is created it is removed
/// again, so a later run doesn't skip a truncated file as already present.
pub fn copy_new_hashing(reader: &mut impl Read, destination: &Path, permissions: std::fs::Permissions) -> std::io::Result<String> {
    let mut writer = std::fs::OpenOptions::new().write(true).create_new(true).open(destination)?;
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
