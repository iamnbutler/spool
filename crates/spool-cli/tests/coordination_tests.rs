use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Barrier};
use std::time::Duration;
use tempfile::TempDir;

fn command(directory: &Path, agent: Option<&str>, args: &[&str]) -> Command {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("spool"));
    cmd.current_dir(directory)
        .env_remove("SPOOL_AGENT")
        .env_remove("TELEPHONE_ADDR")
        .arg("--json");
    if let Some(agent) = agent {
        cmd.args(["--agent", agent]);
    }
    cmd.args(args);
    cmd
}

fn ok(directory: &Path, agent: &str, args: &[&str]) -> Value {
    let output = command(directory, Some(agent), args).output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "Unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn error(directory: &Path, agent: &str, args: &[&str], code: &str) {
    let output = command(directory, Some(agent), args).output().unwrap();
    assert!(!output.status.success(), "{args:?} unexpectedly succeeded");
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], code, "{args:?}: {error}");
}

fn board() -> TempDir {
    let temp = TempDir::new().unwrap();
    ok(temp.path(), "coordinator", &["init"]);
    temp
}

fn add(directory: &Path, title: &str) -> String {
    ok(directory, "coordinator", &["add", title])["id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn parallel(count: usize, run: impl Fn(usize) -> Output + Sync) -> Vec<Output> {
    let barrier = Arc::new(Barrier::new(count));
    std::thread::scope(|scope| {
        let jobs: Vec<_> = (0..count)
            .map(|index| {
                let barrier = barrier.clone();
                let run = &run;
                scope.spawn(move || {
                    barrier.wait();
                    run(index)
                })
            })
            .collect();
        jobs.into_iter().map(|job| job.join().unwrap()).collect()
    })
}

#[test]
fn exactly_one_process_can_claim_a_task() {
    let temp = board();
    let id = add(temp.path(), "Only one owner");
    let results = parallel(16, |index| {
        command(
            temp.path(),
            Some(&format!("agent-{index}")),
            &["claim", &id],
        )
        .output()
        .unwrap()
    });
    let mut winners = 0;
    for output in results {
        if output.status.success() {
            let task: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(task["id"], id);
            assert_eq!(task["work_status"], "in_progress");
            assert!(task["claim"]["token"].as_str().unwrap().len() >= 32);
            winners += 1;
        } else {
            assert_eq!(output.status.code(), Some(2));
            let error: Value = serde_json::from_slice(&output.stderr).unwrap();
            assert_eq!(error["error"]["code"], "claim_conflict");
        }
    }
    assert_eq!(winners, 1);
    ok(temp.path(), "coordinator", &["validate", "--strict"]);
}

#[test]
fn concurrent_next_never_allocates_the_same_task_twice() {
    let temp = board();
    let expected: HashSet<_> = (0..8)
        .map(|index| add(temp.path(), &format!("Task {index}")))
        .collect();
    let results = parallel(16, |index| {
        command(temp.path(), Some(&format!("agent-{index}")), &["next"])
            .output()
            .unwrap()
    });
    let mut claimed = HashSet::new();
    let mut idle = 0;
    for output in results {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let task: Value = serde_json::from_slice(&output.stdout).unwrap();
        if task.is_null() {
            idle += 1;
        } else {
            assert!(claimed.insert(task["id"].as_str().unwrap().to_string()));
        }
    }
    assert_eq!(claimed, expected);
    assert_eq!(idle, 8);
}

#[test]
fn expired_claims_can_be_recovered_and_old_tokens_are_fenced() {
    let temp = board();
    let id = add(temp.path(), "Recover after a crash");
    // A reservation becomes reclaimable after its worker's lease expires.
    ok(temp.path(), "coordinator", &["assign", &id, "alice"]);
    let first = ok(
        temp.path(),
        "alice",
        &["claim", &id, "--lease-seconds", "1"],
    );
    let old_token = first["claim"]["token"].as_str().unwrap();
    std::thread::sleep(Duration::from_millis(1150));
    error(
        temp.path(),
        "alice",
        &["complete", &id, "--token", old_token],
        "lease_expired",
    );
    error(
        temp.path(),
        "alice",
        &["renew", &id, "--token", old_token],
        "lease_expired",
    );
    let second = ok(temp.path(), "bob", &["claim", &id]);
    let token = second["claim"]["token"].as_str().unwrap();
    assert_ne!(token, old_token);
    for args in [
        vec!["complete", &id, "--token", old_token],
        vec!["release", &id, "--token", old_token],
        vec!["renew", &id, "--token", old_token],
    ] {
        error(temp.path(), "alice", &args, "claim_conflict");
    }
    error(
        temp.path(),
        "bob",
        &["complete", &id, "--token", old_token],
        "claim_conflict",
    );
    let renewed = ok(temp.path(), "bob", &["renew", &id, "--token", token]);
    assert_eq!(renewed["claim"]["token"], token);
    let complete = ok(
        temp.path(),
        "bob",
        &[
            "complete",
            &id,
            "--token",
            token,
            "--note",
            "Recovered and tested",
        ],
    );
    assert_eq!(complete["status"], "complete");
    assert!(complete.get("claim").is_none());
    assert_eq!(complete["comments"][0]["body"], "Recovered and tested");
    error(
        temp.path(),
        "bob",
        &["update", &id, "--title", "Stale edit", "--token", token],
        "stale_token",
    );
}

#[test]
fn release_and_reclaim_with_the_same_identity_rotates_the_token() {
    let temp = board();
    let id = add(temp.path(), "Resume within a session");
    let first = ok(temp.path(), "alice", &["claim", &id]);
    let old = first["claim"]["token"].as_str().unwrap();
    ok(
        temp.path(),
        "alice",
        &["release", &id, "--token", old, "--note", "Paused at parser"],
    );
    let second = ok(temp.path(), "alice", &["claim", &id]);
    assert_ne!(second["claim"]["token"], old);
    error(
        temp.path(),
        "alice",
        &["complete", &id, "--token", old],
        "claim_conflict",
    );
}

#[test]
fn all_mutators_respect_claims_including_the_tui_writer_api() {
    let temp = board();
    let id = add(temp.path(), "Protected work");
    let claimed = ok(temp.path(), "alice", &["claim", &id]);
    let token = claimed["claim"]["token"].as_str().unwrap();
    for args in [
        vec!["assign", &id, "bob"],
        vec!["free", &id],
        vec!["complete", &id],
        vec!["update", &id, "--title", "Overwrite"],
        vec!["handoff", &id, "--to", "bob", "--note", "Steal"],
    ] {
        error(temp.path(), "bob", &args, "claim_conflict");
    }
    let ctx = spool::SpoolContext::discover_from(temp.path()).unwrap();
    assert!(spool::writer::complete_task(&ctx, &id, None, "alice", "main").is_err());
    assert!(spool::writer::assign_task(&ctx, &id, Some("bob"), "bob", "main").is_err());
    ok(
        temp.path(),
        "reviewer",
        &["comment", &id, "Consider the boundary case"],
    );
    let updated = ok(
        temp.path(),
        "alice",
        &["update", &id, "--title", "Owned edit", "--token", token],
    );
    assert_eq!(updated["title"], "Owned edit");
}

#[test]
fn dependency_queue_handles_cycles_completion_and_both_edge_spellings() {
    let temp = board();
    let a = add(temp.path(), "Design");
    let b = add(temp.path(), "Implement");
    let c = add(temp.path(), "Review");
    ok(temp.path(), "coordinator", &["block", &b, "--by", &a]);
    ok(temp.path(), "coordinator", &["link", &b, "blocks", &c]);
    error(
        temp.path(),
        "coordinator",
        &["block", &a, "--by", &c],
        "cycle",
    );
    error(
        temp.path(),
        "coordinator",
        &["block", &a, "--by", &a],
        "cycle",
    );
    error(temp.path(), "worker", &["claim", &b], "blocked");
    error(temp.path(), "worker", &["complete", &b], "blocked");
    let ready = ok(temp.path(), "worker", &["ready"]);
    assert_eq!(ready.as_array().unwrap().len(), 1);
    assert_eq!(ready[0]["id"], a);
    ok(temp.path(), "coordinator", &["complete", &a]);
    assert_eq!(ok(temp.path(), "worker", &["ready"])[0]["id"], b);
    ok(temp.path(), "coordinator", &["unblock", &c, "--by", &b]);
    assert!(ok(temp.path(), "worker", &["show", &c])["blockers"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(ok(temp.path(), "worker", &["show", &b])["blocks"]
        .as_array()
        .unwrap()
        .is_empty());
    ok(temp.path(), "coordinator", &["validate", "--strict"]);
}

#[test]
fn next_respects_priority_reservations_and_stream_filters() {
    let temp = board();
    ok(temp.path(), "coordinator", &["stream", "add", "API"]);
    let low = ok(
        temp.path(),
        "coordinator",
        &["add", "Low", "-p", "p3", "--stream", "api"],
    );
    let high = ok(
        temp.path(),
        "coordinator",
        &["add", "High", "-p", "p0", "--stream", "api", "-t", "rust"],
    );
    ok(
        temp.path(),
        "coordinator",
        &[
            "add",
            "Reserved",
            "-p",
            "p0",
            "-a",
            "someone-else",
            "--stream",
            "api",
        ],
    );
    assert_eq!(
        ok(
            temp.path(),
            "worker",
            &["next", "--stream", "api", "--tag", "rust"]
        )["id"],
        high["id"]
    );
    assert_eq!(
        ok(temp.path(), "worker", &["next", "--stream", "api"])["id"],
        low["id"]
    );
    assert!(ok(temp.path(), "worker", &["next", "--stream", "api"]).is_null());
    error(
        temp.path(),
        "worker",
        &["ready", "--stream", "typo"],
        "not_found",
    );
    error(
        temp.path(),
        "worker",
        &["list", "--stream-name", "typo"],
        "not_found",
    );
}

#[test]
fn handoff_preserves_context_and_is_ready_only_for_the_recipient() {
    let temp = board();
    let id = add(temp.path(), "Review the parser");
    let claimed = ok(temp.path(), "codex:one", &["claim", &id]);
    let token = claimed["claim"]["token"].as_str().unwrap();
    let handoff = ok(
        temp.path(),
        "codex:one",
        &[
            "handoff",
            &id,
            "--token",
            token,
            "--to",
            "claude:two",
            "--note",
            "Check Unicode boundaries",
            "--ref",
            "src/parser.rs",
        ],
    );
    assert_eq!(handoff["assignee"], "claude:two");
    assert!(handoff.get("claim").is_none());
    assert_eq!(handoff["comments"][0]["body"], "Check Unicode boundaries");
    assert_eq!(handoff["comments"][0]["ref"], "src/parser.rs");
    assert_eq!(handoff["notification"]["to"], "claude:two");
    assert!(handoff["notification"]["message"]
        .as_str()
        .unwrap()
        .contains(&id));
    assert!(ok(temp.path(), "codex:one", &["next"]).is_null());
    assert_eq!(ok(temp.path(), "claude:two", &["next"])["id"], id);
}

#[test]
fn identity_is_explicit_and_telephone_addresses_are_preserved() {
    let temp = board();
    let id = add(temp.path(), "Needs an individual identity");
    let output = command(temp.path(), None, &["claim", &id])
        .output()
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stderr).unwrap()["error"]["code"],
        "identity_required"
    );
    let mut cmd = command(temp.path(), None, &["claim", &id]);
    cmd.env("TELEPHONE_ADDR", "codex:session-abc");
    let output = cmd.output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["claim"]["agent"],
        "codex:session-abc"
    );
    let output = command(temp.path(), Some("explicit"), &["whoami"])
        .env("SPOOL_AGENT", "environment")
        .env("TELEPHONE_ADDR", "telephone")
        .output()
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["agent"],
        "explicit"
    );
}

