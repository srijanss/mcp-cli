use std::fs::{File, TryLockError};
use std::path::Path;

/// Exclusive per-`MCPCTL_HOME` lock; released when dropped.
#[derive(Debug)]
pub struct StateLock {
    _file: File,
}

impl StateLock {
    pub fn exclusive(state_home: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(state_home).map_err(|error| error.to_string())?;
        let path = state_home.join("mcpctl.lock");
        let file = File::create(&path).map_err(|error| format!("cannot open {}: {error}", path.display()))?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(format!(
                "state is locked by another mcpctl process ({})",
                path.display()
            )),
            Err(TryLockError::Error(error)) => Err(format!("cannot lock {}: {error}", path.display())),
        }
    }
}
