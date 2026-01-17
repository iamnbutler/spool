//! Interactive shell mode for spool commands

use anyhow::{anyhow, Result};
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{Config, Editor, Helper};
use std::borrow::Cow;

use crate::cli::{complete_task, list_tasks, reopen_task, show_task, update_task, OutputFormat};
use crate::context::SpoolContext;
use crate::state::load_or_materialize_state;
use crate::writer::{create_task, get_current_branch, get_current_user};

const COMMANDS: &[&str] = &[
    "add", "list", "show", "update", "complete", "reopen", "help", "quit", "exit",
];

const HELP_TEXT: &str = r#"
spool shell - Interactive mode

Commands:
  add <title> [-d <description>] [-p <priority>] [-a <assignee>] [-t <tag>...]
      Create a new task

  list [--status <open|complete|all>] [--assignee <name>] [--tag <tag>] [--priority <p>]
      List tasks with optional filters

  show <task-id> [--events]
      Show details of a specific task

  update <task-id> [-t <title>] [-d <description>] [-p <priority>]
      Update a task's fields

  complete <task-id> [-r <resolution>]
      Mark a task as complete (resolution: done, wontfix, duplicate, obsolete)

  reopen <task-id>
      Reopen a completed task

  help
      Show this help message

  quit, exit
      Exit the shell

Tab completion is available for commands and task IDs.
Use Up/Down arrows to navigate command history.
"#;

struct SpoolCompleter {
    ctx: SpoolContext,
}

impl SpoolCompleter {
    fn new(ctx: SpoolContext) -> Self {
        Self { ctx }
    }

    fn get_task_ids(&self) -> Vec<String> {
        load_or_materialize_state(&self.ctx)
            .map(|state| state.tasks.keys().cloned().collect())
            .unwrap_or_default()
    }
}

impl Completer for SpoolCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let line_to_cursor = &line[..pos];
        let words: Vec<&str> = line_to_cursor.split_whitespace().collect();

        // If we're at the start or completing the first word, complete commands
        if words.is_empty() || (words.len() == 1 && !line_to_cursor.ends_with(' ')) {
            let prefix = words.first().copied().unwrap_or("");
            let candidates: Vec<Pair> = COMMANDS
                .iter()
                .filter(|cmd| cmd.starts_with(prefix))
                .map(|cmd| Pair {
                    display: cmd.to_string(),
                    replacement: cmd.to_string(),
                })
                .collect();
            let start = line_to_cursor.rfind(' ').map(|i| i + 1).unwrap_or(0);
            return Ok((start, candidates));
        }

        // If we're completing after 'show', 'update', 'complete', or 'reopen', complete task IDs
        let cmd = words.first().copied();
        if (cmd == Some("show")
            || cmd == Some("update")
            || cmd == Some("complete")
            || cmd == Some("reopen"))
            && words.len() <= 2
        {
            let prefix = if line_to_cursor.ends_with(' ') {
                ""
            } else {
                words.get(1).copied().unwrap_or("")
            };

            let task_ids = self.get_task_ids();
            let candidates: Vec<Pair> = task_ids
                .iter()
                .filter(|id| id.starts_with(prefix))
                .map(|id| Pair {
                    display: id.clone(),
                    replacement: id.clone(),
                })
                .collect();

            let start = if line_to_cursor.ends_with(' ') {
                pos
            } else {
                line_to_cursor.rfind(' ').map(|i| i + 1).unwrap_or(0)
            };
            return Ok((start, candidates));
        }

        // Complete flags for list command
        if words.first() == Some(&"list") {
            let last_word = if line_to_cursor.ends_with(' ') {
                ""
            } else {
                words.last().copied().unwrap_or("")
            };

            if last_word.starts_with('-') || last_word.is_empty() {
                let flags = ["--status", "--assignee", "--tag", "--priority", "--format"];
                let candidates: Vec<Pair> = flags
                    .iter()
                    .filter(|f| f.starts_with(last_word))
                    .map(|f| Pair {
                        display: f.to_string(),
                        replacement: f.to_string(),
                    })
                    .collect();

                let start = if line_to_cursor.ends_with(' ') {
                    pos
                } else {
                    line_to_cursor.rfind(' ').map(|i| i + 1).unwrap_or(0)
                };
                return Ok((start, candidates));
            }

            // Complete status values
            let prev_word = if words.len() >= 2 {
                words.get(words.len() - 2).copied()
            } else {
                None
            };

            if prev_word == Some("--status") || prev_word == Some("-s") {
                let statuses = ["open", "complete", "all"];
                let candidates: Vec<Pair> = statuses
                    .iter()
                    .filter(|s| s.starts_with(last_word))
                    .map(|s| Pair {
                        display: s.to_string(),
                        replacement: s.to_string(),
                    })
                    .collect();

                let start = if line_to_cursor.ends_with(' ') {
                    pos
                } else {
                    line_to_cursor.rfind(' ').map(|i| i + 1).unwrap_or(0)
                };
                return Ok((start, candidates));
            }
        }

        Ok((pos, vec![]))
    }
}

