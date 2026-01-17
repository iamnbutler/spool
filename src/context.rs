use anyhow::{anyhow, Context, Result};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::event::Event;

pub struct FabricContext {
    pub root: PathBuf,
    pub events_dir: PathBuf,
    pub archive_dir: PathBuf,
}

impl FabricContext {
    /// Create a new FabricContext with the given root directory
    pub fn new(root: PathBuf) -> Self {
        Self {
            events_dir: root.join("events"),
            archive_dir: root.join("archive"),
            root,
        }
    }

    pub fn discover() -> Result<Self> {
        let mut current = std::env::current_dir()?;
        loop {
            let fabric_dir = current.join(".fabric");
            if fabric_dir.is_dir() {
                return Ok(Self::new(fabric_dir));
            }
            if !current.pop() {
                return Err(anyhow!(
                    "Not in a fabric directory. Run 'fabric init' to create one."
                ));
            }
        }
    }

    pub fn index_path(&self) -> PathBuf {
        self.root.join(".index.json")
    }

    pub fn state_path(&self) -> PathBuf {
        self.root.join(".state.json")
    }

    pub fn get_event_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        if self.events_dir.is_dir() {
            for entry in fs::read_dir(&self.events_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "jsonl") {
                    files.push(path);
                }
            }
        }
        files.sort();
        Ok(files)
    }

    pub fn get_archive_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        if self.archive_dir.is_dir() {
            for entry in fs::read_dir(&self.archive_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "jsonl") {
                    files.push(path);
                }
            }
        }
        files.sort();
        Ok(files)
    }

    pub fn parse_events_from_file(&self, path: &Path) -> Result<Vec<Event>> {
        let file = File::open(path).with_context(|| format!("Failed to open {:?}", path))?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();
        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: Event = serde_json::from_str(&line)
                .with_context(|| format!("Failed to parse line {} in {:?}", line_num + 1, path))?;
            events.push(event);
        }
        Ok(events)
    }
}

pub fn init() -> Result<()> {
    let fabric_dir = PathBuf::from(".fabric");

    if fabric_dir.exists() {
        return Err(anyhow!(".fabric directory already exists"));
    }

    fs::create_dir_all(fabric_dir.join("events"))?;
    fs::create_dir_all(fabric_dir.join("archive"))?;

    let gitignore = r#"# Derived files - rebuilt from events on checkout/merge
# These are caches for fast queries, not source of truth

# Task index: maps task_id -> status, date range, file locations
.index.json

# Materialized state: current snapshot of all tasks
.state.json

# Any temporary files from tooling
*.tmp
*.bak
"#;
    fs::write(fabric_dir.join(".gitignore"), gitignore)?;

    println!("Created .fabric/");
    println!("  .fabric/events/     - Daily event logs");
    println!("  .fabric/archive/    - Monthly rollups");
    println!("  .fabric/.gitignore  - Ignores derived files");

    Ok(())
}
