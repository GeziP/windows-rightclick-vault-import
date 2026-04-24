use std::path::{Path, PathBuf};

pub const SERVICE_NAME: &str = "KBIntake";
pub const SERVICE_DISPLAY_NAME: &str = "KBIntake";

#[cfg(windows)]
mod imp {
    use std::ffi::OsString;
    use std::sync::mpsc::{self, RecvTimeoutError};
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
    use crate::app::App;

    use super::{Path, PathBuf, SERVICE_DISPLAY_NAME, SERVICE_NAME};

    const SERVICE_MISSING_ERROR: i32 = 1060;

    windows_service::define_windows_service!(ffi_service_main, service_main);

    pub fn install(app_data_dir: &Path) -> Result<()> {
        let service_manager = ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
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

        let service = service_manager.create_service(
            &service_info,
            ServiceAccess::QUERY_STATUS | ServiceAccess::CHANGE_CONFIG,
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
        let service_manager =
            ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = service_manager.open_service(
            SERVICE_NAME,
            ServiceAccess::START | ServiceAccess::QUERY_STATUS,
        )?;
        service.start::<std::ffi::OsString>(&[])?;
        wait_for_state(&service, ServiceState::Running, Duration::from_secs(15))?;
        println!("Service '{}' started.", SERVICE_NAME);
        Ok(())
    }

    pub fn stop() -> Result<()> {
        let service_manager =
            ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = service_manager.open_service(
            SERVICE_NAME,
            ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
        )?;
        if service.query_status()?.current_state == ServiceState::Stopped {
            println!("Service '{}' is already stopped.", SERVICE_NAME);
            return Ok(());
        }
        service.stop()?;
        wait_for_state(&service, ServiceState::Stopped, Duration::from_secs(30))?;
        println!("Service '{}' stopped.", SERVICE_NAME);
        Ok(())
    }

    pub fn uninstall() -> Result<()> {
        let service_manager =
            ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = service_manager.open_service(
            SERVICE_NAME,
            ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
        )?;

        service.delete()?;
        if service.query_status()?.current_state != ServiceState::Stopped {
            service.stop()?;
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
        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop => {
                    if let Some(tx) = STOP_TX.get() {
                        let _ = tx.send(());
                    }
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
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
        let app = App::bootstrap_in(app_data_dir)?;

        let (stop_tx, stop_rx) = mpsc::channel();
        STOP_TX
            .set(stop_tx)
            .map_err(|_| anyhow::anyhow!("service stop channel already initialized"))?;

        let poll_interval = Duration::from_secs(app.config.agent.poll_interval_secs.max(1));
        loop {
            if scheduler::process_next_item(&app)? {
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                continue;
            }

            match stop_rx.recv_timeout(poll_interval) {
                Ok(()) => break,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
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

    static STOP_TX: std::sync::OnceLock<mpsc::Sender<()>> = std::sync::OnceLock::new();

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
