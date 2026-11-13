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

/// Records the destination's mode the first time it is read from, i.e. while the copy is in flight.
#[cfg(unix)]
struct ObservesDestination { destination: PathBuf, mode_mid_copy: Option<u32>, done: bool }

#[cfg(unix)]
impl Read for ObservesDestination {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        use std::os::unix::fs::PermissionsExt;
        if self.done { return Ok(0); }
        self.mode_mid_copy = Some(fs::metadata(&self.destination)?.permissions().mode() & 0o777);
        self.done = true;
        let private = b"private contents";
        buffer[..private.len()].copy_from_slice(private);
        Ok(private.len())
    }
}

#[cfg(unix)]
#[test]
fn a_private_source_is_never_readable_by_others_while_it_is_being_copied() {
    use std::os::unix::fs::PermissionsExt;
    let destination = temporary_dir().join("private-notes.txt");
    let mut reader = ObservesDestination { destination: destination.clone(), mode_mid_copy: None, done: false };

    mcp_cli::scaffold_copy::copy_new_hashing(&mut reader, &destination, fs::Permissions::from_mode(0o600)).unwrap();

    assert_eq!(reader.mode_mid_copy.unwrap() & 0o077, 0, "group/other could read the copy before its permissions were applied");
    assert_eq!(fs::metadata(&destination).unwrap().permissions().mode() & 0o777, 0o600);
}
