use std::process::ExitCode;

use anyhow::Error;
use clap::Parser;
use kbintake::cli::{Cli, Commands, JobCommands, ServiceCommands, TargetCommands};
use kbintake::{agent, app, cli, exit_codes, logging};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let _log_guard = match &cli.command {
        Commands::Service {
            command: ServiceCommands::Run,
        } => {
            let log_dir = cli
                .app_data_dir
                .clone()
                .unwrap_or_else(kbintake::config::default_app_data_dir)
                .join("logs");
            match logging::init_service_logging(&log_dir) {
                Ok(guard) => Some(guard),
                Err(err) => return exit_with_error(exit_codes::GENERAL_ERROR, err),
            }
        }
        _ => match logging::init_logging() {
            Ok(guard) => guard,
            Err(err) => return exit_with_error(exit_codes::GENERAL_ERROR, err),
        },
    };

    let app_data_dir = cli.app_data_dir.clone();
    let result = match cli.command {
        Commands::Agent => app::App::bootstrap_at(app_data_dir)
            .and_then(|app| agent::run_agent(&app))
            .map(|()| exit_codes::SUCCESS)
            .map_err(|err| (CommandKind::Agent, err)),
        Commands::Watch { path } => app::App::bootstrap_at(app_data_dir)
            .and_then(|app| {
                let paths = if path.is_empty() { None } else { Some(path) };
                cli::handle_watch(&app, paths)
            })
            .map_err(|err| (CommandKind::Watch, err)),
        Commands::Import {
            target,
            template,
            tags,
            process,
            dry_run,
            json,
            open,
            clipboard,
            paths,
        } => app::App::bootstrap_at(app_data_dir)
            .and_then(|app| {
                cli::handle_import_command(
                    &app, target, template, tags, process, dry_run, json, open, clipboard, paths,
                )
            })
            .map_err(|err| (CommandKind::Import, err)),
        Commands::Jobs { command } => {
            let kind = CommandKind::Jobs(command_kind(&command));
            app::App::bootstrap_at(app_data_dir)
                .and_then(|app| cli::handle_jobs(&app, command))
                .map_err(|err| (kind, err))
        }
        Commands::Targets { command } => {
            let kind = CommandKind::Targets(target_kind(&command));
            app::App::bootstrap_at(app_data_dir)
                .and_then(|app| cli::handle_targets(&app, command))
                .map(|()| exit_codes::SUCCESS)
                .map_err(|err| (kind, err))
        }
        Commands::Config { command } => app::App::bootstrap_at(app_data_dir)
            .and_then(|app| cli::handle_config(&app, command))
            .map(|()| exit_codes::SUCCESS)
            .map_err(|err| (CommandKind::Config, err)),
        Commands::Vault { command } => app::App::bootstrap_at(app_data_dir)
            .and_then(|app| cli::handle_vault(&app, command))
            .map(|()| exit_codes::SUCCESS)
            .map_err(|err| (CommandKind::Vault, err)),
        Commands::Explorer { command } => {
            let lang = app_data_dir
                .as_ref()
                .and_then(|dir| {
                    app::App::bootstrap_at(Some(dir.clone()))
                        .ok()
                        .map(|app| app.config.language().to_string())
                })
                .unwrap_or_else(|| "en".to_string());
            cli::handle_explorer(command, &lang)
                .map(|()| exit_codes::SUCCESS)
                .map_err(|err| (CommandKind::Explorer, err))
        }
        Commands::Service { command } => {
            handle_service_command(command, app_data_dir).map_err(|err| (CommandKind::Service, err))
        }
        Commands::Doctor { fix, migrate } => app::App::bootstrap_at(app_data_dir)
            .and_then(|app| cli::handle_doctor(&app, fix, migrate))
            .map_err(|err| (CommandKind::Doctor, err)),
        Commands::ConfigShow => app::App::bootstrap_at(app_data_dir)
            .and_then(|app| cli::handle_config_show(&app))
            .map(|()| exit_codes::SUCCESS)
            .map_err(|err| (CommandKind::Config, err)),
        Commands::Tui => app::App::bootstrap_at(app_data_dir)
            .and_then(|app| kbintake::tui::run_settings_tui(app.config))
            .map(|()| exit_codes::SUCCESS)
            .map_err(|err| (CommandKind::Config, err)),
        Commands::Obsidian { command } => {
            let kind = CommandKind::Config;
            cli::handle_obsidian(command)
                .map(|()| exit_codes::SUCCESS)
                .map_err(|err| (kind, err))
        }
        Commands::Version => {
            println!("kbintake {}", env!("CARGO_PKG_VERSION"));
            Ok(exit_codes::SUCCESS)
        }
    };

    match result {
        Ok(code) => ExitCode::from(code as u8),
        Err((kind, err)) => exit_with_error(classify_error(kind, &err), err),
    }
}

