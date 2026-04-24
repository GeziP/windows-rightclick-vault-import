use std::path::Path;

use anyhow::{Context, Result};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, EnvFilter};

pub fn init_logging() -> Result<Option<WorkerGuard>> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
    Ok(None)
}

pub fn init_service_logging(log_dir: &Path) -> Result<WorkerGuard> {
    std::fs::create_dir_all(log_dir)
        .with_context(|| format!("failed to create log directory {}", log_dir.display()))?;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let appender = tracing_appender::rolling::daily(log_dir, "service.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(writer)
        .init();
    Ok(guard)
}