impl Hinter for SpoolCompleter {
    type Hint = String;

    fn hint(&self, _line: &str, _pos: usize, _ctx: &rustyline::Context<'_>) -> Option<String> {
        None
    }
}

impl Highlighter for SpoolCompleter {
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Borrowed(hint)
    }
}

impl Validator for SpoolCompleter {}

impl Helper for SpoolCompleter {}

/// Split a command line respecting quoted strings
fn shell_split(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';

    for c in line.chars() {
        if in_quotes {
            if c == quote_char {
                // End of quoted section
                in_quotes = false;
            } else {
                current.push(c);
            }
        } else if c == '"' || c == '\'' {
            // Start of quoted section
            in_quotes = true;
            quote_char = c;
        } else if c == ' ' || c == '\t' {
            // Whitespace outside quotes - token boundary
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else {
            current.push(c);
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

#[allow(clippy::type_complexity)]
fn parse_add_args(
    args: &[&str],
) -> Result<(
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<String>,
)> {
    if args.is_empty() {
        return Err(anyhow!(
            "Usage: add <title> [-d description] [-p priority] [-a assignee] [-t tag...]"
        ));
    }

    let mut title_parts = Vec::new();
    let mut description = None;
    let mut priority = None;
    let mut assignee = None;
    let mut tags = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-d" | "--description" => {
                i += 1;
                if i < args.len() {
                    description = Some(args[i].to_string());
                }
            }
            "-p" | "--priority" => {
                i += 1;
                if i < args.len() {
                    priority = Some(args[i].to_string());
                }
            }
            "-a" | "--assignee" => {
                i += 1;
                if i < args.len() {
                    assignee = Some(args[i].to_string());
                }
            }
            "-t" | "--tag" => {
                i += 1;
                if i < args.len() {
                    tags.push(args[i].to_string());
                }
            }
            _ => {
                if !args[i].starts_with('-') {
                    title_parts.push(args[i]);
                }
            }
        }
        i += 1;
    }

    if title_parts.is_empty() {
        return Err(anyhow!("Task title is required"));
    }

    let title = title_parts.join(" ");
    Ok((title, description, priority, assignee, tags))
}

fn parse_list_args(
    args: &[&str],
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    OutputFormat,
) {
    let mut status = Some("open".to_string());
    let mut assignee = None;
    let mut tag = None;
    let mut priority = None;
    let mut format = OutputFormat::Table;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-s" | "--status" => {
                i += 1;
                if i < args.len() {
                    status = Some(args[i].to_string());
                }
            }
            "-a" | "--assignee" => {
                i += 1;
                if i < args.len() {
                    assignee = Some(args[i].to_string());
                }
            }
            "-t" | "--tag" => {
                i += 1;
                if i < args.len() {
                    tag = Some(args[i].to_string());
                }
            }
            "-p" | "--priority" => {
                i += 1;
                if i < args.len() {
                    priority = Some(args[i].to_string());
                }
            }
            "-f" | "--format" => {
                i += 1;
                if i < args.len() {
                    format = OutputFormat::from_str(args[i]);
                }
            }
            _ => {}
        }
        i += 1;
    }

    (status, assignee, tag, priority, format)
}

fn parse_show_args(args: &[&str]) -> Result<(String, bool)> {
    if args.is_empty() {
        return Err(anyhow!("Usage: show <task-id> [--events]"));
    }

    let id = args[0].to_string();
    let events = args.contains(&"--events") || args.contains(&"-e");

    Ok((id, events))
}

fn parse_resolution_arg(args: &[&str]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        if (args[i] == "-r" || args[i] == "--resolution") && i + 1 < args.len() {
            return Some(args[i + 1].to_string());
        }
        i += 1;
    }
    None
}

fn parse_update_args(args: &[&str]) -> (Option<String>, Option<String>, Option<String>) {
    let mut title = None;
    let mut description = None;
    let mut priority = None;

    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "-t" | "--title" => {
                i += 1;
                if i < args.len() {
                    title = Some(args[i].to_string());
                }
            }
            "-d" | "--description" => {
                i += 1;
                if i < args.len() {
                    description = Some(args[i].to_string());
                }
            }
            "-p" | "--priority" => {
                i += 1;
                if i < args.len() {
                    priority = Some(args[i].to_string());
                }
            }
            _ => {}
        }
        i += 1;
    }

    (title, description, priority)
}

