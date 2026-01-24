use anyhow::Result;
use clap::Parser;

use spool::archive::archive_tasks;
use spool::cli::{
    add_task, assign_task, claim_task, complete_task, free_task, list_tasks, reopen_task,
    show_task, stream_add, stream_list, stream_remove, stream_show, update_task, Cli, Commands,
    OutputFormat, StreamCommands,
};
use spool::context::{init, SpoolContext};
use spool::migration::check_and_migrate;
use spool::shell::run_shell;
use spool::state::rebuild;
use spool::validation::validate;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => init(),
        _ => {
            // For all non-init commands, run migration check first
            let ctx = SpoolContext::discover()?;
            check_and_migrate(&ctx)?;
            run_command(cli.command, ctx)
        }
    }
}

fn run_command(command: Commands, ctx: SpoolContext) -> Result<()> {
    match command {
        Commands::Init => unreachable!("Init handled above"),
        Commands::Add {
            title,
            description,
            priority,
            assignee,
            tag,
            stream,
        } => {
            add_task(
                &ctx,
                &title,
                description.as_deref(),
                priority.as_deref(),
                assignee.as_deref(),
                tag,
                stream.as_deref(),
            )
        }
        Commands::List {
            status,
            assignee,
            tag,
            priority,
            stream,
            format,
        } => {
            let fmt = OutputFormat::from_str(&format);
            list_tasks(
                &ctx,
                Some(&status),
                assignee.as_deref(),
                tag.as_deref(),
                priority.as_deref(),
                stream.as_deref(),
                fmt,
            )
        }
        Commands::Show { id, events } => {
            show_task(&ctx, &id, events)
        }
        Commands::Rebuild => {
            rebuild(&ctx)
        }
        Commands::Archive { days, dry_run } => {
            archive_tasks(&ctx, days, dry_run)?;
            Ok(())
        }
        Commands::Validate { strict } => {
            validate(&ctx, strict)?;
            Ok(())
        }
        Commands::Shell => {
            run_shell(ctx)
        }
        Commands::Complete { id, resolution } => {
            complete_task(&ctx, &id, Some(&resolution))
        }
        Commands::Reopen { id } => {
            reopen_task(&ctx, &id)
        }
        Commands::Update {
            id,
            title,
            description,
            priority,
            stream,
        } => {
            update_task(
                &ctx,
                &id,
                title.as_deref(),
                description.as_deref(),
                priority.as_deref(),
                stream.as_deref(),
            )
        }
        Commands::Assign { id, assignee } => {
            assign_task(&ctx, &id, &assignee)
        }
        Commands::Claim { id } => {
            claim_task(&ctx, &id)
        }
        Commands::Free { id } => {
            free_task(&ctx, &id)
        }
        Commands::Stream { command } => match command {
            StreamCommands::List { format } => {
                let fmt = OutputFormat::from_str(&format);
                stream_list(&ctx, fmt)
            }
            StreamCommands::Show { name, format } => {
                let fmt = OutputFormat::from_str(&format);
                stream_show(&ctx, &name, fmt)
            }
            StreamCommands::Add { id, name } => {
                stream_add(&ctx, &id, &name)
            }
            StreamCommands::Remove { id } => {
                stream_remove(&ctx, &id)
            }
        },
    }
}
