use crate::context::SpoolContext;
use crate::engine::{self, Identity, ReadyFilter, SpoolError};
use crate::event::Operation;
use crate::state::{load_or_materialize_state, State, Task, TaskStatus};
use anyhow::{anyhow, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "spool")]
#[command(version, about = "A shared task board for coding agents")]
pub struct Cli {
    /// Unique identity for this session (or SPOOL_AGENT / TELEPHONE_ADDR)
    #[arg(long, global = true)]
    pub agent: Option<String>,
    /// Current claim token, returned by claim or next
    #[arg(long, global = true)]
    pub token: Option<String>,
    /// Lease length for claim, next, and renew (1..86400 seconds)
    #[arg(long, global = true, default_value_t = engine::DEFAULT_LEASE_SECONDS)]
    pub lease_seconds: u32,
    /// Emit one compact JSON value; failures emit JSON on stderr
    #[arg(long, global = true)]
    pub json: bool,
    /// Run in this directory
    #[arg(short = 'C', long, global = true)]
    pub directory: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize .spool/ directory structure
    Init,
    /// Print the short agent workflow and Telephone integration guide
    Prime,
    /// Show the effective agent identity and its source
    Whoami,
    /// Summarize the board and this agent's work
    Status,
    /// Export the shared board's durable events to this checkout for Git
    Sync,
    /// List unclaimed work with completed prerequisites, in priority order
    Ready {
        #[arg(long)]
        stream: Option<String>,
        #[arg(short, long)]
        tag: Option<String>,
        #[arg(short = 'n', long)]
        limit: Option<usize>,
    },
    /// Atomically select and claim the next ready task; null means no work
    Next {
        #[arg(long)]
        stream: Option<String>,
        #[arg(short, long)]
        tag: Option<String>,
    },
    /// Extend the current claim (requires --token)
    Renew { id: String },
    /// Return work to the queue with a note (a live claim requires --token)
    #[command(visible_alias = "stop")]
    Release {
        id: String,
        #[arg(long, default_value = "Released to the queue")]
        note: String,
    },
    /// Transfer a task with durable context; returns a Telephone message to send
    Handoff {
        id: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        note: String,
        #[arg(long = "ref")]
        reference: Option<String>,
    },
    /// Leave a durable note, optionally referring to a PR, file, or message
    #[command(visible_alias = "note")]
    Comment {
        id: String,
        body: String,
        #[arg(long = "ref")]
        reference: Option<String>,
    },
    /// Add a prerequisite: the task cannot start until --by is complete
    Block {
        id: String,
        #[arg(long)]
        by: String,
    },
    /// Remove a prerequisite
    Unblock {
        id: String,
        #[arg(long)]
        by: String,
    },
    /// Add a relationship (blocks, blocked_by, parent)
    Link {
        id: String,
        #[arg(value_parser = ["blocks", "blocked_by", "parent"])]
        relation: String,
        target: String,
    },
    /// Remove a relationship
    Unlink {
        id: String,
        #[arg(value_parser = ["blocks", "blocked_by", "parent"])]
        relation: String,
        target: String,
    },
    /// Create a new task
    Add {
        /// Task title
        title: String,
        /// Task description
        #[arg(short, long)]
        description: Option<String>,
        /// Priority (p0, p1, p2, p3)
        #[arg(short, long, value_parser = ["p0", "p1", "p2", "p3"])]
        priority: Option<String>,
        /// Assignee (@username)
        #[arg(short, long)]
        assignee: Option<String>,
        /// Tags (can be used multiple times)
        #[arg(short, long)]
        tag: Vec<String>,
        /// Stream to add the task to
        #[arg(long)]
        stream: Option<String>,
    },
    /// List tasks with optional filtering
    List {
        /// Status filter: open, complete, or all (default: open)
        #[arg(short, long, default_value = "open", value_parser = ["open", "complete", "all", "in_progress", "blocked"])]
        status: String,
        /// Filter by assignee
        #[arg(short, long)]
        assignee: Option<String>,
        /// Filter by tag
        #[arg(short, long)]
        tag: Option<String>,
        /// Filter by priority
        #[arg(short, long)]
        priority: Option<String>,
        /// Filter by stream ID
        #[arg(long, conflicts_with = "stream_name")]
        stream: Option<String>,
        /// Filter by stream name
        #[arg(long, conflicts_with = "stream")]
        stream_name: Option<String>,
        /// Show only tasks without a stream
        #[arg(long, conflicts_with_all = ["stream", "stream_name"])]
        no_stream: bool,
        /// Filter by this agent's assignment or live claim
        #[arg(long)]
        mine: bool,
        /// Search title, description, and tags
        #[arg(short = 'q', long)]
        search: Option<String>,
        #[arg(short = 'n', long)]
        limit: Option<usize>,
        /// Output format: table, json, or ids
        #[arg(short, long, default_value = "table", value_parser = ["table", "json", "ids"])]
        format: String,
    },
    /// Show details of a specific task
    Show {
        /// Task ID to show
        id: String,
        /// Show raw event history
        #[arg(long)]
        events: bool,
    },
    /// Rebuild .index.json and .state.json from events
    Rebuild,
    /// Archive completed tasks older than N days
    Archive {
        /// Days after completion to archive (default: 30)
        #[arg(short, long, default_value = "30")]
        days: u32,
        /// Show what would be archived without doing it
        #[arg(long)]
        dry_run: bool,
    },
    /// Validate event files for correctness
    Validate {
        /// Fail on warnings too
        #[arg(long)]
        strict: bool,
    },
    /// Mark a task as complete
    Complete {
        /// Task ID to complete
        id: String,
        /// Resolution: done, wontfix, duplicate, obsolete
        #[arg(short, long, default_value = "done", value_parser = ["done", "wontfix", "duplicate", "obsolete"])]
        resolution: String,
        #[arg(long)]
        note: Option<String>,
        #[arg(long = "ref", requires = "note")]
        reference: Option<String>,
    },
    /// Reopen a completed task
    Reopen {
        /// Task ID to reopen
        id: String,
    },
    /// Update a task's fields
    Update {
        /// Task ID to update
        id: String,
        /// New title
        #[arg(short, long)]
        title: Option<String>,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        /// New priority
        #[arg(short, long, value_parser = ["p0", "p1", "p2", "p3"])]
        priority: Option<String>,
        /// Move to stream (use "" to remove from stream)
        #[arg(long)]
        stream: Option<String>,
        #[arg(long)]
        add_tag: Vec<String>,
        #[arg(long)]
        remove_tag: Vec<String>,
    },
    /// Assign a task to a user
    Assign {
        /// Task ID to assign
        id: String,
        /// Assignee (@username)
        assignee: String,
    },
    /// Acquire a task lease (requires a session identity)
    #[command(visible_alias = "start")]
    Claim {
        /// Task ID to claim
        id: String,
    },
    /// Manage streams (workstreams/projects)
    #[command(visible_alias = "streams")]
    Stream {
        #[command(subcommand)]
        command: StreamCommands,
    },
    /// Unassign a task
    Free {
        /// Task ID to free
        id: String,
    },
}

