use std::path::{Path, PathBuf};

pub const SERVICE_NAME: &str = "KBIntake";
pub const SERVICE_DISPLAY_NAME: &str = "KBIntake";

#[cfg(windows)]
mod imp {
    use std::ffi::OsString;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result};
    use windows_service::service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
        ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::service_dispatcher;
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    use crate::agent::scheduler;
    use crate::agent::watcher;
    use crate::app::App;

    use super::{Path, PathBuf, SERVICE_DISPLAY_NAME, SERVICE_NAME};

    const SERVICE_MISSING_ERROR: i32 = 1060;
    const ACCESS_DENIED_ERROR: i32 = 5;

    windows_service::define_windows_service!(ffi_service_main, service_main);

    pub fn install(app_data_dir: &Path) -> Result<()> {
        let service_manager = with_admin_hint(
            ServiceManager::local_computer(
                None::<&str>,
                ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
            ),
            "install the Windows service",
        )?;
        let executable_path =
            std::env::current_exe().context("failed to resolve current executable path")?;
        let service_info = ServiceInfo {
            name: OsString::from(SERVICE_NAME),
            display_name: OsString::from(SERVICE_DISPLAY_NAME),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path,
            launch_arguments: vec![
                OsString::from("--app-data-dir"),
                app_data_dir.as_os_str().to_os_string(),
                OsString::from("service"),
                OsString::from("run"),
            ],
            dependencies: vec![],
            account_name: None,
            account_password: None,
        };

        let service = with_admin_hint(
            service_manager.create_service(
                &service_info,
                ServiceAccess::QUERY_STATUS | ServiceAccess::CHANGE_CONFIG,
            ),
            "create the Windows service",
        )?;
        service.set_description(
            "Background KBIntake queue worker that processes queued imports continuously.",
        )?;
        println!(
            "Installed service '{}' for app data {}",
            SERVICE_NAME,
            app_data_dir.display()
        );
        Ok(())
    }

    pub fn start() -> Result<()> {
        let service_manager = with_admin_hint(
            ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT),
            "connect to the Windows Service Control Manager",
        )?;
        let service = with_admin_hint(
            service_manager.open_service(
                SERVICE_NAME,
                ServiceAccess::START | ServiceAccess::QUERY_STATUS,
            ),
            "start the Windows service",
        )?;
        with_admin_hint(
            service.start::<std::ffi::OsString>(&[]),
            "start the Windows service",
        )?;
        wait_for_state(&service, ServiceState::Running, Duration::from_secs(15))?;
        println!("Service '{}' started.", SERVICE_NAME);
        Ok(())
    }

    pub fn stop() -> Result<()> {
        let service_manager = with_admin_hint(
            ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT),
            "connect to the Windows Service Control Manager",
        )?;
        let service = with_admin_hint(
            service_manager.open_service(
                SERVICE_NAME,
                ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
            ),
            "stop the Windows service",
        )?;
        if service.query_status()?.current_state == ServiceState::Stopped {
            println!("Service '{}' is already stopped.", SERVICE_NAME);
            return Ok(());
        }
        with_admin_hint(service.stop(), "stop the Windows service")?;
        wait_for_state(&service, ServiceState::Stopped, Duration::from_secs(30))?;
        println!("Service '{}' stopped.", SERVICE_NAME);
        Ok(())
    }

    pub fn uninstall() -> Result<()> {
        let service_manager = with_admin_hint(
            ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT),
            "connect to the Windows Service Control Manager",
        )?;
        let service = with_admin_hint(
            service_manager.open_service(
                SERVICE_NAME,
                ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
            ),
            "remove the Windows service",
        )?;

        with_admin_hint(service.delete(), "remove the Windows service")?;
        if service.query_status()?.current_state != ServiceState::Stopped {
            with_admin_hint(service.stop(), "stop the Windows service")?;
            wait_for_state(&service, ServiceState::Stopped, Duration::from_secs(30))?;
        }
        println!("Service '{}' removed.", SERVICE_NAME);
        Ok(())
    }

    pub fn status() -> Result<String> {
        let service_manager =
            ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        match service_manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS) {
            Ok(service) => {
                let status = service.query_status()?;
                Ok(match status.current_state {
                    ServiceState::Running => "running",
                    ServiceState::Stopped => "stopped",
                    ServiceState::StartPending => "start_pending",
                    ServiceState::StopPending => "stop_pending",
                    ServiceState::Paused => "paused",
                    ServiceState::PausePending => "pause_pending",
                    ServiceState::ContinuePending => "continue_pending",
                }
                .to_string())
            }
            Err(err) if is_service_missing_error(&err) => Ok("not installed".to_string()),
            Err(err) => Err(err.into()),
        }
    }

    pub fn run_dispatcher(app_data_dir: PathBuf) -> Result<()> {
        SERVICE_APP_DATA_DIR
            .set(app_data_dir)
            .map_err(|_| anyhow::anyhow!("service app data directory already initialized"))?;
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
        Ok(())
    }

    static SERVICE_APP_DATA_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

    fn service_main(_arguments: Vec<OsString>) {
        if let Err(err) = run_service() {
            tracing::error!(error = %err, "service loop failed");
        }
    }

    fn run_service() -> Result<()> {
        let shutdown_flag = Arc::new(AtomicBool::new(false));

        let event_handler = {
            let flag = Arc::clone(&shutdown_flag);
            move |control_event| -> ServiceControlHandlerResult {
                match control_event {
                    ServiceControl::Stop => {
                        flag.store(true, Ordering::SeqCst);
                        ServiceControlHandlerResult::NoError
                    }
                    ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                    _ => ServiceControlHandlerResult::NotImplemented,
                }
            }
        };

        let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;
        status_handle.set_service_status(service_status(
            ServiceState::Running,
            ServiceControlAccept::STOP,
            ServiceExitCode::Win32(0),
            0,
            Duration::default(),
        ))?;

        let app_data_dir = SERVICE_APP_DATA_DIR
            .get()
            .cloned()
            .context("service app data directory not initialized")?;
        let app = App::bootstrap_in(app_data_dir.clone())?;

        let poll_interval = Duration::from_secs(app.config.agent.poll_interval_secs.max(1));

        // Spawn watcher thread if configured.
        let watcher_handle = if app.config.agent.watch_in_service && !app.config.watch.is_empty() {
            let flag = Arc::clone(&shutdown_flag);
            let dir = app_data_dir.clone();
            Some(thread::spawn(move || -> Result<()> {
                let watcher_app = App::bootstrap_in(dir)?;
                match watcher::run_watcher(&watcher_app, None, flag) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        // Watcher errors are non-fatal to the service.
                        tracing::warn!(error = %e, "watcher thread exited with error");
                        Ok(())
                    }
                }
            }))
        } else {
            None
        };

        // Queue processor loop.
        let sleep_chunk = Duration::from_millis(200);
        loop {
            if scheduler::process_next_item(&app)? {
                if shutdown_flag.load(Ordering::SeqCst) {
                    break;
                }
                continue;
            }

            // Poll-interval sleep with shutdown check.
            let mut elapsed = Duration::ZERO;
            while elapsed < poll_interval {
                if shutdown_flag.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(sleep_chunk.min(poll_interval - elapsed));
                elapsed += sleep_chunk;
            }
            if shutdown_flag.load(Ordering::SeqCst) {
                break;
            }
        }

        // Wait for watcher thread to finish.
        if let Some(handle) = watcher_handle {
            let _ = handle.join();
        }

        status_handle.set_service_status(service_status(
            ServiceState::Stopped,
            ServiceControlAccept::empty(),
            ServiceExitCode::Win32(0),
            0,
            Duration::default(),
        ))?;
        Ok(())
    }

    fn wait_for_state(
        service: &windows_service::service::Service,
        desired_state: ServiceState,
        timeout: Duration,
    ) -> Result<()> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if service.query_status()?.current_state == desired_state {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(250));
        }
        anyhow::bail!(
            "service '{}' did not reach state {:?} within {:?}",
            SERVICE_NAME,
            desired_state,
            timeout
        );
    }

    fn is_service_missing_error(err: &windows_service::Error) -> bool {
        matches!(
            err,
            windows_service::Error::Winapi(source)
                if source.raw_os_error() == Some(SERVICE_MISSING_ERROR)
        )
    }

    fn is_access_denied_error(err: &windows_service::Error) -> bool {
        matches!(
            err,
            windows_service::Error::Winapi(source)
                if source.raw_os_error() == Some(ACCESS_DENIED_ERROR)
        )
    }

    fn with_admin_hint<T>(result: windows_service::Result<T>, action: &str) -> Result<T> {
        result.map_err(|err| {
            if is_access_denied_error(&err) {
                anyhow::anyhow!(
                    "failed to {action}: access denied. Re-run the command from an elevated Administrator shell."
                )
            } else {
                err.into()
            }
        })
    }

    fn service_status(
        current_state: ServiceState,
        controls_accepted: ServiceControlAccept,
        exit_code: ServiceExitCode,
        checkpoint: u32,
        wait_hint: Duration,
    ) -> ServiceStatus {
        ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state,
            controls_accepted,
            exit_code,
            checkpoint,
            wait_hint,
            process_id: None,
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use anyhow::Result;

    use super::{Path, PathBuf, SERVICE_NAME};

    pub fn install(_app_data_dir: &Path) -> Result<()> {
        anyhow::bail!("service install is only supported on Windows")
    }

    pub fn start() -> Result<()> {
        anyhow::bail!("service start is only supported on Windows")
    }

    pub fn stop() -> Result<()> {
        anyhow::bail!("service stop is only supported on Windows")
    }

    pub fn uninstall() -> Result<()> {
        anyhow::bail!("service uninstall is only supported on Windows")
    }

    pub fn status() -> Result<String> {
        Ok(format!("{SERVICE_NAME} is not available on this platform"))
    }

    pub fn run_dispatcher(_app_data_dir: PathBuf) -> Result<()> {
        anyhow::bail!("service run is only supported on Windows")
    }
}

pub use imp::{install, run_dispatcher, start, status, stop, uninstall};