#[test]
fn fresh_reads_ignore_stale_or_corrupt_cache_files_and_lists_stay_compact() {
    let temp = board();
    let id = add(temp.path(), "Fresh task");
    ok(temp.path(), "coordinator", &["rebuild"]);
    fs::write(temp.path().join(".spool/.state.json"), "not JSON").unwrap();
    ok(
        temp.path(),
        "coordinator",
        &[
            "update",
            &id,
            "--title",
            "最新のタスク 🧵",
            "--description",
            "Long context stays in show",
        ],
    );
    ok(
        temp.path(),
        "coordinator",
        &["comment", &id, "Durable context"],
    );
    let list = ok(temp.path(), "worker", &["list"]);
    assert_eq!(list[0]["title"], "最新のタスク 🧵");
    assert!(list[0].get("description").is_none());
    assert!(list[0].get("comments").is_none());
    let show = ok(temp.path(), "worker", &["show", &id]);
    assert_eq!(show["comments"][0]["body"], "Durable context");
    error(
        temp.path(),
        "worker",
        &["add", "", "--priority", "p1"],
        "invalid_input",
    );
    error(
        temp.path(),
        "worker",
        &["add", "Bad priority", "--priority", "urgent"],
        "invalid_input",
    );
}

fn git(directory: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(directory)
        .args([
            "-c",
            "user.name=Spool Test",
            "-c",
            "user.email=spool@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn linked_worktrees_share_one_board_but_clones_do_not_inherit_claims() {
    let temp = TempDir::new().unwrap();
    let repository = temp.path().join("repository");
    let worker = temp.path().join("worker");
    let clone = temp.path().join("clone");
    fs::create_dir(&repository).unwrap();
    git(&repository, &["init", "-q"]);
    fs::write(repository.join("README"), "fixture").unwrap();
    git(&repository, &["add", "README"]);
    git(&repository, &["commit", "-qm", "Initial commit"]);
    // This worktree predates spool init and has no .spool directory.
    git(
        &repository,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worker",
            worker.to_str().unwrap(),
        ],
    );
    ok(&repository, "coordinator", &["init"]);
    let id = add(&repository, "Shared before sync");
    assert_eq!(ok(&worker, "worker", &["show", &id])["id"], id);
    let child_id = add(&worker, "Created in worker");
    assert_eq!(
        ok(&repository, "coordinator", &["show", &child_id])["id"],
        child_id
    );
    let results = parallel(12, |index| {
        command(
            if index % 2 == 0 { &repository } else { &worker },
            Some(&format!("worker-{index}")),
            &["claim", &id],
        )
        .output()
        .unwrap()
    });
    assert_eq!(
        results
            .iter()
            .filter(|output| output.status.success())
            .count(),
        1
    );
    for output in results.iter().filter(|output| !output.status.success()) {
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stderr).unwrap()["error"]["code"],
            "claim_conflict"
        );
    }
    assert_eq!(
        ok(&repository, "coordinator", &["status"])["board"],
        ok(&worker, "worker", &["status"])["board"]
    );
    ok(&repository, "coordinator", &["sync"]);
    let snapshot = fs::read_dir(repository.join(".spool/events"))
        .unwrap()
        .map(|entry| fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect::<String>();
    assert!(!snapshot.contains("\"op\":\"claim\""));
    assert!(!snapshot.contains("\"token\""));
    git(&repository, &["add", ".spool"]);
    git(&repository, &["commit", "-qm", "Task history"]);
    git(
        &repository,
        &[
            "clone",
            "-q",
            "--no-local",
            repository.to_str().unwrap(),
            clone.to_str().unwrap(),
        ],
    );
    let cloned_task = ok(&clone, "remote-worker", &["show", &id]);
    assert!(cloned_task.get("claim").is_none());
    assert_eq!(
        ok(&clone, "remote-worker", &["show", &child_id])["id"],
        child_id
    );
    // Export from a worktree which still has the older Git snapshot.
    ok(&worker, "worker", &["sync"]);
    ok(&worker, "worker", &["sync"]);
    assert_eq!(
        ok(&worker, "worker", &["list"])[0]["id"],
        ok(&repository, "worker", &["list"])[0]["id"]
    );
}