/// Stream subcommands for managing workstreams/projects
#[derive(Subcommand)]
pub enum StreamCommands {
    /// Create a new stream
    Add {
        /// Stream name
        name: String,
        /// Stream description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// List all streams
    List {
        /// Output format: table, json, or ids
        #[arg(short, long, default_value = "table")]
        format: String,
    },
    /// Show details of a stream and its tasks
    Show {
        /// Stream ID
        id: Option<String>,
        /// Stream name (alternative to ID)
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Update stream metadata
    Update {
        /// Stream ID
        id: String,
        /// New name
        #[arg(short, long)]
        name: Option<String>,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Delete a stream (must have no tasks assigned)
    Delete {
        /// Stream ID
        id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Table,
    Json,
    Ids,
}

impl OutputFormat {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "json" => OutputFormat::Json,
            "ids" => OutputFormat::Ids,
            _ => OutputFormat::Table,
        }
    }
}

struct Reply {
    value: Value,
    text: String,
}

impl Reply {
    fn new(value: Value, text: impl Into<String>) -> Self {
        Self {
            value,
            text: text.into(),
        }
    }
}

const PRIME: &str = r#"Spool coordinates tasks; Telephone carries messages.

Use one identity per agent session:
  export SPOOL_AGENT=<unique-session-name>
  # TELEPHONE_ADDR is also accepted, or pass --agent explicitly.

Work loop:
  spool ready --json                 # unclaimed tasks with resolved prerequisites
  spool next --json                  # atomically claim the highest-priority ready task
  spool show <id> --json              # full context and current lease
  spool renew <id> --token <token>    # renew before the 15-minute lease expires
  spool comment <id> 'Progress / decisions / evidence' --ref <path-or-url>
  spool complete <id> --token <token> --note 'What changed and how it was checked'
  spool release <id> --token <token> --note 'Where to resume'

Use the token returned in claim.token. A stale or expired token cannot complete
or overwrite reclaimed work. next returns null when no work is available.

Plan and hand off:
  spool stream add <project> -d 'Objective and scope'
  spool add 'Concrete task' --stream <project> -p p1 -d 'Acceptance criteria'
  spool block <task> --by <prerequisite>
  spool handoff <id> --to <agent-address> --token <token> --note 'Context and next step' --json

handoff records context and assignment, releases the claim, and returns a
notification payload. Send notification.message to notification.to with
telephone send when you want to notify that agent. Receiving a message does
not claim the task; the recipient must claim it before working. Spool never
sends messages automatically. A handoff to a human or an agent without a
Telephone address works the same way.

All worktrees of this repository share one live board in the Git common
directory. Run spool sync before committing .spool/events. It exports durable
history; claims stay local. Independent clones do not share atomic claims.

--json works on every command. Lists are compact; show includes full notes.
Use --stream, list --mine, --search, and --limit to keep context focused.
"#;

fn format_is_json(cli: &Cli) -> bool {
    cli.json
        || matches!(&cli.command,
        Commands::List { format, .. } | Commands::Stream { command: StreamCommands::List { format } }
        if format == "json")
}

pub fn main_entry() -> i32 {
    use std::io::Write;
    let args: Vec<_> = std::env::args_os().collect();
    let requested_json = args
        .iter()
        .any(|arg| arg == "--json" || arg == "--format=json" || arg == "-fjson")
        || args
            .windows(2)
            .any(|pair| (pair[0] == "--format" || pair[0] == "-f") && pair[1] == "json");
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(error) => {
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                let _ = error.print();
                return 0;
            }
            if requested_json {
                eprintln!(
                    "{}",
                    json!({"error": {"code": "invalid_input", "message": error.to_string()}})
                );
            } else {
                let _ = error.print();
            }
            return 2;
        }
    };
    let json_output = format_is_json(&cli);
    match run(cli) {
        Ok(reply) => {
            let output = if json_output {
                reply.value.to_string()
            } else {
                reply.text
            };
            match writeln!(std::io::stdout().lock(), "{output}") {
                Ok(()) => 0,
                Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => 0,
                Err(error) => {
                    eprintln!("Could not write output: {error}");
                    1
                }
            }
        }
        Err(error) => {
            let code = error
                .downcast_ref::<SpoolError>()
                .map(|error| error.code)
                .unwrap_or("error");
            if json_output {
                eprintln!(
                    "{}",
                    json!({"error": {"code": code, "message": format!("{error:#}")}})
                );
            } else {
                eprintln!("Error: {error:#}");
            }
            if matches!(
                code,
                "claim_conflict" | "stale_token" | "lease_expired" | "busy" | "assigned"
            ) {
                2
            } else {
                1
            }
        }
    }
}

