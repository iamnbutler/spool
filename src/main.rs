use anyhow::Result;
use clap::Parser;

use spool::archive::archive_tasks;
use spool::cli::{
    add_task, assign_task, claim_task, complete_task, free_task, list_tasks, reopen_task,
    show_task, update_task, Cli, Commands, OutputFormat,
};
use spool::context::{init, SpoolContext};
use spool::shell::run_shell;
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
        } => {
            let ctx = SpoolContext::discover()?;
            add_task(
                &ctx,
                &title,
                description.as_deref(),
                priority.as_deref(),
                assignee.as_deref(),
                tag,
            )
        }
        Commands::List {
            status,
            assignee,
            tag,
            priority,
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
        Commands::Shell => {
            let ctx = SpoolContext::discover()?;
            run_shell(ctx)
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
        } => {
            let ctx = SpoolContext::discover()?;
            update_task(
                &ctx,
                &id,
                title.as_deref(),
                description.as_deref(),
                priority.as_deref(),
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
    }
}
