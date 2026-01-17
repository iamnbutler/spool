use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use tempfile::TempDir;

use spool::context::SpoolContext;
use spool::event::{Event, Operation};
use spool::state::{build_index, materialize, Task, TaskStatus};

fn create_test_spool(num_tasks: usize) -> (TempDir, SpoolContext) {
    let temp_dir = TempDir::new().unwrap();
    let spool_dir = temp_dir.path().join(".spool");
    fs::create_dir_all(spool_dir.join("events")).unwrap();
    fs::create_dir_all(spool_dir.join("archive")).unwrap();

    let ctx = SpoolContext {
        root: spool_dir.clone(),
        events_dir: spool_dir.join("events"),
        archive_dir: spool_dir.join("archive"),
    };

    // Generate test events
    let event_file = ctx.events_dir.join("2026-01-01.jsonl");
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(&event_file)
        .unwrap();
    let mut writer = BufWriter::new(file);

    for i in 0..num_tasks {
        let event = serde_json::json!({
            "v": 1,
            "op": "create",
            "id": format!("task-{:06}", i),
            "ts": "2026-01-01T12:00:00Z",
            "by": "@bench",
            "branch": "main",
            "d": {
                "title": format!("Test task {}", i),
                "description": "A test task for benchmarking",
                "priority": if i % 3 == 0 { "high" } else if i % 3 == 1 { "medium" } else { "low" },
                "assignee": format!("@user-{}", i % 10),
                "tags": ["benchmark", "test"]
            }
        });
        writeln!(writer, "{}", event).unwrap();

        // Add a comment to half the tasks
        if i % 2 == 0 {
            let comment = serde_json::json!({
                "v": 1,
                "op": "comment",
                "id": format!("task-{:06}", i),
                "ts": "2026-01-01T13:00:00Z",
                "by": "@bench",
                "branch": "main",
                "d": {
                    "body": "This is a benchmark comment"
                }
            });
            writeln!(writer, "{}", comment).unwrap();
        }

        // Complete a quarter of the tasks
        if i % 4 == 0 {
            let complete = serde_json::json!({
                "v": 1,
                "op": "complete",
                "id": format!("task-{:06}", i),
                "ts": "2026-01-01T14:00:00Z",
                "by": "@bench",
                "branch": "main",
                "d": {
                    "resolution": "done"
                }
            });
            writeln!(writer, "{}", complete).unwrap();
        }
    }
    writer.flush().unwrap();

    (temp_dir, ctx)
}

fn bench_event_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");

    let event = Event {
        v: 1,
        op: Operation::Create,
        id: "test-1234".to_string(),
        ts: chrono::Utc::now(),
        by: "@bench".to_string(),
        branch: "main".to_string(),
        d: serde_json::json!({
            "title": "Test task",
            "description": "A test task for benchmarking with some description text",
            "priority": "high",
            "assignee": "@user",
            "tags": ["tag1", "tag2", "tag3"]
        }),
    };

    group.bench_function("event_to_json", |b| {
        b.iter(|| serde_json::to_string(black_box(&event)).unwrap())
    });

    let json = serde_json::to_string(&event).unwrap();
    group.bench_function("event_from_json", |b| {
        b.iter(|| serde_json::from_str::<Event>(black_box(&json)).unwrap())
    });

    let task = Task {
        id: "test-1234".to_string(),
        title: "Test task".to_string(),
        description: Some("A test task for benchmarking".to_string()),
        status: TaskStatus::Open,
        priority: Some("high".to_string()),
        tags: vec!["tag1".to_string(), "tag2".to_string()],
        assignee: Some("@user".to_string()),
        created: chrono::Utc::now(),
        created_by: "@bench".to_string(),
        created_branch: "main".to_string(),
        updated: chrono::Utc::now(),
        completed: None,
        resolution: None,
        parent: None,
        blocks: vec![],
        blocked_by: vec![],
        comments: vec![],
        archived: None,
    };

    group.bench_function("task_to_json", |b| {
        b.iter(|| serde_json::to_string(black_box(&task)).unwrap())
    });

    group.finish();
}

fn bench_state_materialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("materialization");

    for size in [100, 500, 1000, 5000].iter() {
        let (_temp_dir, ctx) = create_test_spool(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("materialize", size), size, |b, _| {
            b.iter(|| materialize(black_box(&ctx)).unwrap())
        });
    }

    group.finish();
}

fn bench_index_building(c: &mut Criterion) {
    let mut group = c.benchmark_group("indexing");

    for size in [100, 500, 1000, 5000].iter() {
        let (_temp_dir, ctx) = create_test_spool(*size);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("build_index", size), size, |b, _| {
            b.iter(|| build_index(black_box(&ctx)).unwrap())
        });
    }

    group.finish();
}

fn bench_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("queries");

    let (_temp_dir, ctx) = create_test_spool(1000);
    let state = materialize(&ctx).unwrap();

    group.bench_function("filter_by_status_open", |b| {
        b.iter(|| {
            state
                .tasks
                .values()
                .filter(|t| t.status == TaskStatus::Open)
                .count()
        })
    });

    group.bench_function("filter_by_assignee", |b| {
        b.iter(|| {
            state
                .tasks
                .values()
                .filter(|t| t.assignee.as_deref() == Some("@user-5"))
                .count()
        })
    });

    group.bench_function("filter_by_priority", |b| {
        b.iter(|| {
            state
                .tasks
                .values()
                .filter(|t| t.priority.as_deref() == Some("high"))
                .count()
        })
    });

    group.bench_function("filter_combined", |b| {
        b.iter(|| {
            state
                .tasks
                .values()
                .filter(|t| {
                    t.status == TaskStatus::Open
                        && t.priority.as_deref() == Some("high")
                        && t.tags.contains(&"benchmark".to_string())
                })
                .count()
        })
    });

    group.bench_function("sort_by_created", |b| {
        b.iter(|| {
            let mut tasks: Vec<_> = state.tasks.values().collect();
            tasks.sort_by_key(|t| t.created);
            tasks.len()
        })
    });

    group.finish();
}

fn bench_event_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("parsing");

    for size in [100, 500, 1000].iter() {
        let (_temp_dir, ctx) = create_test_spool(*size);
        let files = ctx.get_event_files().unwrap();
        let file = &files[0];

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("parse_events", size), size, |b, _| {
            b.iter(|| ctx.parse_events_from_file(black_box(file)).unwrap())
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_event_serialization,
    bench_state_materialization,
    bench_index_building,
    bench_queries,
    bench_event_parsing,
);

criterion_main!(benches);
