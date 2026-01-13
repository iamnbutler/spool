use anyhow::Result;
use clap::Parser;

use fabric::archive::archive_tasks;
use fabric::cli::{list_tasks, show_task, Cli, Commands, OutputFormat};
use fabric::context::{init, FabricContext};
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
    }
}
