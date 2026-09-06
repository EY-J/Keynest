mod diagnostics;
mod ipc;
mod platform;
mod security;
mod settings;
#[allow(dead_code)]
pub(crate) mod vault;

use std::{sync::Arc, time::Duration};

use ipc::DataFolderService;
use platform::startup::{
    minimize_for_launch, StartupService, TauriMainWindowMinimizer, TauriStartupRegistration,
};
use security::{
    AuthService, AutoLockService, ClipboardService, KdfParams, LockCoordinator, OsEntropy,
    ProfileStore, SecurityOperationGate, TauriClipboardPort, TauriLockEventSink,
};
use settings::{SettingsService, SettingsStore};
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;
use vault::VaultService;

fn resume_lock(
    auto_lock: &AutoLockService,
    settings: &SettingsService,
    operation_gate: &SecurityOperationGate,
) {
    let guard = operation_gate.lock();
    if settings.snapshot(false).lock_on_sleep {
        let _ = auto_lock.lock_now_with_operation_guard(&guard);
    }
}

fn shutdown_auto_lock(auto_lock: &AutoLockService) {
    auto_lock.shutdown();
}

fn configure_capture_protection(config: &mut tauri::Config) {
    // Screenshot protection is intentionally disabled in debug builds to allow UI development and visual regression testing. Release builds must keep capture protection enabled.
    if cfg!(debug_assertions) {
        for window in &mut config.app.windows {
            window.content_protected = false;
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    diagnostics::install_panic_hook();
    let mut context = tauri::generate_context!();
    configure_capture_protection(context.config_mut());
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .setup(|app| {
            minimize_for_launch(std::env::args_os(), &TauriMainWindowMinimizer::new(app))?;

            let app_data_dir = app.path().app_data_dir()?;
            let operation_gate = SecurityOperationGate::new();
            app.manage(operation_gate.clone());
            app.manage(DataFolderService::new(app_data_dir.clone()));
            app.manage(StartupService::new(Arc::new(
                TauriStartupRegistration::new(app.handle().clone()),
            )));

            let settings_store = SettingsStore::new(app_data_dir.clone());
            let settings = SettingsService::load(settings_store)?;
            let snapshot = settings.snapshot(false);
            app.manage(settings);

            let kdf_params = KdfParams::production();
            let store = ProfileStore::new(app_data_dir.clone());
            let auth = AuthService::load(store, kdf_params, Arc::new(OsEntropy));
            app.manage(auth.clone());
            app.manage(VaultService::new(app_data_dir, Arc::new(OsEntropy)));

            let clipboard = ClipboardService::new(
                Arc::new(TauriClipboardPort::new(app.handle().clone())),
                Duration::from_secs(snapshot.clipboard_clear_seconds),
            );
            app.manage(clipboard.clone());

            let coordinator = LockCoordinator::new(
                auth,
                clipboard,
                Arc::new(TauriLockEventSink::new(app.handle().clone())),
                operation_gate,
            );
            app.manage(coordinator.clone());
            app.manage(AutoLockService::new(
                Arc::new(coordinator),
                Duration::from_secs(snapshot.auto_lock_seconds),
            ));
            #[cfg(windows)]
            {
                let auto_lock = app.state::<AutoLockService>().inner().clone();
                let settings = app.state::<SettingsService>().inner().clone();
                let gate = app.state::<SecurityOperationGate>().inner().clone();
                app.manage(platform::session::SessionMonitor::start(move |trigger| {
                    let guard = gate.lock();
                    if trigger == platform::session::EnvironmentLock::Session
                        || settings.snapshot(false).lock_on_sleep
                    {
                        let _ = auto_lock.lock_now_with_operation_guard(&guard);
                    }
                })?);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::get_auth_status,
            ipc::create_master_password,
            ipc::complete_recovery_key_display,
            ipc::get_recovery_status,
            ipc::recover_master_password,
            ipc::regenerate_recovery_key,
            ipc::copy_recovery_key,
            ipc::unlock,
            ipc::lock,
            ipc::get_settings,
            ipc::set_auto_lock_seconds,
            ipc::set_clipboard_clear_seconds,
            ipc::set_lock_on_sleep,
            ipc::set_theme,
            ipc::set_launch_at_startup,
            ipc::record_activity,
            ipc::change_master_password,
            ipc::reset_keynest,
            ipc::reset_keynest_authenticated,
            ipc::open_keynest_data_folder,
            ipc::list_vault_records,
            ipc::create_vault_record,
            ipc::get_vault_record,
            ipc::get_vault_record_summary,
            ipc::update_vault_record,
            ipc::delete_vault_record,
            ipc::copy_vault_password
        ])
        .build(context)
        .expect("error while building tauri application")
        .run(|app, event| match event {
            tauri::RunEvent::Resumed => {
                resume_lock(
                    app.state::<AutoLockService>().inner(),
                    app.state::<SettingsService>().inner(),
                    app.state::<SecurityOperationGate>().inner(),
                );
            }
            tauri::RunEvent::ExitRequested { api, code, .. } => {
                shutdown_auto_lock(app.state::<AutoLockService>().inner());
                let clipboard = app.state::<ClipboardService>();
                if clipboard.begin_process_exit_cleanup() {
                    api.prevent_exit();
                    let app = app.clone();
                    let finish = Arc::new(move || app.exit(code.unwrap_or(0)));
                    let _ =
                        clipboard.start_process_exit_cleanup(Duration::from_millis(250), finish);
                }
            }
            tauri::RunEvent::Exit => {
                shutdown_auto_lock(app.state::<AutoLockService>().inner());
            }
            _ => {}
        });
}

#[cfg(test)]
mod lifecycle_tests {
    use std::{
        sync::{Arc, Condvar, Mutex},
        thread,
        time::Duration,
    };

    use super::{resume_lock, AutoLockService};
    use crate::security::{AuthStatus, LockActions, LockError, SecurityOperationGate};
    use crate::settings::{SettingsService, SettingsStore};

    struct BlockingResumeActions {
        started: (Mutex<bool>, Condvar),
        released: (Mutex<bool>, Condvar),
    }

    impl BlockingResumeActions {
        fn new() -> Self {
            Self {
                started: (Mutex::new(false), Condvar::new()),
                released: (Mutex::new(false), Condvar::new()),
            }
        }

        fn wait_until_started(&self) {
            let started = self.started.0.lock().unwrap();
            let (started, timeout) = self
                .started
                .1
                .wait_timeout_while(started, Duration::from_secs(1), |started| !*started)
                .unwrap();
            assert!(*started && !timeout.timed_out());
        }

        fn release(&self) {
            *self.released.0.lock().unwrap() = true;
            self.released.1.notify_all();
        }
    }

    impl LockActions for BlockingResumeActions {
        fn status(&self) -> AuthStatus {
            AuthStatus::Unlocked
        }

        fn lock(&self) -> Result<AuthStatus, LockError> {
            *self.started.0.lock().unwrap() = true;
            self.started.1.notify_all();
            let released = self.released.0.lock().unwrap();
            drop(
                self.released
                    .1
                    .wait_while(released, |released| !*released)
                    .unwrap(),
            );
            Ok(AuthStatus::Locked)
        }
    }

    #[test]
    fn resume_handler_does_not_return_before_lock_action_finishes() {
        let actions = Arc::new(BlockingResumeActions::new());
        let auto_lock = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
        let resumed_service = auto_lock.clone();
        let temp = tempfile::tempdir().unwrap();
        let settings =
            SettingsService::load(SettingsStore::new(temp.path().to_path_buf())).unwrap();
        let operation_gate = SecurityOperationGate::new();
        let (returned_tx, returned_rx) = std::sync::mpsc::channel();
        let resumed = thread::spawn(move || {
            resume_lock(&resumed_service, &settings, &operation_gate);
            returned_tx.send(()).unwrap();
        });
        actions.wait_until_started();
        assert!(returned_rx.try_recv().is_err());
        actions.release();
        returned_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        resumed.join().unwrap();
    }

    #[test]
    fn resume_handler_skips_lock_when_sleep_lock_is_disabled() {
        let actions = Arc::new(BlockingResumeActions::new());
        actions.release();
        let auto_lock = AutoLockService::new_for_test(actions.clone(), Duration::from_secs(300));
        let temp = tempfile::tempdir().unwrap();
        let settings =
            SettingsService::load(SettingsStore::new(temp.path().to_path_buf())).unwrap();
        settings.set_lock_on_sleep(false).unwrap();

        resume_lock(&auto_lock, &settings, &SecurityOperationGate::new());

        assert!(!*actions.started.0.lock().unwrap());
    }
}