fn execute_command(ctx: &SpoolContext, line: &str) -> Result<bool> {
    let parts = shell_split(line);
    if parts.is_empty() {
        return Ok(true); // Continue
    }

    let cmd = parts[0].as_str();
    let args: Vec<&str> = parts[1..].iter().map(|s| s.as_str()).collect();
    let args = args.as_slice();

    match cmd {
        "help" | "?" => {
            println!("{}", HELP_TEXT);
        }
        "quit" | "exit" => {
            return Ok(false); // Stop
        }
        "add" => {
            let (title, description, priority, assignee, tags) = parse_add_args(args)?;
            let user = get_current_user()?;
            let branch = get_current_branch()?;

            let id = create_task(
                ctx,
                &title,
                description.as_deref(),
                priority.as_deref(),
                assignee.as_deref(),
                tags,
                &user,
                &branch,
            )?;
            println!("Created task: {}", id);
        }
        "list" | "ls" => {
            let (status, assignee, tag, priority, format) = parse_list_args(args);
            list_tasks(
                ctx,
                status.as_deref(),
                assignee.as_deref(),
                tag.as_deref(),
                priority.as_deref(),
                format,
            )?;
        }
        "show" => {
            let (id, events) = parse_show_args(args)?;
            show_task(ctx, &id, events)?;
        }
        "update" | "edit" => {
            if args.is_empty() {
                return Err(anyhow!(
                    "Usage: update <task-id> [-t title] [-d description] [-p priority]"
                ));
            }
            let id = args[0];
            let (title, description, priority) = parse_update_args(&args[1..]);
            update_task(
                ctx,
                id,
                title.as_deref(),
                description.as_deref(),
                priority.as_deref(),
            )?;
        }
        "complete" | "done" | "close" => {
            if args.is_empty() {
                return Err(anyhow!(
                    "Usage: complete <task-id> [-r done|wontfix|duplicate|obsolete]"
                ));
            }
            let id = args[0];
            let resolution = parse_resolution_arg(args);
            complete_task(ctx, id, resolution.as_deref())?;
        }
        "reopen" => {
            if args.is_empty() {
                return Err(anyhow!("Usage: reopen <task-id>"));
            }
            let id = args[0];
            reopen_task(ctx, id)?;
        }
        _ => {
            println!(
                "Unknown command: {}. Type 'help' for available commands.",
                cmd
            );
        }
    }

    Ok(true)
}

/// Run the interactive shell
pub fn run_shell(ctx: SpoolContext) -> Result<()> {
    let config = Config::builder()
        .history_ignore_space(true)
        .history_ignore_dups(true)?
        .build();

    let completer = SpoolCompleter::new(SpoolContext::new(ctx.root.clone()));
    let mut rl: Editor<SpoolCompleter, DefaultHistory> = Editor::with_config(config)?;
    rl.set_helper(Some(completer));

    // Load history
    let history_path = dirs::data_local_dir()
        .map(|p| p.join("spool").join("shell_history"))
        .unwrap_or_else(|| ctx.root.join(".shell_history"));

    if let Some(parent) = history_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let _ = rl.load_history(&history_path);

    println!("spool shell v0.1.0");
    println!("Type 'help' for available commands, 'quit' to exit.\n");

    loop {
        let readline = rl.readline("spool> ");

        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(line);

                match execute_command(&ctx, line) {
                    Ok(true) => continue,
                    Ok(false) => break,
                    Err(e) => println!("Error: {}", e),
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("exit");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    // Save history
    let _ = rl.save_history(&history_path);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_split_simple() {
        let result = shell_split("add Task title -p 1");
        assert_eq!(result, vec!["add", "Task", "title", "-p", "1"]);
    }

    #[test]
    fn test_shell_split_double_quotes() {
        let result = shell_split(r#"add Task -d "This is a description""#);
        assert_eq!(result, vec!["add", "Task", "-d", "This is a description"]);
    }

    #[test]
    fn test_shell_split_single_quotes() {
        let result = shell_split("add Task -d 'Single quoted description'");
        assert_eq!(
            result,
            vec!["add", "Task", "-d", "Single quoted description"]
        );
    }

    #[test]
    fn test_shell_split_mixed() {
        let result = shell_split(r#"add "Quoted title" -p 1 -d "Quoted description""#);
        assert_eq!(
            result,
            vec!["add", "Quoted title", "-p", "1", "-d", "Quoted description"]
        );
    }

    #[test]
    fn test_shell_split_empty() {
        let result = shell_split("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_shell_split_only_whitespace() {
        let result = shell_split("   \t  ");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_add_args_basic() {
        let args: &[&str] = &["Task", "title", "-p", "1"];
        let (title, desc, priority, assignee, tags) = parse_add_args(args).unwrap();
        assert_eq!(title, "Task title");
        assert_eq!(priority, Some("1".to_string()));
        assert!(desc.is_none());
        assert!(assignee.is_none());
        assert!(tags.is_empty());
    }

    #[test]
    fn test_parse_add_args_with_description() {
        let args: &[&str] = &["Task", "-d", "A description", "-p", "2"];
        let (title, desc, priority, _, _) = parse_add_args(args).unwrap();
        assert_eq!(title, "Task");
        assert_eq!(desc, Some("A description".to_string()));
        assert_eq!(priority, Some("2".to_string()));
    }
}