#[test]
fn crashed_lock_owner_does_not_wedge_the_board() {
    let temp = board();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "lock_owner_helper", "--nocapture"])
        .env("SPOOL_TEST_LOCK_BOARD", temp.path().join(".spool"))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    loop {
        let mut line = String::new();
        assert!(
            reader.read_line(&mut line).unwrap() > 0,
            "Lock helper exited before acquiring its lock"
        );
        if line.contains("LOCK_ACQUIRED") {
            break;
        }
    }
    child.kill().unwrap();
    child.wait().unwrap();
    add(temp.path(), "Writes work after the lock owner died");
}

#[test]
fn lock_owner_helper() {
    let Some(directory) = std::env::var_os("SPOOL_TEST_LOCK_BOARD") else {
        return;
    };
    let ctx = spool::SpoolContext::new(directory.into());
    let _lock = spool::concurrency::FileLock::acquire(&ctx).unwrap();
    println!("LOCK_ACQUIRED");
    std::io::stdout().flush().unwrap();
    std::thread::sleep(Duration::from_secs(30));
}

#[test]
fn archive_duplicates_and_missing_dependencies_do_not_make_work_ready() {
    let temp = board();
    let event = json!({"v":1,"op":"create","id":"legacy","ts":"2024-01-01T00:00:00Z","by":"agent","branch":"main","d":{"title":"Legacy task","blocked_by":["missing"]}});
    let comment = json!({"v":1,"op":"comment","id":"legacy","ts":"2024-01-01T01:00:00Z","by":"agent","branch":"main","d":{"body":"Only once"}});
    let content = format!("{event}\n{comment}\n");
    fs::write(temp.path().join(".spool/events/legacy.jsonl"), &content).unwrap();
    fs::write(temp.path().join(".spool/archive/legacy.jsonl"), &content).unwrap();
    let task = ok(temp.path(), "worker", &["show", "legacy", "--events"]);
    assert_eq!(task["comments"].as_array().unwrap().len(), 1);
    assert_eq!(task["events"].as_array().unwrap().len(), 2);
    assert_eq!(task["work_status"], "blocked");
    assert!(ok(temp.path(), "worker", &["next"]).is_null());
    error(temp.path(), "worker", &["claim", "legacy"], "blocked");
}

