//! Cross-process transactions. The kernel releases locks when a process exits,
//! including crashes; the lock file must never be unlinked while in use.

use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::time::{Duration, Instant};

use crate::context::SpoolContext;
use crate::engine::SpoolError;

pub struct FileLock {
    _file: File,
}

impl FileLock {
    pub fn acquire(ctx: &SpoolContext) -> Result<Self> {
        let path = ctx.root.join(".lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("Cannot open board lock {}", path.display()))?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(SpoolError::new(
                            "busy",
                            "Board is busy; retry the command shortly",
                        )
                        .into());
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}
