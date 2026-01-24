//! Migration module for spool format versions
//!
//! This module handles one-time migrations between spool format versions.
//! Migrations are isolated and run automatically when an older format is detected.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::context::SpoolContext;
use crate::event::{Event, Operation};
use crate::id::generate_id;
use crate::writer::write_event;

/// Current format version
pub const CURRENT_FORMAT_VERSION: &str = "0.4.0";

/// Version file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub format_version: String,
    pub migrated_at: Option<DateTime<Utc>>,
}

impl Default for VersionInfo {
    fn default() -> Self {
        Self {
            format_version: CURRENT_FORMAT_VERSION.to_string(),
            migrated_at: None,
        }
    }
}

/// Get the path to the version file
fn version_path(ctx: &SpoolContext) -> PathBuf {
    ctx.root.join("version.json")
}

/// Read the current version info, if it exists
pub fn read_version(ctx: &SpoolContext) -> Option<VersionInfo> {
    let path = version_path(ctx);
    if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
    } else {
        None
    }
}

/// Write version info to disk
fn write_version(ctx: &SpoolContext, version: &VersionInfo) -> Result<()> {
    let path = version_path(ctx);
    let content = serde_json::to_string_pretty(version)?;
    fs::write(&path, content).with_context(|| format!("Failed to write {:?}", path))?;
    Ok(())
}

/// Check if migration is needed and perform it
pub fn check_and_migrate(ctx: &SpoolContext) -> Result<()> {
    let version = read_version(ctx);

    match version {
        Some(v) if v.format_version == CURRENT_FORMAT_VERSION => {
            // Already at current version, nothing to do
            Ok(())
        }
        Some(v) => {
            // Old version detected, need migration
            migrate_from_version(ctx, &v.format_version)?;
            Ok(())
        }
        None => {
            // No version file - either fresh install or pre-0.4.0
            // Check if there are any events (indicates pre-0.4.0)
            let event_files = ctx.get_event_files()?;
            let archive_files = ctx.get_archive_files()?;

            if event_files.is_empty() && archive_files.is_empty() {
                // Fresh install, just write version file
                let version = VersionInfo {
                    format_version: CURRENT_FORMAT_VERSION.to_string(),
                    migrated_at: None,
                };
                write_version(ctx, &version)?;
            } else {
                // Pre-0.4.0 spool, need migration
                migrate_from_version(ctx, "0.3.1")?;
            }
            Ok(())
        }
    }
}

/// Migrate from a specific version to current
fn migrate_from_version(ctx: &SpoolContext, from_version: &str) -> Result<()> {
    eprintln!(
        "Migrating spool from {} to {}...",
        from_version, CURRENT_FORMAT_VERSION
    );

    match from_version {
        "0.3.1" | "0.3.0" | "0.2.0" | "0.1.0" => {
            migrate_0_3_1_to_0_4_0(ctx)?;
        }
        _ => {
            eprintln!(
                "  Warning: Unknown version {}, attempting migration anyway",
                from_version
            );
            migrate_0_3_1_to_0_4_0(ctx)?;
        }
    }

    // Write new version file
    let version = VersionInfo {
        format_version: CURRENT_FORMAT_VERSION.to_string(),
        migrated_at: Some(Utc::now()),
    };
    write_version(ctx, &version)?;

    // Clear cache files to force rebuild
    let _ = fs::remove_file(ctx.state_path());
    let _ = fs::remove_file(ctx.index_path());

    eprintln!("Migration complete.");
    Ok(())
}

