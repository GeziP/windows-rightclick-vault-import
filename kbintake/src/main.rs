use anyhow::Result;
use clap::Parser;
use kbintake::cli::{Cli, Commands};
use kbintake::{agent, app, cli, logging};

fn main() -> Result<()> {
    logging::init_logging()?;

    let cli = Cli::parse();
    let app = app::App::bootstrap()?;

    match cli.command {
        Commands::Agent => agent::run_agent(&app)?,
        Commands::Import {
            target,
            process,
            paths,
        } => cli::handle_import_command(&app, target, process, paths)?,
        Commands::Jobs { command } => cli::handle_jobs(&app, command)?,
        Commands::Targets { command } => cli::handle_targets(&app, command)?,
        Commands::Config { command } => cli::handle_config(&app, command)?,
        Commands::Doctor => cli::handle_doctor(&app)?,
        Commands::ConfigShow => cli::handle_config_show(&app)?,
    }

    Ok(())
}
