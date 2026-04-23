use anyhow::Result;
use clap::Parser;
use kbintake::cli::{Cli, Commands};
use kbintake::{agent, app, cli, logging};

fn main() -> Result<()> {
    logging::init_logging()?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Agent => agent::run_agent(&app::App::bootstrap()?)?,
        Commands::Import {
            target,
            process,
            paths,
        } => cli::handle_import_command(&app::App::bootstrap()?, target, process, paths)?,
        Commands::Jobs { command } => cli::handle_jobs(&app::App::bootstrap()?, command)?,
        Commands::Targets { command } => cli::handle_targets(&app::App::bootstrap()?, command)?,
        Commands::Config { command } => cli::handle_config(&app::App::bootstrap()?, command)?,
        Commands::Explorer { command } => cli::handle_explorer(command)?,
        Commands::Doctor => cli::handle_doctor(&app::App::bootstrap()?)?,
        Commands::ConfigShow => cli::handle_config_show(&app::App::bootstrap()?)?,
    }

    Ok(())
}
