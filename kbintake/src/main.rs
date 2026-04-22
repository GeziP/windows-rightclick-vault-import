mod adapter;
mod agent;
mod app;
mod cli;
mod config;
mod db;
mod domain;
mod logging;
mod processor;
mod queue;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

fn main() -> Result<()> {
    logging::init_logging()?;

    let cli = Cli::parse();
    let app = app::App::bootstrap()?;

    match cli.command {
        Commands::Agent => agent::run_agent(&app)?,
        Commands::Import { paths } => cli::handle_import(&app, paths)?,
        Commands::Jobs { command } => cli::handle_jobs(&app, command)?,
        Commands::Doctor => cli::handle_doctor(&app)?,
        Commands::ConfigShow => cli::handle_config_show(&app)?,
    }

    Ok(())
}