/// Migrate from 0.3.1 to 0.4.0
///
/// This migration:
/// 1. Scans all events for unique stream names on tasks
/// 2. Creates CreateStream events for each unique stream name
/// 3. Updates tasks to reference stream IDs instead of names (via SetStream events)
fn migrate_0_3_1_to_0_4_0(ctx: &SpoolContext) -> Result<()> {
    eprintln!("  Converting implicit streams to explicit stream entities...");

    // Collect all unique stream names and the tasks that use them
    let mut stream_names: HashMap<String, Vec<String>> = HashMap::new(); // name -> [task_ids]
    let mut task_streams: HashMap<String, String> = HashMap::new(); // task_id -> stream_name

    // Process archive files first
    for file in ctx.get_archive_files()? {
        let events = ctx.parse_events_from_file(&file)?;
        collect_stream_info(&events, &mut stream_names, &mut task_streams);
    }

    // Then process event files
    for file in ctx.get_event_files()? {
        let events = ctx.parse_events_from_file(&file)?;
        collect_stream_info(&events, &mut stream_names, &mut task_streams);
    }

    if stream_names.is_empty() {
        eprintln!("  No streams found, nothing to migrate.");
        return Ok(());
    }

    eprintln!("  Found {} streams to convert", stream_names.len());

    // Get migration user info
    let user = get_migration_user();
    let branch = get_migration_branch();

    // Create a mapping of stream name -> new stream ID
    let mut stream_id_map: HashMap<String, String> = HashMap::new();

    // Create stream entities
    for stream_name in stream_names.keys() {
        let stream_id = generate_id();
        stream_id_map.insert(stream_name.clone(), stream_id.clone());

        let event = Event {
            v: 1,
            op: Operation::CreateStream,
            id: stream_id.clone(),
            ts: Utc::now(),
            by: user.clone(),
            branch: branch.clone(),
            d: serde_json::json!({
                "name": stream_name,
                "description": format!("Migrated from stream name '{}'", stream_name),
            }),
        };

        write_event(ctx, &event)?;
        eprintln!("  Created stream: {} ({})", stream_name, stream_id);
    }

    // Update tasks to use stream IDs instead of names
    for (task_id, stream_name) in &task_streams {
        if let Some(stream_id) = stream_id_map.get(stream_name) {
            let event = Event {
                v: 1,
                op: Operation::SetStream,
                id: task_id.clone(),
                ts: Utc::now(),
                by: user.clone(),
                branch: branch.clone(),
                d: serde_json::json!({
                    "stream": stream_id,
                }),
            };

            write_event(ctx, &event)?;
        }
    }

    eprintln!(
        "  Updated {} tasks with stream references",
        task_streams.len()
    );

    Ok(())
}

/// Collect stream information from events
fn collect_stream_info(
    events: &[Event],
    stream_names: &mut HashMap<String, Vec<String>>,
    task_streams: &mut HashMap<String, String>,
) {
    for event in events {
        match &event.op {
            Operation::Create => {
                // Check if task was created with a stream
                if let Some(stream) = event.d.get("stream").and_then(|v| v.as_str()) {
                    if !stream.is_empty() {
                        stream_names
                            .entry(stream.to_string())
                            .or_default()
                            .push(event.id.clone());
                        task_streams.insert(event.id.clone(), stream.to_string());
                    }
                }
            }
            Operation::SetStream => {
                // Check if stream was set on a task
                if let Some(stream) = event.d.get("stream").and_then(|v| v.as_str()) {
                    if !stream.is_empty() {
                        stream_names
                            .entry(stream.to_string())
                            .or_default()
                            .push(event.id.clone());
                        task_streams.insert(event.id.clone(), stream.to_string());
                    }
                } else {
                    // Stream was removed
                    task_streams.remove(&event.id);
                }
            }
            _ => {}
        }
    }
}

fn get_migration_user() -> String {
    // Try git config first
    if let Ok(output) = std::process::Command::new("git")
        .args(["config", "user.name"])
        .output()
    {
        if output.status.success() {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !name.is_empty() {
                return format!("@{}", name.to_lowercase().replace(' ', "-"));
            }
        }
    }

    // Fall back to USER environment variable
    if let Ok(user) = std::env::var("USER") {
        return format!("@{}", user);
    }

    "@migration".to_string()
}

fn get_migration_branch() -> String {
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
    {
        if output.status.success() {
            return String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
    }
    "main".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_context() -> (TempDir, SpoolContext) {
        let temp_dir = TempDir::new().unwrap();
        let spool_dir = temp_dir.path().join(".spool");
        fs::create_dir_all(spool_dir.join("events")).unwrap();
        fs::create_dir_all(spool_dir.join("archive")).unwrap();
        let ctx = SpoolContext::new(spool_dir);
        (temp_dir, ctx)
    }

    #[test]
    fn test_version_read_write() {
        let (_temp_dir, ctx) = create_test_context();

        // Initially no version file
        assert!(read_version(&ctx).is_none());

        // Write version
        let version = VersionInfo {
            format_version: "0.4.0".to_string(),
            migrated_at: Some(Utc::now()),
        };
        write_version(&ctx, &version).unwrap();

        // Read it back
        let read_back = read_version(&ctx).unwrap();
        assert_eq!(read_back.format_version, "0.4.0");
        assert!(read_back.migrated_at.is_some());
    }

    #[test]
    fn test_fresh_install_creates_version() {
        let (_temp_dir, ctx) = create_test_context();

        // No events, no version file
        check_and_migrate(&ctx).unwrap();

        // Should have created version file
        let version = read_version(&ctx).unwrap();
        assert_eq!(version.format_version, CURRENT_FORMAT_VERSION);
        assert!(version.migrated_at.is_none()); // Fresh install, not migrated
    }
}
