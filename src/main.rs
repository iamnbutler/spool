use anyhow::Result;
use clap::Parser;

use fabric::archive::archive_tasks;
use fabric::cli::{complete_task, list_tasks, reopen_task, show_task, update_task, Cli, Commands, OutputFormat};
use fabric::context::{init, FabricContext};
use fabric::shell::run_shell;
use fabric::state::rebuild;
use fabric::validation::validate;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => init(),
        Commands::List {
            status,
            assignee,
            tag,
            priority,
            format,
        } => {
            let ctx = FabricContext::discover()?;
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
            let ctx = FabricContext::discover()?;
            show_task(&ctx, &id, events)
        }
        Commands::Rebuild => {
            let ctx = FabricContext::discover()?;
            rebuild(&ctx)
        }
        Commands::Archive { days, dry_run } => {
            let ctx = FabricContext::discover()?;
            archive_tasks(&ctx, days, dry_run)?;
            Ok(())
        }
        Commands::Validate { strict } => {
            let ctx = FabricContext::discover()?;
            validate(&ctx, strict)?;
            Ok(())
        }
        Commands::Shell => {
            let ctx = FabricContext::discover()?;
            run_shell(ctx)
        }
        Commands::Complete { id, resolution } => {
            let ctx = FabricContext::discover()?;
            complete_task(&ctx, &id, Some(&resolution))
        }
        Commands::Reopen { id } => {
            let ctx = FabricContext::discover()?;
            reopen_task(&ctx, &id)
        }
        Commands::Update {
            id,
            title,
            description,
            priority,
        } => {
            let ctx = FabricContext::discover()?;
            update_task(
                &ctx,
                &id,
                title.as_deref(),
                description.as_deref(),
                priority.as_deref(),
            )
        }
    }
}
