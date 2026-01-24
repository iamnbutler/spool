use anyhow::{Context, Result};
use std::fs;

use crate::context::SpoolContext;

const CURRENT_VERSION: &str = "0.4.0";
const PREVIOUS_VERSION: &str = "0.3.1";

pub fn check_and_migrate(ctx: &SpoolContext) -> Result<()> {
    let version_file = ctx.version_path();
    
    // If version file exists, check if we need to migrate
    if version_file.exists() {
        let stored_version = fs::read_to_string(&version_file)
            .context("Failed to read version file")?
            .trim()
            .to_string();
        
        if stored_version == CURRENT_VERSION {
            // Already on current version, nothing to do
            return Ok(());
        }
        
        // Need to migrate from stored version to current
        migrate_from(&stored_version, ctx)?;
    } else {
        // No version file exists
        // Check if this is a fresh init (no events) or needs migration
        let event_files = ctx.get_event_files()?;
        let archive_files = ctx.get_archive_files()?;
        
        if event_files.is_empty() && archive_files.is_empty() {
            // Fresh init, just write current version
            fs::write(&version_file, CURRENT_VERSION)
                .context("Failed to write version file")?;
            return Ok(());
        }
        
        // Has events but no version file - must be 0.3.1 or earlier
        println!("Detected spool data without version marker. Assuming version {}.", PREVIOUS_VERSION);
        migrate_from(PREVIOUS_VERSION, ctx)?;
    }
    
    Ok(())
}

fn migrate_from(from_version: &str, ctx: &SpoolContext) -> Result<()> {
    match from_version {
        "0.3.1" => migrate_0_3_1_to_0_4_0(ctx)?,
        _ => {
            println!("Warning: Unknown version '{}', assuming compatible", from_version);
        }
    }
    
    // Write new version
    let version_file = ctx.version_path();
    fs::write(&version_file, CURRENT_VERSION)
        .context("Failed to write version file")?;
    
    Ok(())
}

fn migrate_0_3_1_to_0_4_0(_ctx: &SpoolContext) -> Result<()> {
    println!("Migrating spool from version 0.3.1 to 0.4.0...");
    
    // In this migration, the data format hasn't changed
    // Only the CLI API has changed (stream command structure)
    // So we just need to mark the migration as complete
    
    println!("Migration to 0.4.0 complete!");
    println!("");
    println!("BREAKING CHANGE: The 'spool stream' command syntax has changed:");
    println!("  Old: spool stream <task-id> [name]");
    println!("  New: spool stream add <task-id> <name>");
    println!("       spool stream remove <task-id>");
    println!("       spool stream list");
    println!("       spool stream show <name>");
    println!("");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_fresh_init_writes_version() {
        let temp_dir = TempDir::new().unwrap();
        let spool_dir = temp_dir.path().join(".spool");
        fs::create_dir_all(&spool_dir.join("events")).unwrap();
        fs::create_dir_all(&spool_dir.join("archive")).unwrap();
        
        let ctx = SpoolContext::new(spool_dir.clone());
        check_and_migrate(&ctx).unwrap();
        
        let version_content = fs::read_to_string(spool_dir.join(".version")).unwrap();
        assert_eq!(version_content.trim(), "0.4.0");
    }

    #[test]
    fn test_existing_version_no_migration() {
        let temp_dir = TempDir::new().unwrap();
        let spool_dir = temp_dir.path().join(".spool");
        fs::create_dir_all(&spool_dir.join("events")).unwrap();
        fs::create_dir_all(&spool_dir.join("archive")).unwrap();
        fs::write(spool_dir.join(".version"), "0.4.0").unwrap();
        
        let ctx = SpoolContext::new(spool_dir.clone());
        check_and_migrate(&ctx).unwrap();
        
        let version_content = fs::read_to_string(spool_dir.join(".version")).unwrap();
        assert_eq!(version_content.trim(), "0.4.0");
    }

    #[test]
    fn test_migration_from_0_3_1() {
        let temp_dir = TempDir::new().unwrap();
        let spool_dir = temp_dir.path().join(".spool");
        fs::create_dir_all(&spool_dir.join("events")).unwrap();
        fs::create_dir_all(&spool_dir.join("archive")).unwrap();
        
        // Create a fake event file to simulate existing data
        fs::write(
            spool_dir.join("events/2024-01-15.jsonl"),
            r#"{"v":1,"op":"create","id":"test-1","ts":"2024-01-15T10:00:00Z","by":"@test","branch":"main","d":{"title":"Test"}}"#
        ).unwrap();
        
        let ctx = SpoolContext::new(spool_dir.clone());
        check_and_migrate(&ctx).unwrap();
        
        let version_content = fs::read_to_string(spool_dir.join(".version")).unwrap();
        assert_eq!(version_content.trim(), "0.4.0");
    }
}
