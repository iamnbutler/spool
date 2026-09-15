//! Immutable events and explicit Git interchange. Every published file is whole
//! and content addressed, so copies and merges are idempotent.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::concurrency::FileLock;
use crate::context::{initialize_directory, SpoolContext};
use crate::event::{Event, Operation};

pub fn fingerprint(event: &Event) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(event)?)))
}

/// Caller holds the board lock. Atomic publication also protects file watchers
/// and readers which only need a snapshot of one file.
pub(crate) fn publish(directory: &Path, event: &Event) -> Result<bool> {
    fs::create_dir_all(directory)?;
    let name = format!(
        "{}-{}.jsonl",
        event.ts.format("%Y%m%dT%H%M%S%.9fZ"),
        fingerprint(event)?
    );
    let destination = directory.join(name);
    let mut bytes = serde_json::to_vec(event)?;
    bytes.push(b'\n');
    if destination.exists() {
        if fs::read(&destination)? != bytes {
            bail!("Event file was modified: {}", destination.display());
        }
        return Ok(false);
    }
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    match temporary.persist_noclobber(&destination) {
        Ok(_) => {
            sync_directory(directory)?;
            Ok(true)
        }
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            if fs::read(&destination)? != bytes {
                bail!("Event file was modified: {}", destination.display());
            }
            Ok(false)
        }
        Err(error) => Err(error.error.into()),
    }
}

pub(crate) fn sync_directory(directory: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(directory)?.sync_all()?;
    Ok(())
}

/// Also understands the old daily logs and monthly archives. Sort the union,
/// not each file independently, and remove exact duplicates from old archives.
pub fn read_events(ctx: &SpoolContext, include_local: bool) -> Result<Vec<Event>> {
    let mut files = ctx.get_archive_files()?;
    files.extend(ctx.get_event_files()?);
    if include_local {
        files.extend(ctx.get_local_event_files()?);
    }
    let mut unique = BTreeMap::new();
    for file in files {
        let events = ctx.parse_events_from_file(&file)?;
        verify_filename(&file, &events)?;
        for event in events {
            if event.v != 1 {
                bail!(
                    "Unsupported event version {} in {}",
                    event.v,
                    file.display()
                );
            }
            unique.entry(fingerprint(&event)?).or_insert(event);
        }
    }
    let mut events: Vec<_> = unique.into_iter().collect();
    events.sort_by(|(hash_a, a), (hash_b, b)| {
        let rank = |op: &Operation| match op {
            Operation::CreateStream => 0,
            Operation::Create => 1,
            _ => 2,
        };
        (a.ts, rank(&a.op), hash_a).cmp(&(b.ts, rank(&b.op), hash_b))
    });
    Ok(events.into_iter().map(|(_, event)| event).collect())
}

pub(crate) fn verify_filename(path: &Path, events: &[Event]) -> Result<()> {
    if let Some((prefix, hash)) = path
        .file_stem()
        .and_then(|name| name.to_str())
        .and_then(|name| name.rsplit_once('-'))
    {
        if prefix.contains('T')
            && hash.len() == 64
            && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            && (events.len() != 1 || fingerprint(&events[0])? != hash)
        {
            bail!("Event file was modified: {}", path.display());
        }
    }
    Ok(())
}

/// Import only durable history. A claim is a lease on this machine's live board,
/// never a lock carried between independent clones by Git.
pub(crate) fn import(ctx: &SpoolContext, checkout: &Path) -> Result<usize> {
    let source = SpoolContext::new(checkout.to_path_buf());
    let mut imported = 0;
    for event in read_events(&source, false)? {
        if !event.is_local() && publish(&ctx.events_dir, &event)? {
            imported += 1;
        }
    }
    Ok(imported)
}

#[derive(Debug, Serialize)]
pub struct SyncReport {
    pub board: String,
    pub checkout: String,
    pub exported: usize,
}

pub fn sync(ctx: &SpoolContext) -> Result<SyncReport> {
    let _lock = FileLock::acquire(ctx)?;
    let checkout = ctx.checkout_root.as_ref().unwrap_or(&ctx.root);
    if checkout != &ctx.root {
        initialize_directory(checkout)?;
        import(ctx, checkout)?;
    }
    // An event already present in a legacy daily log need not be exported a
    // second time just because the live board stores it in a separate file.
    let known: HashSet<_> = read_events(&SpoolContext::new(checkout.clone()), false)?
        .iter()
        .map(fingerprint)
        .collect::<Result<_>>()?;
    let mut exported = 0;
    for event in read_events(ctx, false)? {
        if !event.is_local()
            && !known.contains(&fingerprint(&event)?)
            && publish(&checkout.join("events"), &event)?
        {
            exported += 1;
        }
    }
    Ok(SyncReport {
        board: ctx.root.display().to_string(),
        checkout: checkout.display().to_string(),
        exported,
    })
}

pub(crate) fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let directory = path.parent().context("File has no parent directory")?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    serde_json::to_writer(&mut temporary, value)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    sync_directory(directory)
}
