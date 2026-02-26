use anyhow::{anyhow, Result};
use chrono::DateTime;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::context::SpoolContext;
use crate::state::materialize;

#[derive(Debug)]
pub struct ValidationResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn validate(ctx: &SpoolContext, strict: bool) -> Result<ValidationResult> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut created_ids: HashSet<String> = HashSet::new();

    // Validate event files
    for file in ctx.get_event_files()? {
        let filename = file
            .file_name()
            .ok_or_else(|| {
                anyhow::anyhow!("event file path has no filename component: {:?}", file)
            })?
            .to_string_lossy()
            .to_string();
        validate_event_file(
            &file,
            &filename,
            &mut errors,
            &mut warnings,
            &mut created_ids,
        )?;
    }

    // Validate archive files
    for file in ctx.get_archive_files()? {
        let filename = file
            .file_name()
            .ok_or_else(|| {
                anyhow::anyhow!("archive file path has no filename component: {:?}", file)
            })?
            .to_string_lossy()
            .to_string();
        validate_event_file(
            &file,
            &filename,
            &mut errors,
            &mut warnings,
            &mut created_ids,
        )?;
    }

    // Only check for orphaned references if no errors occurred
    // (materialize will fail on invalid events)
    if errors.is_empty() {
        let state = materialize(ctx)?;
        for task in state.tasks.values() {
            for blocked_by in &task.blocked_by {
                if !state.tasks.contains_key(blocked_by) {
                    warnings.push(format!(
                        "Task {} references non-existent blocked_by: {}",
                        task.id, blocked_by
                    ));
                }
            }
            for blocks in &task.blocks {
                if !state.tasks.contains_key(blocks) {
                    warnings.push(format!(
                        "Task {} references non-existent blocks: {}",
                        task.id, blocks
                    ));
                }
            }
            if let Some(parent) = &task.parent {
                if !state.tasks.contains_key(parent) {
                    warnings.push(format!(
                        "Task {} references non-existent parent: {}",
                        task.id, parent
                    ));
                }
            }
        }
    }

    let result = ValidationResult { errors, warnings };

    // Print results
    if result.errors.is_empty() && result.warnings.is_empty() {
        println!("Validation passed. No issues found.");
    } else {
        if !result.errors.is_empty() {
            println!("Errors ({}):", result.errors.len());
            for error in &result.errors {
                println!("  ERROR: {}", error);
            }
        }
        if !result.warnings.is_empty() {
            println!("Warnings ({}):", result.warnings.len());
            for warning in &result.warnings {
                println!("  WARN: {}", warning);
            }
        }

        if strict && !result.errors.is_empty() {
            return Err(anyhow!(
                "Validation failed with {} errors",
                result.errors.len()
            ));
        }
        if strict && !result.warnings.is_empty() {
            return Err(anyhow!(
                "Validation failed with {} warnings (--strict mode)",
                result.warnings.len()
            ));
        }
    }

    Ok(result)
}

fn validate_event_file(
    path: &Path,
    filename: &str,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
    created_ids: &mut HashSet<String>,
) -> Result<()> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            errors.push(format!("Cannot open {}: {}", filename, e));
            return Ok(());
        }
    };
    let reader = BufReader::new(file);

    for (line_num, line) in reader.lines().enumerate() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                errors.push(format!("{}:{}: Read error: {}", filename, line_num + 1, e));
                continue;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        let event: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!(
                    "{}:{}: Invalid JSON: {}",
                    filename,
                    line_num + 1,
                    e
                ));
                continue;
            }
        };

        // Check required fields
        let required = ["v", "op", "id", "ts", "by", "branch", "d"];
        for field in required {
            if event.get(field).is_none() {
                errors.push(format!(
                    "{}:{}: Missing required field '{}'",
                    filename,
                    line_num + 1,
                    field
                ));
            }
        }

        // Check schema version
        if let Some(v) = event.get("v").and_then(|v| v.as_u64()) {
            if v != 1 {
                warnings.push(format!(
                    "{}:{}: Unknown schema version {}",
                    filename,
                    line_num + 1,
                    v
                ));
            }
        }

        // Track creates for orphan detection
        if let Some(op) = event.get("op").and_then(|v| v.as_str()) {
            if let Some(id) = event.get("id").and_then(|v| v.as_str()) {
                if op == "create" {
                    if created_ids.contains(id) {
                        warnings.push(format!(
                            "{}:{}: Duplicate create for task {}",
                            filename,
                            line_num + 1,
                            id
                        ));
                    }
                    created_ids.insert(id.to_string());
                } else if !created_ids.contains(id) {
                    warnings.push(format!(
                        "{}:{}: Event for task {} before create",
                        filename,
                        line_num + 1,
                        id
                    ));
                }
            }
        }

        // Validate timestamp format
        if let Some(ts) = event.get("ts").and_then(|v| v.as_str()) {
            if DateTime::parse_from_rfc3339(ts).is_err() {
                errors.push(format!(
                    "{}:{}: Invalid timestamp format: {}",
                    filename,
                    line_num + 1,
                    ts
                ));
            }
        }
    }

    Ok(())
}
