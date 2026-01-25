use anyhow::Result;
use clap::Parser;

use spool::archive::archive_tasks;
use spool::cli::{
    add_stream, add_task, assign_task, claim_task, complete_task, delete_stream, free_task,
    list_streams, list_tasks, reopen_task, show_stream, show_task, update_stream_cmd, update_task,
    Cli, Commands, OutputFormat, StreamCommands,
};
use spool::context::{init, SpoolContext};
use spool::state::rebuild;
use spool::validation::validate;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => init(),
        Commands::Add {
            title,
            description,
            priority,
            assignee,
            tag,
            stream,
        } => {
            let ctx = SpoolContext::discover()?;
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
            stream_name,
            no_stream,
            format,
        } => {
            let ctx = SpoolContext::discover()?;
            let fmt = OutputFormat::from_str(&format);
            list_tasks(
                &ctx,
                Some(&status),
                assignee.as_deref(),
                tag.as_deref(),
                priority.as_deref(),
                stream.as_deref(),
                stream_name.as_deref(),
                no_stream,
                fmt,
            )
        }
        Commands::Show { id, events } => {
            let ctx = SpoolContext::discover()?;
            show_task(&ctx, &id, events)
        }
        Commands::Rebuild => {
            let ctx = SpoolContext::discover()?;
            rebuild(&ctx)
        }
        Commands::Archive { days, dry_run } => {
            let ctx = SpoolContext::discover()?;
            archive_tasks(&ctx, days, dry_run)?;
            Ok(())
        }
        Commands::Validate { strict } => {
            let ctx = SpoolContext::discover()?;
            validate(&ctx, strict)?;
            Ok(())
        }
        Commands::Complete { id, resolution } => {
            let ctx = SpoolContext::discover()?;
            complete_task(&ctx, &id, Some(&resolution))
        }
        Commands::Reopen { id } => {
            let ctx = SpoolContext::discover()?;
            reopen_task(&ctx, &id)
        }
        Commands::Update {
            id,
            title,
            description,
            priority,
            stream,
        } => {
            let ctx = SpoolContext::discover()?;
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
            let ctx = SpoolContext::discover()?;
            assign_task(&ctx, &id, &assignee)
        }
        Commands::Claim { id } => {
            let ctx = SpoolContext::discover()?;
            claim_task(&ctx, &id)
        }
        Commands::Free { id } => {
            let ctx = SpoolContext::discover()?;
            free_task(&ctx, &id)
        }
        Commands::Stream { command } => {
            let ctx = SpoolContext::discover()?;
            match command {
                StreamCommands::Add { name, description } => {
                    add_stream(&ctx, &name, description.as_deref())
                }
                StreamCommands::List { format } => {
                    let fmt = OutputFormat::from_str(&format);
                    list_streams(&ctx, fmt)
                }
                StreamCommands::Show { id, name } => {
                    show_stream(&ctx, id.as_deref(), name.as_deref())
                }
                StreamCommands::Update {
                    id,
                    name,
                    description,
                } => update_stream_cmd(&ctx, &id, name.as_deref(), description.as_deref()),
                StreamCommands::Delete { id } => delete_stream(&ctx, &id),
            }
        }
    }
}