#[test]
fn large_concurrent_notes_are_complete_and_durable() {
    let temp = board();
    let id = add(temp.path(), "Concurrent notes");
    let results = parallel(8, |index| {
        let body = format!("{index}:{}", "🧵 shared context ".repeat(2048));
        command(
            temp.path(),
            Some(&format!("writer-{index}")),
            &["comment", &id, &body],
        )
        .output()
        .unwrap()
    });
    assert!(results.iter().all(|result| result.status.success()));
    let task = ok(temp.path(), "coordinator", &["show", &id]);
    let notes = task["comments"].as_array().unwrap();
    assert_eq!(notes.len(), 8);
    for index in 0..8 {
        let expected = format!("{index}:{}", "🧵 shared context ".repeat(2048));
        assert!(notes.iter().any(|note| note["body"] == expected));
    }
    ok(temp.path(), "coordinator", &["validate", "--strict"]);
}

#[test]
fn simultaneous_dependency_edits_cannot_create_a_cycle() {
    let temp = board();
    let a = add(temp.path(), "A");
    let b = add(temp.path(), "B");
    let results = parallel(2, |index| {
        let (task, prerequisite) = if index == 0 { (&a, &b) } else { (&b, &a) };
        command(
            temp.path(),
            Some("coordinator"),
            &["block", task, "--by", prerequisite],
        )
        .output()
        .unwrap()
    });
    assert_eq!(
        results
            .iter()
            .filter(|result| result.status.success())
            .count(),
        1
    );
    let rejected = results
        .iter()
        .find(|result| !result.status.success())
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&rejected.stderr).unwrap()["error"]["code"],
        "cycle"
    );
    ok(temp.path(), "coordinator", &["validate", "--strict"]);
}

