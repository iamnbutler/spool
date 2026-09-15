use anyhow::{anyhow, Context, Result};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::concurrency::FileLock;
use crate::event::Event;
use crate::migration;

#[derive(Debug, Clone)]
pub struct SpoolContext {
    pub root: PathBuf,
    pub events_dir: PathBuf,
    pub archive_dir: PathBuf,
    /// The current worktree's Git interchange directory, when using a shared board.
    pub checkout_root: Option<PathBuf>,
}

impl SpoolContext {
    /// Create a new SpoolContext with the given root directory
    pub fn new(root: PathBuf) -> Self {
        Self {
            events_dir: root.join("events"),
            archive_dir: root.join("archive"),
            root,
            checkout_root: None,
        }
    }

    pub fn discover() -> Result<Self> {
        Self::discover_from(&std::env::current_dir()?)
    }

    pub fn discover_from(directory: &Path) -> Result<Self> {
        let mut current = directory.canonicalize()?;
        loop {
            let spool_dir = current.join(".spool");
            // A repository is a discovery boundary. Linked worktrees may predate
            // spool init, but can still join an already initialized shared board.
            if current.join(".git").exists() {
                let output = Command::new("git")
                    .current_dir(&current)
                    .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
                    .output()?;
                if !output.status.success() {
                    return Err(anyhow!(
                        "Cannot resolve the repository's Git common directory"
                    ));
                }
                let common = PathBuf::from(String::from_utf8(output.stdout)?.trim());
                let board = common.join("spool");
                if !spool_dir.is_dir() && !board.join("events").is_dir() {
                    break;
                }
                if spool_dir.is_dir() {
                    migration::check_and_migrate(&Self::new(spool_dir.clone()))?;
                    initialize_directory(&spool_dir)?;
                }
                initialize_directory(&board)?;
                let mut ctx = Self::new(board.canonicalize()?);
                ctx.checkout_root = Some(spool_dir.clone());
                migration::check_and_migrate(&ctx)?;
                let _lock = FileLock::acquire(&ctx)?;
                crate::store::import(&ctx, &spool_dir)?;
                return Ok(ctx);
            }
            if spool_dir.is_dir() {
                let ctx = Self::new(spool_dir);
                migration::check_and_migrate(&ctx)?;
                initialize_directory(&ctx.root)?;
                return Ok(ctx);
            }
            if !current.pop() {
                break;
            }
        }
        Err(anyhow!(
            "Not in a spool directory. Run 'spool init' to create one."
        ))
    }

    pub fn index_path(&self) -> PathBuf {
        self.root.join(".index.json")
    }

    pub fn state_path(&self) -> PathBuf {
        self.root.join(".state.json")
    }

    pub fn get_event_files(&self) -> Result<Vec<PathBuf>> {
        event_files(&self.events_dir)
    }

    pub fn get_archive_files(&self) -> Result<Vec<PathBuf>> {
        event_files(&self.archive_dir)
    }

    pub fn local_events_dir(&self) -> PathBuf {
        self.root.join(".local/events")
    }

    pub fn get_local_event_files(&self) -> Result<Vec<PathBuf>> {
        event_files(&self.local_events_dir())
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

fn event_files(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if directory.is_dir() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_file() && path.extension().is_some_and(|ext| ext == "jsonl") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

pub fn init() -> Result<()> {
    let spool_dir = PathBuf::from(".spool");

    if spool_dir.exists() {
        return Err(anyhow!(".spool directory already exists"));
    }

    initialize_directory(&spool_dir)?;
    println!("Created .spool/");
    println!("Run 'spool prime' for the agent workflow; 'spool sync' before committing.");
    Ok(())
}

pub(crate) fn initialize_directory(root: &Path) -> Result<()> {
    fs::create_dir_all(root.join("events"))?;
    fs::create_dir_all(root.join("archive"))?;
    let ignore_path = root.join(".gitignore");
    let mut ignore = match fs::read_to_string(&ignore_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    let previous = ignore.clone();
    for pattern in [
        ".index.json",
        ".state.json",
        ".lock",
        ".local/",
        ".tmp*",
        "*.tmp",
        "*.bak",
    ] {
        if !ignore.lines().any(|line| line == pattern) {
            if !ignore.is_empty() && !ignore.ends_with('\n') {
                ignore.push('\n');
            }
            ignore.push_str(pattern);
            ignore.push('\n');
        }
    }
    if ignore != previous {
        fs::write(ignore_path, ignore)?;
    }
    if !root.join("version.json").exists() {
        crate::store::write_json(
            &root.join("version.json"),
            &migration::VersionInfo::default(),
        )?;
    }
    Ok(())
}
