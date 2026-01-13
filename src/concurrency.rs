use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::context::FabricContext;
use crate::event::{Event, Operation};

/// Global sequence counter for optimistic locking
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Version information for conflict detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    /// Sequence number within current session
    pub seq: u64,
    /// Timestamp of last modification
    pub ts: String,
    /// Hash of the last event for this task
    pub last_event_hash: String,
}

/// Result of an optimistic write attempt
#[derive(Debug)]
pub enum WriteResult {
    /// Write succeeded
    Success,
    /// Write failed due to concurrent modification
    Conflict {
        expected_version: Version,
        actual_version: Version,
    },
    /// Other error occurred
    Error(String),
}

/// Lock file for coordinating writes
pub struct FileLock {
    path: PathBuf,
    _lock_file: Option<File>,
}

impl FileLock {
    /// Acquire an advisory lock on the events directory
    pub fn acquire(ctx: &FabricContext) -> Result<Self> {
        let path = ctx.root.join(".lock");

        // Try to create lock file exclusively
        let lock_file = OpenOptions::new().write(true).create_new(true).open(&path);

        match lock_file {
            Ok(mut f) => {
                // Write PID and timestamp
                writeln!(f, "{}:{}", std::process::id(), Utc::now().to_rfc3339())?;
                Ok(Self {
                    path,
                    _lock_file: Some(f),
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Check if lock is stale (older than 60 seconds)
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Some(ts_str) = content.split(':').nth(1) {
                        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(ts_str.trim()) {
                            let age = Utc::now().signed_duration_since(ts);
                            if age.num_seconds() > 60 {
                                // Stale lock, remove and retry
                                fs::remove_file(&path)?;
                                return Self::acquire(ctx);
                            }
                        }
                    }
                }
                Err(anyhow!("Lock held by another process"))
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Get the current version of a task by reading its last event
pub fn get_task_version(ctx: &FabricContext, task_id: &str) -> Result<Option<Version>> {
    let mut last_event: Option<Event> = None;

    // Scan event files in reverse chronological order
    let mut files = ctx.get_event_files()?;
    files.reverse();

    for file in files {
        let events = ctx.parse_events_from_file(&file)?;
        for event in events.into_iter().rev() {
            if event.id == task_id {
                last_event = Some(event);
                break;
            }
        }
        if last_event.is_some() {
            break;
        }
    }

    match last_event {
        Some(event) => {
            let event_json = serde_json::to_string(&event)?;
            let hash = format!("{:x}", md5_hash(&event_json));
            Ok(Some(Version {
                seq: SEQUENCE.fetch_add(1, Ordering::SeqCst),
                ts: event.ts.to_rfc3339(),
                last_event_hash: hash,
            }))
        }
        None => Ok(None),
    }
}

/// Write an event with optimistic locking
pub fn write_event_with_version(
    ctx: &FabricContext,
    event: &Event,
    expected_version: Option<&Version>,
) -> Result<WriteResult> {
    // Acquire file lock
    let _lock = FileLock::acquire(ctx)?;

    // Check current version
    let current_version = get_task_version(ctx, &event.id)?;

    // Version conflict detection
    match (expected_version, &current_version) {
        (Some(expected), Some(actual)) => {
            if expected.last_event_hash != actual.last_event_hash {
                return Ok(WriteResult::Conflict {
                    expected_version: expected.clone(),
                    actual_version: actual.clone(),
                });
            }
        }
        (Some(_expected), None) => {
            // Expected a version but task doesn't exist
            if event.op != Operation::Create {
                return Ok(WriteResult::Error(
                    "Task does not exist but expected version provided".to_string(),
                ));
            }
        }
        (None, Some(_actual)) => {
            // No expected version but task exists - allow for creates
            if event.op == Operation::Create {
                return Ok(WriteResult::Error("Task already exists".to_string()));
            }
        }
        (None, None) => {
            // No version check needed for new tasks
            if event.op != Operation::Create {
                return Ok(WriteResult::Error("Task does not exist".to_string()));
            }
        }
    }

    // Write event
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let event_file = ctx.events_dir.join(format!("{}.jsonl", today));

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&event_file)?;
    let mut writer = BufWriter::new(file);

    let json = serde_json::to_string(event)?;
    writeln!(writer, "{}", json)?;
    writer.flush()?;

    Ok(WriteResult::Success)
}

/// Retry a write operation with exponential backoff
pub fn write_with_retry<F>(
    ctx: &FabricContext,
    max_retries: u32,
    mut operation: F,
) -> Result<WriteResult>
where
    F: FnMut(&FabricContext) -> Result<(Event, Option<Version>)>,
{
    let mut retries = 0;
    let mut delay_ms = 10;

    loop {
        let (event, version) = operation(ctx)?;
        let result = write_event_with_version(ctx, &event, version.as_ref())?;

        match result {
            WriteResult::Success => return Ok(WriteResult::Success),
            WriteResult::Conflict { .. } if retries < max_retries => {
                retries += 1;
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                delay_ms *= 2; // Exponential backoff
            }
            other => return Ok(other),
        }
    }
}

/// Simple MD5 hash for version fingerprinting
fn md5_hash(input: &str) -> u128 {
    let mut hash: u128 = 0;
    for (i, byte) in input.bytes().enumerate() {
        hash = hash.wrapping_add((byte as u128).wrapping_mul((i as u128).wrapping_add(1)));
        hash = hash.wrapping_mul(31);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_ctx() -> (TempDir, FabricContext) {
        let temp_dir = TempDir::new().unwrap();
        let fabric_dir = temp_dir.path().join(".fabric");
        fs::create_dir_all(fabric_dir.join("events")).unwrap();
        fs::create_dir_all(fabric_dir.join("archive")).unwrap();

        let ctx = FabricContext {
            root: fabric_dir.clone(),
            events_dir: fabric_dir.join("events"),
            archive_dir: fabric_dir.join("archive"),
        };

        (temp_dir, ctx)
    }

    #[test]
    fn test_file_lock_acquire_release() {
        let (_temp, ctx) = setup_test_ctx();

        {
            let _lock = FileLock::acquire(&ctx).unwrap();
            // Lock should be held
            assert!(FileLock::acquire(&ctx).is_err());
        }

        // Lock should be released
        let _lock = FileLock::acquire(&ctx).unwrap();
    }

    #[test]
    fn test_version_tracking() {
        let (_temp, ctx) = setup_test_ctx();

        // Initially no version
        let version = get_task_version(&ctx, "task-1").unwrap();
        assert!(version.is_none());

        // Write an event
        let event = Event {
            v: 1,
            op: Operation::Create,
            id: "task-1".to_string(),
            ts: Utc::now(),
            by: "@test".to_string(),
            branch: "main".to_string(),
            d: serde_json::json!({"title": "Test"}),
        };

        let result = write_event_with_version(&ctx, &event, None).unwrap();
        assert!(matches!(result, WriteResult::Success));

        // Now should have a version
        let version = get_task_version(&ctx, "task-1").unwrap();
        assert!(version.is_some());
    }

    #[test]
    fn test_conflict_detection() {
        let (_temp, ctx) = setup_test_ctx();

        // Create task
        let create_event = Event {
            v: 1,
            op: Operation::Create,
            id: "task-1".to_string(),
            ts: Utc::now(),
            by: "@test".to_string(),
            branch: "main".to_string(),
            d: serde_json::json!({"title": "Test"}),
        };
        write_event_with_version(&ctx, &create_event, None).unwrap();

        // Get version
        let version1 = get_task_version(&ctx, "task-1").unwrap().unwrap();

        // Simulate concurrent update by another process
        let update_event = Event {
            v: 1,
            op: Operation::Update,
            id: "task-1".to_string(),
            ts: Utc::now(),
            by: "@other".to_string(),
            branch: "main".to_string(),
            d: serde_json::json!({"title": "Updated by other"}),
        };
        write_event_with_version(&ctx, &update_event, Some(&version1)).unwrap();

        // Now try to update with stale version
        let version2 = Version {
            seq: version1.seq,
            ts: version1.ts.clone(),
            last_event_hash: version1.last_event_hash.clone(),
        };

        let conflicting_event = Event {
            v: 1,
            op: Operation::Update,
            id: "task-1".to_string(),
            ts: Utc::now(),
            by: "@test".to_string(),
            branch: "main".to_string(),
            d: serde_json::json!({"title": "My update"}),
        };

        let result = write_event_with_version(&ctx, &conflicting_event, Some(&version2)).unwrap();
        assert!(matches!(result, WriteResult::Conflict { .. }));
    }
}