#[test]
fn modified_event_files_and_newer_formats_fail_closed() {
    let temp = board();
    let id = add(temp.path(), "Immutable history");
    let path = fs::read_dir(temp.path().join(".spool/events"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let original = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        original.replace("Immutable history", "Changed in place"),
    )
    .unwrap();
    let output = command(temp.path(), Some("worker"), &["claim", &id])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        serde_json::from_slice::<Value>(&output.stderr).unwrap()["error"]["message"]
            .as_str()
            .unwrap()
            .contains("modified")
    );
    fs::write(&path, original).unwrap();
    let future = r#"{"format_version":"999.0.0","migrated_at":null}"#;
    fs::write(temp.path().join(".spool/version.json"), future).unwrap();
    let output = command(temp.path(), Some("worker"), &["show", &id])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        fs::read_to_string(temp.path().join(".spool/version.json")).unwrap(),
        future
    );
}

#[test]
fn both_dependency_spellings_guard_the_dependent_tasks_claim() {
    let temp = board();
    let prerequisite = add(temp.path(), "Prerequisite");
    let dependent = add(temp.path(), "Dependent");
    let claim = ok(temp.path(), "worker", &["claim", &dependent]);
    let token = claim["claim"]["token"].as_str().unwrap();
    error(
        temp.path(),
        "other",
        &["link", &prerequisite, "blocks", &dependent],
        "claim_conflict",
    );
    ok(
        temp.path(),
        "worker",
        &[
            "link",
            &prerequisite,
            "blocks",
            &dependent,
            "--token",
            token,
        ],
    );
    error(
        temp.path(),
        "other",
        &["unblock", &dependent, "--by", &prerequisite],
        "claim_conflict",
    );
    ok(
        temp.path(),
        "worker",
        &[
            "unlink",
            &prerequisite,
            "blocks",
            &dependent,
            "--token",
            token,
        ],
    );
    assert!(ok(temp.path(), "worker", &["show", &dependent])["blockers"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn abandoned_blocked_work_can_be_released_without_impersonating_its_owner() {
    let temp = board();
    let prerequisite = add(temp.path(), "Newly discovered prerequisite");
    let id = add(temp.path(), "Abandoned work");
    let claim = ok(
        temp.path(),
        "alice",
        &["claim", &id, "--lease-seconds", "1"],
    );
    let token = claim["claim"]["token"].as_str().unwrap();
    ok(
        temp.path(),
        "alice",
        &["block", &id, "--by", &prerequisite, "--token", token],
    );
    error(
        temp.path(),
        "coordinator",
        &["release", &id],
        "claim_conflict",
    );
    std::thread::sleep(Duration::from_millis(1150));
    error(temp.path(), "bob", &["claim", &id], "blocked");
    let released = ok(
        temp.path(),
        "coordinator",
        &["release", &id, "--note", "Worker exited; recover the plan"],
    );
    assert!(released.get("claim").is_none());
    assert_eq!(released["comments"][0]["by"], "coordinator");
    ok(
        temp.path(),
        "coordinator",
        &["unblock", &id, "--by", &prerequisite],
    );
    assert_eq!(
        ok(temp.path(), "bob", &["claim", &id])["claim"]["agent"],
        "bob"
    );
    error(
        temp.path(),
        "alice",
        &["release", &id, "--token", token],
        "claim_conflict",
    );
}