fn run(cli: Cli) -> Result<Reply> {
    if let Some(directory) = &cli.directory {
        std::env::set_current_dir(directory)?;
    }
    let identity = Identity::resolve(cli.agent.as_deref())?;
    let token = cli.token.as_deref();
    if matches!(cli.command, Commands::Prime) {
        return Ok(Reply::new(
            json!({"workflow": PRIME, "identity": identity}),
            PRIME,
        ));
    }
    if matches!(cli.command, Commands::Whoami) {
        return Ok(Reply::new(
            json!(identity),
            format!("{} ({})", identity.agent, identity.source),
        ));
    }
    if matches!(cli.command, Commands::Init) {
        let root = PathBuf::from(".spool");
        if root.exists() {
            return Err(anyhow!(".spool directory already exists"));
        }
        crate::context::initialize_directory(&root)?;
        let ctx = SpoolContext::discover()?;
        return Ok(Reply::new(json!({"board": ctx.root, "checkout": ctx.checkout_root}),
            "Created .spool/\nRun 'spool prime' for the agent workflow; 'spool sync' before committing."));
    }
    let ctx = SpoolContext::discover()?;
    match cli.command {
        Commands::Init | Commands::Prime | Commands::Whoami => unreachable!(),
        Commands::Sync => {
            let report = crate::store::sync(&ctx)?;
            let text = format!(
                "Exported {} events to {}. Commit .spool/events with Git.",
                report.exported, report.checkout
            );
            Ok(Reply::new(json!(report), text))
        }
        Commands::Status => {
            let state = load_or_materialize_state(&ctx)?;
            let now = Utc::now();
            let mut ready = 0;
            let mut blocked = 0;
            let mut active = 0;
            let mut complete = 0;
            let mut mine = Vec::new();
            for task in state.tasks.values().filter(|task| task.archived.is_none()) {
                match engine::work_status(&state, task, now) {
                    "complete" => complete += 1,
                    "blocked" => blocked += 1,
                    "in_progress" => active += 1,
                    _ => {}
                }
                if engine::is_ready(&state, task, &identity.agent, now) {
                    ready += 1;
                }
                if belongs_to(task, &identity.agent) {
                    mine.push(task);
                }
            }
            engine::sort_tasks(&mut mine);
            let value = json!({"board": ctx.root, "checkout": ctx.checkout_root, "identity": identity,
                "ready": ready, "blocked": blocked, "in_progress": active, "complete": complete,
                "mine": mine.iter().map(|task| engine::view(&state, task, &identity.agent, false)).collect::<Vec<_>>()});
            Ok(Reply::new(value, format!("{}\nBoard: {}\n{ready} ready for you · {active} in progress · {blocked} blocked · {complete} complete\n{}",
                identity.agent, ctx.root.display(), table(&state, &mine))))
        }
        Commands::Add {
            title,
            description,
            priority,
            assignee,
            tag,
            stream,
        } => {
            let mut data = json!({"title": title, "tags": tag});
            optional(&mut data, "description", description);
            optional(&mut data, "priority", priority);
            optional(&mut data, "assignee", assignee);
            optional(&mut data, "stream", stream);
            mutate(
                &ctx,
                &identity,
                None,
                Operation::Create,
                &crate::id::generate_id(),
                data,
                "Created task",
            )
        }
        Commands::List {
            status,
            assignee,
            tag,
            priority,
            stream,
            stream_name,
            no_stream,
            mine,
            search,
            limit,
            format,
        } => {
            let state = load_or_materialize_state(&ctx)?;
            let selected = stream
                .or(stream_name)
                .map(|name| engine::stream(&state, &name).map(|stream| stream.id.clone()))
                .transpose()?;
            let query = search.map(|text| text.to_lowercase());
            let mut tasks: Vec<_> = state
                .tasks
                .values()
                .filter(|task| {
                    let status_matches = match status.as_str() {
                        "open" => task.status == TaskStatus::Open,
                        "complete" => task.status == TaskStatus::Complete,
                        "all" => true,
                        status => engine::work_status(&state, task, Utc::now()) == status,
                    };
                    task.archived.is_none()
                        && status_matches
                        && assignee
                            .as_deref()
                            .map(|agent| {
                                task.assignee.as_deref() == Some(agent)
                                    || task.claim.as_ref().is_some_and(|claim| {
                                        claim.agent == agent && claim.is_live(Utc::now())
                                    })
                            })
                            .unwrap_or(true)
                        && tag
                            .as_deref()
                            .map(|tag| {
                                task.tags
                                    .iter()
                                    .any(|value| value.eq_ignore_ascii_case(tag))
                            })
                            .unwrap_or(true)
                        && priority
                            .as_deref()
                            .map(|priority| task.priority.as_deref().unwrap_or("p2") == priority)
                            .unwrap_or(true)
                        && selected
                            .as_deref()
                            .map(|stream| task.stream.as_deref() == Some(stream))
                            .unwrap_or(true)
                        && (!no_stream || task.stream.is_none())
                        && (!mine || belongs_to(task, &identity.agent))
                        && query
                            .as_deref()
                            .map(|query| {
                                format!(
                                    "{} {} {}",
                                    task.title,
                                    task.description.as_deref().unwrap_or_default(),
                                    task.tags.join(" ")
                                )
                                .to_lowercase()
                                .contains(query)
                            })
                            .unwrap_or(true)
                })
                .collect();
            engine::sort_tasks(&mut tasks);
            if let Some(limit) = limit {
                tasks.truncate(limit);
            }
            let text = if format == "ids" {
                tasks
                    .iter()
                    .map(|task| task.id.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                table(&state, &tasks)
            };
            Ok(Reply::new(
                json!(tasks
                    .iter()
                    .map(|task| engine::view(&state, task, &identity.agent, false))
                    .collect::<Vec<_>>()),
                text,
            ))
        }
        Commands::Ready { stream, tag, limit } => {
            let state = load_or_materialize_state(&ctx)?;
            let selected = stream
                .map(|name| engine::stream(&state, &name).map(|stream| stream.id.clone()))
                .transpose()?;
            let mut tasks: Vec<_> = state
                .tasks
                .values()
                .filter(|task| {
                    engine::is_ready(&state, task, &identity.agent, Utc::now())
                        && selected
                            .as_deref()
                            .map(|id| task.stream.as_deref() == Some(id))
                            .unwrap_or(true)
                        && tag
                            .as_deref()
                            .map(|tag| {
                                task.tags
                                    .iter()
                                    .any(|value| value.eq_ignore_ascii_case(tag))
                            })
                            .unwrap_or(true)
                })
                .collect();
            engine::sort_tasks(&mut tasks);
            if let Some(limit) = limit {
                tasks.truncate(limit);
            }
            Ok(Reply::new(
                json!(tasks
                    .iter()
                    .map(|task| engine::view(&state, task, &identity.agent, false))
                    .collect::<Vec<_>>()),
                table(&state, &tasks),
            ))
        }
        Commands::Next { stream, tag } => claimed_reply(
            &identity,
            engine::claim(
                &ctx,
                None,
                &identity,
                cli.lease_seconds,
                ReadyFilter {
                    stream: stream.as_deref(),
                    tag: tag.as_deref(),
                },
            )?,
        ),
        Commands::Claim { id } => claimed_reply(
            &identity,
            engine::claim(
                &ctx,
                Some(&id),
                &identity,
                cli.lease_seconds,
                ReadyFilter::default(),
            )?,
        ),
        Commands::Renew { id } => {
            let (state, id) = engine::renew(&ctx, &id, &identity, token, cli.lease_seconds)?;
            Ok(task_reply(&state, &id, &identity, "Renewed claim"))
        }
        Commands::Show { id, events } => {
            let _lock = crate::concurrency::FileLock::acquire(&ctx)?;
            let history = crate::store::read_events(&ctx, true)?;
            let state = crate::state::materialize_events(history.clone())?;
            let task = engine::task(&state, &id)?;
            let mut value = engine::view(&state, task, &identity.agent, true);
            let mut text = task_text(&state, task);
            if events {
                let history: Vec<_> = history
                    .into_iter()
                    .filter(|event| event.id == task.id)
                    .collect();
                text.push_str("\nEvent History:\n");
                for event in &history {
                    text.push_str(&format!(
                        "  {} {} by {} on {}\n",
                        event.ts, event.op, event.by, event.branch
                    ));
                }
                value["events"] = json!(history);
            }
            Ok(Reply::new(value, text))
        }
        Commands::Complete {
            id,
            resolution,
            note,
            reference,
        } => {
            let mut data = json!({"resolution": resolution});
            optional(&mut data, "body", note);
            optional(&mut data, "ref", reference);
            mutate(
                &ctx,
                &identity,
                token,
                Operation::Complete,
                &id,
                data,
                "Completed task",
            )
        }
        Commands::Reopen { id } => mutate(
            &ctx,
            &identity,
            token,
            Operation::Reopen,
            &id,
            json!({}),
            "Reopened task",
        ),
        Commands::Update {
            id,
            title,
            description,
            priority,
            stream,
            add_tag,
            remove_tag,
        } => {
            let mut data = json!({});
            optional(&mut data, "title", title);
            optional(&mut data, "description", description);
            optional(&mut data, "priority", priority);
            if let Some(stream) = stream {
                data["stream"] = if stream.is_empty() {
                    Value::Null
                } else {
                    json!(stream)
                };
            }
            if !add_tag.is_empty() {
                data["add_tags"] = json!(add_tag);
            }
            if !remove_tag.is_empty() {
                data["remove_tags"] = json!(remove_tag);
            }
            mutate(
                &ctx,
                &identity,
                token,
                Operation::Update,
                &id,
                data,
                "Updated task",
            )
        }
        Commands::Assign { id, assignee } => mutate(
            &ctx,
            &identity,
            token,
            Operation::Assign,
            &id,
            json!({"to": assignee}),
            "Assigned task",
        ),
        Commands::Free { id } => mutate(
            &ctx,
            &identity,
            token,
            Operation::Assign,
            &id,
            json!({"to": null}),
            "Freed task",
        ),
        Commands::Release { id, note } => mutate(
            &ctx,
            &identity,
            token,
            Operation::Handoff,
            &id,
            json!({"to": null, "body": note}),
            "Released task",
        ),
        Commands::Handoff {
            id,
            to,
            note,
            reference,
        } => {
            let mut data = json!({"to": to, "body": note});
            optional(&mut data, "ref", reference);
            let mut reply = mutate(
                &ctx,
                &identity,
                token,
                Operation::Handoff,
                &id,
                data,
                "Handed off task",
            )?;
            let id = reply.value["id"].as_str().unwrap_or(&id);
            let directory = ctx
                .checkout_root
                .as_ref()
                .unwrap_or(&ctx.root)
                .parent()
                .unwrap_or(&ctx.root);
            let message = format!("Spool task {id}: {}\nRepository: {}\nFrom: {}\nAssigned to: {to}\n{note}\nRead spool show {id} --json, then claim the task with your own identity before working.", reply.value["title"].as_str().unwrap_or_default(), directory.display(), identity.agent);
            reply.value["notification"] = json!({"to": to, "message": message});
            reply.text.push_str(&format!(
                " to {to}\nContext saved. Telephone message (send when ready):\n{message}"
            ));
            Ok(reply)
        }
        Commands::Comment {
            id,
            body,
            reference,
        } => {
            let mut data = json!({"body": body});
            optional(&mut data, "ref", reference);
            mutate(
                &ctx,
                &identity,
                None,
                Operation::Comment,
                &id,
                data,
                "Commented on task",
            )
        }
        Commands::Block { id, by } => mutate(
            &ctx,
            &identity,
            token,
            Operation::Link,
            &id,
            json!({"rel": "blocked_by", "target": by}),
            "Blocked task",
        ),
        Commands::Unblock { id, by } => mutate(
            &ctx,
            &identity,
            token,
            Operation::Unlink,
            &id,
            json!({"rel": "blocked_by", "target": by}),
            "Unblocked task",
        ),
        Commands::Link {
            id,
            relation,
            target,
        } => mutate(
            &ctx,
            &identity,
            token,
            Operation::Link,
            &id,
            json!({"rel": relation, "target": target}),
            "Linked task",
        ),
        Commands::Unlink {
            id,
            relation,
            target,
        } => mutate(
            &ctx,
            &identity,
            token,
            Operation::Unlink,
            &id,
            json!({"rel": relation, "target": target}),
            "Unlinked task",
        ),
        Commands::Stream { command } => stream_command(&ctx, &identity, command),
        Commands::Rebuild => {
            let _lock = crate::concurrency::FileLock::acquire(&ctx)?;
            let state = crate::state::materialize(&ctx)?;
            crate::store::write_json(&ctx.state_path(), &state)?;
            crate::store::write_json(&ctx.index_path(), &crate::state::build_index(&ctx)?)?;
            Ok(Reply::new(
                json!({"tasks": state.tasks.len(), "streams": state.streams.len()}),
                "Rebuild complete. Reads always use the event log.",
            ))
        }
        Commands::Archive { days, dry_run } => {
            let ids = crate::archive::archive_tasks_quiet(&ctx, days, dry_run)?;
            let text = if dry_run {
                format!("Would archive {} tasks", ids.len())
            } else {
                format!("Archived {} tasks", ids.len())
            };
            Ok(Reply::new(json!({"tasks": ids, "dry_run": dry_run}), text))
        }
        Commands::Validate { strict } => {
            let result = crate::validation::inspect(&ctx)?;
            if !result.errors.is_empty() || (strict && !result.warnings.is_empty()) {
                return Err(SpoolError::new(
                    "invalid_history",
                    format!(
                        "Validation failed: {}",
                        result
                            .errors
                            .iter()
                            .chain(&result.warnings)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("; ")
                    ),
                )
                .into());
            }
            let text = if result.warnings.is_empty() {
                "Validation passed. No issues found.".to_string()
            } else {
                format!(
                    "Validation passed with warnings:\n{}",
                    result.warnings.join("\n")
                )
            };
            Ok(Reply::new(json!(result), text))
        }
    }
}

fn optional(data: &mut Value, key: &str, value: Option<String>) {
    if let Some(value) = value {
        data[key] = json!(value);
    }
}

fn belongs_to(task: &Task, agent: &str) -> bool {
    task.assignee.as_deref() == Some(agent)
        || task
            .claim
            .as_ref()
            .is_some_and(|claim| claim.agent == agent && claim.is_live(Utc::now()))
}

fn mutate(
    ctx: &SpoolContext,
    identity: &Identity,
    token: Option<&str>,
    operation: Operation,
    id: &str,
    data: Value,
    label: &str,
) -> Result<Reply> {
    let state = engine::commit(
        ctx,
        engine::event(operation, id, &identity.agent, data)?,
        token,
    )?;
    let id = engine::task(&state, id)?.id.clone();
    Ok(task_reply(&state, &id, identity, label))
}

fn task_reply(state: &State, id: &str, identity: &Identity, label: &str) -> Reply {
    let task = &state.tasks[id];
    let mut text = format!("{label}: {id}");
    if let Some(assignee) = &task.assignee {
        text.push_str(&format!("\nAssignee: {assignee}"));
    }
    if let Some(resolution) = &task.resolution {
        text.push_str(&format!("\nResolution: {resolution}"));
    }
    if let Some(claim) = &task.claim {
        text.push_str(&format!(
            "\nAgent: {}\nToken: {}\nLease expires: {}",
            claim.agent, claim.token, claim.expires_at
        ));
    }
    Reply::new(engine::view(state, task, &identity.agent, true), text)
}

fn claimed_reply(identity: &Identity, claimed: Option<(State, String)>) -> Result<Reply> {
    Ok(match claimed {
        Some((state, id)) => {
            let mut reply = task_reply(&state, &id, identity, "Claimed task");
            reply
                .text
                .push_str(&format!("\n{}", state.tasks[&id].title));
            if let Some(description) = &state.tasks[&id].description {
                reply.text.push_str(&format!("\n{description}"));
            }
            reply
        }
        None => Reply::new(Value::Null, "No ready tasks."),
    })
}

fn table(state: &State, tasks: &[&Task]) -> String {
    if tasks.is_empty() {
        return "No tasks found.".into();
    }
    let mut text = format!(
        "{:<20} {:<12} {:<8} {:<20} TITLE\n",
        "ID", "STATUS", "PRIORITY", "ASSIGNEE / AGENT"
    );
    for task in tasks {
        let agent = task
            .claim
            .as_ref()
            .filter(|claim| claim.is_live(Utc::now()))
            .map(|claim| claim.agent.as_str())
            .or(task.assignee.as_deref())
            .unwrap_or("-");
        text.push_str(&format!(
            "{:<20} {:<12} {:<8} {:<20} {}\n",
            task.id,
            engine::work_status(state, task, Utc::now()),
            task.priority.as_deref().unwrap_or("p2"),
            agent,
            task.title
                .chars()
                .take(80)
                .collect::<String>()
                .replace('\n', " ")
        ));
    }
    text.trim_end().to_string()
}

fn task_text(state: &State, task: &Task) -> String {
    let mut text = format!(
        "ID:       {}\nTitle:    {}\nStatus:   {}\nPriority: {}\n",
        task.id,
        task.title,
        engine::work_status(state, task, Utc::now()),
        task.priority.as_deref().unwrap_or("p2")
    );
    if let Some(stream) = &task.stream {
        text.push_str(&format!(
            "Stream:   {} ({})\n",
            state
                .streams
                .get(stream)
                .map(|stream| stream.name.as_str())
                .unwrap_or("missing"),
            stream
        ));
    }
    if let Some(assignee) = &task.assignee {
        text.push_str(&format!("Assignee: {assignee}\n"));
    }
    if let Some(claim) = &task.claim {
        text.push_str(&format!(
            "Claim:    {} until {}{}\nToken:    {}\nWorktree: {}\n",
            claim.agent,
            claim.expires_at,
            if claim.is_live(Utc::now()) {
                ""
            } else {
                " (expired)"
            },
            claim.token,
            claim.worktree
        ));
    }
    if !task.tags.is_empty() {
        text.push_str(&format!("Tags:     {}\n", task.tags.join(", ")));
    }
    if let Some(description) = &task.description {
        text.push_str(&format!(
            "Description:\n  {}\n",
            description.replace('\n', "\n  ")
        ));
    }
    text.push_str(&format!(
        "Created:  {} by {} on {}\nUpdated:  {}\n",
        task.created, task.created_by, task.created_branch, task.updated
    ));
    if let Some(completed) = task.completed {
        text.push_str(&format!(
            "Completed: {} ({})\n",
            completed,
            task.resolution.as_deref().unwrap_or("done")
        ));
    }
    if let Some(archived) = &task.archived {
        text.push_str(&format!("Archived: {archived}\n"));
    }
    if let Some(parent) = &task.parent {
        text.push_str(&format!("Parent:   {parent}\n"));
    }
    if !task.blocks.is_empty() {
        text.push_str(&format!("Blocks:   {}\n", task.blocks.join(", ")));
    }
    let dependencies = engine::dependencies(state, task);
    if !dependencies.is_empty() {
        text.push_str(&format!("Blocked by: {}\n", dependencies.join(", ")));
    }
    if !task.comments.is_empty() {
        text.push_str("\nComments:\n");
        for comment in &task.comments {
            text.push_str(&format!(
                "  [{} - {}]\n  {}\n",
                comment.ts,
                comment.by,
                comment.body.replace('\n', "\n  ")
            ));
            if let Some(reference) = &comment.r#ref {
                text.push_str(&format!("  ref: {reference}\n"));
            }
        }
    }
    text.trim_end().to_string()
}