#[derive(Debug, Clone, Copy)]
enum CommandKind {
    Agent,
    Watch,
    Import,
    Jobs(JobKind),
    Targets(TargetKind),
    Vault,
    Config,
    Explorer,
    Service,
    Doctor,
}

#[derive(Debug, Clone, Copy)]
enum JobKind {
    List,
    Show,
    Retry,
    Undo,
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
        JobCommands::List { .. } => JobKind::List,
        JobCommands::Show { .. } => JobKind::Show,
        JobCommands::Retry { .. } => JobKind::Retry,
        JobCommands::Undo { .. } => JobKind::Undo,
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

fn handle_service_command(
    command: ServiceCommands,
    app_data_dir: Option<std::path::PathBuf>,
) -> anyhow::Result<i32> {
    match command {
        ServiceCommands::Install => {
            let app = app::App::bootstrap_at(app_data_dir)?;
            kbintake::service::install(&app.config.app_data_dir)?;
            Ok(exit_codes::SUCCESS)
        }
        ServiceCommands::Start => {
            kbintake::service::start()?;
            Ok(exit_codes::SUCCESS)
        }
        ServiceCommands::Stop => {
            kbintake::service::stop()?;
            Ok(exit_codes::SUCCESS)
        }
        ServiceCommands::Uninstall => {
            kbintake::service::uninstall()?;
            Ok(exit_codes::SUCCESS)
        }
        ServiceCommands::Status => {
            println!("Service status: {}", kbintake::service::status()?);
            Ok(exit_codes::SUCCESS)
        }
        ServiceCommands::Run => {
            let app_data_dir = app_data_dir.unwrap_or_else(kbintake::config::default_app_data_dir);
            kbintake::service::run_dispatcher(app_data_dir)?;
            Ok(exit_codes::SUCCESS)
        }
    }
}

fn classify_error(kind: CommandKind, err: &Error) -> i32 {
    if matches!(
        kind,
        CommandKind::Doctor | CommandKind::Explorer | CommandKind::Service
    ) {
        return exit_codes::GENERAL_ERROR;
    }

    if is_missing_row(err) {
        return match kind {
            CommandKind::Jobs(JobKind::Show | JobKind::Retry | JobKind::Undo) => {
                exit_codes::INVALID_ARGUMENTS
            }
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
    if lower_message.contains("cannot remove")
        || lower_message.contains("cannot undo batch")
        || lower_message.contains("archived")
    {
        return exit_codes::OPERATION_REJECTED;
    }
    if matches!(kind, CommandKind::Config) {
        return exit_codes::INVALID_ARGUMENTS;
    }
    if message.contains("no input paths provided")
        || message.contains("no importable files found")
        || message.contains("failed to scan path")
        || message.contains("unsupported status filter")
        || message.contains("--json and --table cannot be used together")
        || message.contains("target already configured")
        || message.contains("target name")
        || message.contains("not a directory")
        || message.contains("clipboard")
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
