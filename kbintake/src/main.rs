use std::process::ExitCode;

use anyhow::Error;
use clap::Parser;
use kbintake::cli::{Cli, Commands, JobCommands, TargetCommands};
use kbintake::{agent, app, cli, exit_codes, logging};

fn main() -> ExitCode {
    if let Err(err) = logging::init_logging() {
        return exit_with_error(exit_codes::GENERAL_ERROR, err);
    }

    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Agent => app::App::bootstrap()
            .and_then(|app| agent::run_agent(&app))
            .map(|()| exit_codes::SUCCESS)
            .map_err(|err| (CommandKind::Agent, err)),
        Commands::Import {
            target,
            process,
            dry_run,
            json,
            paths,
        } => app::App::bootstrap()
            .and_then(|app| cli::handle_import_command(&app, target, process, dry_run, json, paths))
            .map_err(|err| (CommandKind::Import, err)),
        Commands::Jobs { command } => {
            let kind = CommandKind::Jobs(command_kind(&command));
            app::App::bootstrap()
                .and_then(|app| cli::handle_jobs(&app, command))
                .map(|()| exit_codes::SUCCESS)
                .map_err(|err| (kind, err))
        }
        Commands::Targets { command } => {
            let kind = CommandKind::Targets(target_kind(&command));
            app::App::bootstrap()
                .and_then(|app| cli::handle_targets(&app, command))
                .map(|()| exit_codes::SUCCESS)
                .map_err(|err| (kind, err))
        }
        Commands::Config { command } => app::App::bootstrap()
            .and_then(|app| cli::handle_config(&app, command))
            .map(|()| exit_codes::SUCCESS)
            .map_err(|err| (CommandKind::Config, err)),
        Commands::Vault { command } => app::App::bootstrap()
            .and_then(|app| cli::handle_vault(&app, command))
            .map(|()| exit_codes::SUCCESS)
            .map_err(|err| (CommandKind::Vault, err)),
        Commands::Explorer { command } => cli::handle_explorer(command)
            .map(|()| exit_codes::SUCCESS)
            .map_err(|err| (CommandKind::Explorer, err)),
        Commands::Doctor => app::App::bootstrap()
            .and_then(|app| cli::handle_doctor(&app))
            .map(|()| exit_codes::SUCCESS)
            .map_err(|err| (CommandKind::Doctor, err)),
        Commands::ConfigShow => app::App::bootstrap()
            .and_then(|app| cli::handle_config_show(&app))
            .map(|()| exit_codes::SUCCESS)
            .map_err(|err| (CommandKind::Config, err)),
    };

    match result {
        Ok(code) => ExitCode::from(code as u8),
        Err((kind, err)) => exit_with_error(classify_error(kind, &err), err),
    }
}

#[derive(Debug, Clone, Copy)]
enum CommandKind {
    Agent,
    Import,
    Jobs(JobKind),
    Targets(TargetKind),
    Vault,
    Config,
    Explorer,
    Doctor,
}

#[derive(Debug, Clone, Copy)]
enum JobKind {
    List,
    Show,
    Retry,
}

#[derive(Debug, Clone, Copy)]
enum TargetKind {
    List,
    Show,
    Add,
    Rename,
    Remove,
    SetDefault,
}

fn command_kind(command: &JobCommands) -> JobKind {
    match command {
        JobCommands::List => JobKind::List,
        JobCommands::Show { .. } => JobKind::Show,
        JobCommands::Retry { .. } => JobKind::Retry,
    }
}

fn target_kind(command: &TargetCommands) -> TargetKind {
    match command {
        TargetCommands::List { .. } => TargetKind::List,
        TargetCommands::Show { .. } => TargetKind::Show,
        TargetCommands::Add { .. } => TargetKind::Add,
        TargetCommands::Rename { .. } => TargetKind::Rename,
        TargetCommands::Remove { .. } => TargetKind::Remove,
        TargetCommands::SetDefault { .. } => TargetKind::SetDefault,
    }
}

fn exit_with_error(code: i32, err: Error) -> ExitCode {
    eprintln!("ERROR [{code}]: {err:#}");
    ExitCode::from(code as u8)
}

fn classify_error(kind: CommandKind, err: &Error) -> i32 {
    if matches!(kind, CommandKind::Doctor | CommandKind::Explorer) {
        return exit_codes::GENERAL_ERROR;
    }

    if is_missing_row(err) {
        return match kind {
            CommandKind::Jobs(JobKind::Show | JobKind::Retry) => exit_codes::INVALID_ARGUMENTS,
            CommandKind::Targets(
                TargetKind::Show | TargetKind::Rename | TargetKind::Remove | TargetKind::SetDefault,
            ) => exit_codes::TARGET_NOT_FOUND,
            _ => exit_codes::DATABASE_ERROR,
        };
    }

    if has_database_error(err) {
        return exit_codes::DATABASE_ERROR;
    }

    let message = err.to_string();
    let lower_message = message.to_ascii_lowercase();
    if message.contains("target not configured") {
        return exit_codes::TARGET_NOT_FOUND;
    }
    if lower_message.contains("cannot remove") || lower_message.contains("archived") {
        return exit_codes::OPERATION_REJECTED;
    }
    if matches!(kind, CommandKind::Config) {
        return exit_codes::INVALID_ARGUMENTS;
    }
    if message.contains("no input paths provided")
        || message.contains("no importable files found")
        || message.contains("failed to scan path")
        || message.contains("target already configured")
        || message.contains("target name")
        || message.contains("not a directory")
    {
        return exit_codes::INVALID_ARGUMENTS;
    }

    exit_codes::GENERAL_ERROR
}

fn has_database_error(err: &Error) -> bool {
    err.chain()
        .any(|cause| cause.downcast_ref::<rusqlite::Error>().is_some())
}

fn is_missing_row(err: &Error) -> bool {
    err.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<rusqlite::Error>(),
            Some(rusqlite::Error::QueryReturnedNoRows)
        )
    })
}