fn stream_command(
    ctx: &SpoolContext,
    identity: &Identity,
    command: StreamCommands,
) -> Result<Reply> {
    match command {
        StreamCommands::List { format } => {
            let state = load_or_materialize_state(ctx)?;
            let mut streams: Vec<_> = state.streams.values().collect();
            streams.sort_by(|a, b| (&a.name, &a.id).cmp(&(&b.name, &b.id)));
            let mut text = if streams.is_empty() {
                "No streams found.".into()
            } else {
                format!("{:<20} {:<24} {:<8} COMPLETE\n", "ID", "NAME", "OPEN")
            };
            let values: Vec<_> = streams
                .iter()
                .map(|stream| {
                    let tasks: Vec<_> = state
                        .tasks
                        .values()
                        .filter(|task| {
                            task.stream.as_deref() == Some(&stream.id) && task.archived.is_none()
                        })
                        .collect();
                    let open = tasks
                        .iter()
                        .filter(|task| task.status == TaskStatus::Open)
                        .count();
                    let complete = tasks.len() - open;
                    text.push_str(&format!(
                        "{:<20} {:<24} {:<8} {}\n",
                        stream.id, stream.name, open, complete
                    ));
                    let mut value = json!(stream);
                    value["open"] = json!(open);
                    value["complete"] = json!(complete);
                    value
                })
                .collect();
            if format == "ids" {
                text = streams
                    .iter()
                    .map(|stream| stream.id.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            Ok(Reply::new(json!(values), text.trim_end()))
        }
        StreamCommands::Show { id, name } => {
            let state = load_or_materialize_state(ctx)?;
            let stream = engine::stream(
                &state,
                id.as_deref()
                    .or(name.as_deref())
                    .ok_or_else(|| anyhow!("Either stream ID or --name must be provided"))?,
            )?;
            let mut tasks: Vec<_> = state
                .tasks
                .values()
                .filter(|task| {
                    task.stream.as_deref() == Some(&stream.id) && task.archived.is_none()
                })
                .collect();
            engine::sort_tasks(&mut tasks);
            let open = tasks
                .iter()
                .filter(|task| task.status == TaskStatus::Open)
                .count();
            let complete = tasks.len() - open;
            let text = format!("ID:          {}\nName:        {}\nDescription: {}\n\nTasks: {open} open, {complete} complete\n{}", stream.id, stream.name, stream.description.as_deref().unwrap_or_default(), table(&state, &tasks));
            Ok(Reply::new(
                json!({"stream": stream, "open": open, "complete": complete, "tasks": tasks.iter().map(|task| engine::view(&state, task, &identity.agent, false)).collect::<Vec<_>>()}),
                text,
            ))
        }
        command => {
            let (operation, id, data, label) = match command {
                StreamCommands::Add { name, description } => {
                    let mut data = json!({"name": name});
                    optional(&mut data, "description", description);
                    (
                        Operation::CreateStream,
                        crate::id::generate_id(),
                        data,
                        "Created stream",
                    )
                }
                StreamCommands::Update {
                    id,
                    name,
                    description,
                } => {
                    let mut data = json!({});
                    optional(&mut data, "name", name);
                    optional(&mut data, "description", description);
                    (Operation::UpdateStream, id, data, "Updated stream")
                }
                StreamCommands::Delete { id } => {
                    (Operation::DeleteStream, id, json!({}), "Deleted stream")
                }
                _ => unreachable!(),
            };
            let state = engine::commit(
                ctx,
                engine::event(operation.clone(), &id, &identity.agent, data)?,
                None,
            )?;
            if operation == Operation::DeleteStream {
                return Ok(Reply::new(
                    json!({"id": id, "deleted": true}),
                    format!("{label}: {id}"),
                ));
            }
            let stream = engine::stream(&state, &id)?;
            Ok(Reply::new(
                json!(stream),
                format!("{label}: {} ({})", stream.name, stream.id),
            ))
        }
    }
}
