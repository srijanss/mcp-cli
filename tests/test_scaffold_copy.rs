use std::{fs, io::Read, path::PathBuf, sync::atomic::{AtomicUsize, Ordering}};

fn temporary_dir() -> PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "mcpctl-scaffold-copy-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

/// Yields `chunk` once, then fails the next read.
struct FailsAfterFirstChunk { chunk: Option<&'static [u8]> }

impl Read for FailsAfterFirstChunk {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self.chunk.take() {
            Some(chunk) => { buffer[..chunk.len()].copy_from_slice(chunk); Ok(chunk.len()) }
            None => Err(std::io::Error::other("disk went away")),
        }
    }
}

#[test]
fn a_copy_that_fails_midway_leaves_no_destination_behind() {
    let destination = temporary_dir().join("hooks.md");
    let permissions = fs::metadata(temporary_dir()).unwrap().permissions();

    let result = mcp_cli::scaffold_copy::copy_new_hashing(&mut FailsAfterFirstChunk { chunk: Some(b"half a file") }, &destination, permissions);

    assert!(result.is_err());
    assert!(!destination.exists(), "a partial copy would be skipped as already present on the next init");
}
