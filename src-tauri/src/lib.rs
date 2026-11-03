mod bridge;
mod commands;
mod platform;

use std::sync::Arc;

use droplink_core::{Engine, EngineConfig};
use parking_lot::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

use bridge::TauriSink;

/// Paths handed to DropLink from outside (Explorer *Send to*, command line,
/// a second launch) waiting for the UI to pick them up.
pub struct PendingPaths(pub Mutex<Vec<String>>);

const OPEN_PATHS_EVENT: &str = "droplink://open-paths";

fn init_logging(app: &AppHandle) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    let filter = EnvFilter::try_from_env("DROPLINK_LOG").unwrap_or_else(|_| EnvFilter::new("info,mdns_sd=warn"));
    let log_dir = app.path().app_log_dir().ok()?;
    std::fs::create_dir_all(&log_dir).ok()?;
    let appender = tracing_appender::rolling::daily(&log_dir, "droplink.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(writer).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stderr))
        .try_init();
    Some(guard)
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn deliver_paths(app: &AppHandle, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    if let Some(p) = app.try_state::<PendingPaths>() {
        p.0.lock().extend(paths.iter().cloned());
    }
    let _ = app.emit(OPEN_PATHS_EVENT, paths);
    show_main(app);
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open DropLink", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit DropLink", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("DropLink")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "quit" => {
                if let Some(engine) = app.try_state::<Engine>() {
                    engine.shutdown();
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            deliver_paths(app, platform::paths_from_args(args));
            show_main(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .manage(PendingPaths(Mutex::new(platform::paths_from_args(std::env::args()))))
        .setup(|app| {
            let handle = app.handle().clone();
            let guard = init_logging(&handle);
            app.manage(Mutex::new(guard));

            let data_dir = handle.path().app_data_dir()?;
            let sink = Arc::new(TauriSink::new(handle.clone()));
            let cfg = EngineConfig::new(data_dir, env!("CARGO_PKG_VERSION"));
            let engine = tauri::async_runtime::block_on(Engine::start(cfg, sink.clone()))
                .map_err(|e| format!("DropLink engine failed to start: {e}"))?;
            sink.attach(engine.clone());
            app.manage(engine);

            build_tray(&handle)?;
            if std::env::args().any(|a| a == "--minimized") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.minimize();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let keep = window
                    .app_handle()
                    .try_state::<Engine>()
                    .map(|e| e.settings().close_to_tray)
                    .unwrap_or(false);
                if keep {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::list_devices,
            commands::local_device,
            commands::connect_manual,
            commands::probe_device,
            commands::rename_device,
            commands::forget_device,
            commands::pair_device,
            commands::respond_pairing,
            commands::revoke_device,
            commands::block_device,
            commands::unblock_device,
            commands::set_device_permissions,
            commands::list_groups,
            commands::create_group,
            commands::update_group,
            commands::delete_group,
            commands::send_files,
            commands::send_to_group,
            commands::inspect_paths,
            commands::list_deliveries,
            commands::pause_delivery,
            commands::resume_delivery,
            commands::cancel_delivery,
            commands::clear_finished_deliveries,
            commands::list_incoming_requests,
            commands::respond_incoming,
            commands::send_text,
            commands::list_texts,
            commands::delete_text,
            commands::clear_texts,
            commands::save_text_file,
            commands::clipboard_history,
            commands::send_clipboard,
            commands::send_clip_entry,
            commands::copy_text,
            commands::copy_clip,
            commands::delete_clip,
            commands::clear_clipboard_history,
            commands::history,
            commands::clear_history,
            commands::stats,
            commands::security_events,
            commands::update_settings,
            commands::network_interfaces,
            commands::cache_info,
            commands::clear_partial_cache,
            commands::reveal_path,
            commands::open_path,
            commands::set_send_to,
            commands::take_pending_paths,
            commands::quit_app,
        ])
        .build(tauri::generate_context!())
        .expect("error while building DropLink");

    app.run(|app, event| match event {
        RunEvent::ExitRequested { .. } | RunEvent::Exit => {
            if let Some(engine) = app.try_state::<Engine>() {
                engine.shutdown();
            }
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => show_main(app),
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        RunEvent::Opened { urls } => {
            let paths = urls.into_iter().filter_map(|u| u.to_file_path().ok()).map(|p| p.to_string_lossy().into_owned()).collect();
            deliver_paths(app, paths);
        }
        _ => {}
    });
}
