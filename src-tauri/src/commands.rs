//! Typed commands exposed to the frontend. Each one is a thin wrapper over
//! the engine; argument and result types are mirrored in `src/lib/types.ts`.

use std::path::PathBuf;

use droplink_core::models::*;
use droplink_core::text::SendResult;
use droplink_core::{Engine, Error, Settings};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;

use crate::platform;
use crate::PendingPaths;

type CmdResult<T> = std::result::Result<T, Error>;

fn other(e: impl std::fmt::Display) -> Error {
    Error::Other(e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    local: LocalDeviceView,
    settings: Settings,
    devices: Vec<DeviceView>,
    groups: Vec<GroupView>,
    deliveries: Vec<DeliveryView>,
    incoming_requests: Vec<IncomingRequestView>,
    platform: &'static str,
    clipboard_available: bool,
    autostart_enabled: bool,
    send_to_installed: bool,
    pending_paths: Vec<String>,
}

#[tauri::command]
pub fn bootstrap(app: AppHandle, engine: State<'_, Engine>, pending: State<'_, PendingPaths>) -> CmdResult<Bootstrap> {
    Ok(Bootstrap {
        local: engine.local_device(),
        settings: engine.settings(),
        devices: engine.list_devices()?,
        groups: engine.list_groups()?,
        deliveries: engine.list_deliveries(200)?,
        incoming_requests: engine.incoming_requests(),
        platform: droplink_core::util::os_name(),
        clipboard_available: engine.clipboard_available(),
        autostart_enabled: app.autolaunch().is_enabled().unwrap_or(false),
        send_to_installed: platform::send_to_installed(),
        pending_paths: std::mem::take(&mut *pending.0.lock()),
    })
}

// ---- devices ------------------------------------------------------------

#[tauri::command]
pub fn list_devices(engine: State<'_, Engine>) -> CmdResult<Vec<DeviceView>> {
    engine.list_devices()
}

#[tauri::command]
pub fn local_device(engine: State<'_, Engine>) -> LocalDeviceView {
    engine.local_device()
}

#[tauri::command]
pub async fn connect_manual(engine: State<'_, Engine>, address: String) -> CmdResult<DeviceView> {
    engine.connect_manual(&address).await
}

#[tauri::command]
pub async fn probe_device(engine: State<'_, Engine>, id: String) -> CmdResult<bool> {
    engine.probe(&id).await
}

#[tauri::command]
pub fn rename_device(engine: State<'_, Engine>, id: String, alias: Option<String>) -> CmdResult<()> {
    engine.rename_device(&id, alias)
}

#[tauri::command]
pub fn forget_device(engine: State<'_, Engine>, id: String) -> CmdResult<()> {
    engine.forget_device(&id)
}

#[tauri::command]
pub fn pair_device(engine: State<'_, Engine>, id: String) -> CmdResult<String> {
    engine.pair(&id)
}

#[tauri::command]
pub fn respond_pairing(engine: State<'_, Engine>, pairing_id: String, accept: bool) -> CmdResult<()> {
    engine.respond_pairing(&pairing_id, accept)
}

#[tauri::command]
pub fn revoke_device(engine: State<'_, Engine>, id: String) -> CmdResult<()> {
    engine.revoke_device(&id)
}

#[tauri::command]
pub fn block_device(engine: State<'_, Engine>, id: String) -> CmdResult<()> {
    engine.block_device(&id)
}

#[tauri::command]
pub fn unblock_device(engine: State<'_, Engine>, id: String) -> CmdResult<()> {
    engine.unblock_device(&id)
}

#[tauri::command]
pub fn set_device_permissions(
    engine: State<'_, Engine>,
    id: String,
    auto_accept: Option<bool>,
    clipboard_sync: Option<bool>,
) -> CmdResult<()> {
    engine.set_device_permissions(&id, auto_accept, clipboard_sync)
}

// ---- groups -------------------------------------------------------------

#[tauri::command]
pub fn list_groups(engine: State<'_, Engine>) -> CmdResult<Vec<GroupView>> {
    engine.list_groups()
}

#[tauri::command]
pub fn create_group(engine: State<'_, Engine>, name: String, color: String, members: Vec<String>) -> CmdResult<GroupView> {
    engine.create_group(&name, &color, &members)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupPatch {
    name: Option<String>,
    color: Option<String>,
    auto_accept: Option<bool>,
    clipboard_sync: Option<bool>,
    members: Option<Vec<String>>,
}

#[tauri::command]
pub fn update_group(engine: State<'_, Engine>, id: String, patch: GroupPatch) -> CmdResult<()> {
    engine.update_group(&id, patch.name, patch.color, patch.auto_accept, patch.clipboard_sync, patch.members)
}

#[tauri::command]
pub fn delete_group(engine: State<'_, Engine>, id: String) -> CmdResult<()> {
    engine.delete_group(&id)
}

// ---- transfers ----------------------------------------------------------

#[tauri::command]
pub async fn send_files(engine: State<'_, Engine>, paths: Vec<String>, recipients: Vec<String>) -> CmdResult<String> {
    engine.send_files(paths.into_iter().map(PathBuf::from).collect(), recipients).await
}

#[tauri::command]
pub async fn send_to_group(engine: State<'_, Engine>, paths: Vec<String>, group_id: String) -> CmdResult<String> {
    engine.send_to_group(paths.into_iter().map(PathBuf::from).collect(), &group_id).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathInfo {
    path: String,
    name: String,
    is_dir: bool,
    size: u64,
    file_count: u64,
}

/// Describes dropped/selected paths for the send screen (folders are summed).
#[tauri::command]
pub async fn inspect_paths(paths: Vec<String>) -> CmdResult<Vec<PathInfo>> {
    tokio::task::spawn_blocking(move || {
        paths
            .into_iter()
            .filter_map(|p| {
                let path = PathBuf::from(&p);
                let meta = std::fs::metadata(&path).ok()?;
                let name = path.file_name()?.to_string_lossy().into_owned();
                if meta.is_dir() {
                    let c = droplink_core::transfer::files::collect(std::slice::from_ref(&path)).ok()?;
                    let files: Vec<_> = c.items.iter().filter(|i| !i.is_dir).collect();
                    Some(PathInfo {
                        path: p,
                        name,
                        is_dir: true,
                        size: files.iter().map(|i| i.size).sum(),
                        file_count: files.len() as u64,
                    })
                } else {
                    Some(PathInfo { path: p, name, is_dir: false, size: meta.len(), file_count: 1 })
                }
            })
            .collect()
    })
    .await
    .map_err(other)
}

#[tauri::command]
pub fn list_deliveries(engine: State<'_, Engine>, limit: Option<u32>) -> CmdResult<Vec<DeliveryView>> {
    engine.list_deliveries(limit.unwrap_or(200))
}

#[tauri::command]
pub fn pause_delivery(engine: State<'_, Engine>, id: String) -> CmdResult<()> {
    engine.pause_delivery(&id)
}

#[tauri::command]
pub async fn resume_delivery(engine: State<'_, Engine>, id: String) -> CmdResult<()> {
    engine.resume_delivery(&id).await
}

#[tauri::command]
pub async fn cancel_delivery(engine: State<'_, Engine>, id: String) -> CmdResult<()> {
    engine.cancel_delivery(&id).await
}

#[tauri::command]
pub fn clear_finished_deliveries(engine: State<'_, Engine>) -> CmdResult<usize> {
    engine.clear_finished_deliveries()
}

#[tauri::command]
pub fn list_incoming_requests(engine: State<'_, Engine>) -> Vec<IncomingRequestView> {
    engine.incoming_requests()
}

#[tauri::command]
pub fn respond_incoming(
    engine: State<'_, Engine>,
    id: String,
    accept: bool,
    dest_dir: Option<String>,
    overwrite: Option<bool>,
) -> CmdResult<()> {
    engine.respond_incoming(&id, accept, dest_dir.map(PathBuf::from), overwrite)
}

// ---- text ---------------------------------------------------------------

#[tauri::command]
pub async fn send_text(engine: State<'_, Engine>, content: String, recipients: Vec<String>) -> CmdResult<Vec<SendResult>> {
    engine.send_text(content, recipients).await
}

#[tauri::command]
pub fn list_texts(engine: State<'_, Engine>, limit: Option<u32>) -> CmdResult<Vec<TextMessageView>> {
    engine.list_texts(limit.unwrap_or(200))
}

#[tauri::command]
pub fn delete_text(engine: State<'_, Engine>, id: String) -> CmdResult<()> {
    engine.delete_text(&id)
}

#[tauri::command]
pub fn clear_texts(engine: State<'_, Engine>) -> CmdResult<()> {
    engine.clear_texts()
}

/// Writes text to a path the user picked in a save dialog.
#[tauri::command]
pub async fn save_text_file(path: String, content: String) -> CmdResult<()> {
    tokio::fs::write(&path, content.as_bytes()).await.map_err(Error::Io)
}

// ---- clipboard ----------------------------------------------------------

#[tauri::command]
pub fn clipboard_history(engine: State<'_, Engine>, limit: Option<u32>) -> CmdResult<Vec<ClipboardEntryView>> {
    engine.clipboard_history(limit.unwrap_or(100))
}

#[tauri::command]
pub async fn send_clipboard(engine: State<'_, Engine>, recipients: Vec<String>) -> CmdResult<Vec<SendResult>> {
    engine.send_clipboard(recipients).await
}

#[tauri::command]
pub async fn send_clip_entry(engine: State<'_, Engine>, id: i64, recipients: Vec<String>) -> CmdResult<Vec<SendResult>> {
    let text = engine.clip_text(id)?.ok_or_else(|| Error::Invalid("only text entries can be re-sent".into()))?;
    engine.send_clipboard_text(text, recipients).await
}

#[tauri::command]
pub async fn copy_text(engine: State<'_, Engine>, text: String) -> CmdResult<()> {
    engine.copy_text(text).await
}

#[tauri::command]
pub async fn copy_clip(engine: State<'_, Engine>, id: i64) -> CmdResult<()> {
    engine.copy_clip(id).await
}

#[tauri::command]
pub fn delete_clip(engine: State<'_, Engine>, id: i64) -> CmdResult<()> {
    engine.delete_clip(id)
}

#[tauri::command]
pub fn clear_clipboard_history(engine: State<'_, Engine>) -> CmdResult<()> {
    engine.clear_clipboard_history()
}

// ---- history, stats, security --------------------------------------------

#[tauri::command]
pub fn history(engine: State<'_, Engine>, query: HistoryQuery) -> CmdResult<Vec<HistoryEntry>> {
    engine.history(&query)
}

#[tauri::command]
pub fn clear_history(engine: State<'_, Engine>) -> CmdResult<usize> {
    engine.clear_history()
}

#[tauri::command]
pub fn stats(engine: State<'_, Engine>, start_of_day: i64) -> CmdResult<StatsView> {
    engine.stats(start_of_day)
}

#[tauri::command]
pub fn security_events(engine: State<'_, Engine>, limit: Option<u32>) -> CmdResult<Vec<SecurityEventView>> {
    engine.security_events(limit.unwrap_or(100))
}

// ---- settings & system ----------------------------------------------------

#[tauri::command]
pub async fn update_settings(app: AppHandle, engine: State<'_, Engine>, settings: Settings) -> CmdResult<Settings> {
    let saved = engine.update_settings(settings).await?;
    let autolaunch = app.autolaunch();
    let enabled = autolaunch.is_enabled().unwrap_or(false);
    if saved.launch_at_login != enabled {
        let r = if saved.launch_at_login { autolaunch.enable() } else { autolaunch.disable() };
        r.map_err(|e| Error::Other(format!("could not change login item: {e}")))?;
    }
    Ok(saved)
}

#[tauri::command]
pub fn network_interfaces(engine: State<'_, Engine>) -> Vec<NetworkInterfaceView> {
    engine.network_interfaces()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    partial_transfers: u32,
    partial_bytes: u64,
    data_dir: String,
    log_dir: String,
}

#[tauri::command]
pub fn cache_info(app: AppHandle, engine: State<'_, Engine>) -> CmdResult<CacheInfo> {
    let (partial_transfers, partial_bytes) = engine.partial_cache()?;
    let data_dir = app.path().app_data_dir().map_err(other)?;
    let log_dir = app.path().app_log_dir().map_err(other)?;
    Ok(CacheInfo {
        partial_transfers,
        partial_bytes,
        data_dir: data_dir.to_string_lossy().into_owned(),
        log_dir: log_dir.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub fn clear_partial_cache(engine: State<'_, Engine>) -> CmdResult<u32> {
    engine.expire_stale_partials(0)
}

#[tauri::command]
pub fn reveal_path(app: AppHandle, path: String) -> CmdResult<()> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(Error::Invalid("that file no longer exists".into()));
    }
    app.opener().reveal_item_in_dir(p).map_err(other)
}

#[tauri::command]
pub fn open_path(app: AppHandle, path: String) -> CmdResult<()> {
    if !PathBuf::from(&path).exists() {
        return Err(Error::Invalid("that file no longer exists".into()));
    }
    app.opener().open_path(path, None::<&str>).map_err(other)
}

#[tauri::command]
pub fn set_send_to(enabled: bool) -> CmdResult<bool> {
    platform::set_send_to(enabled).map_err(Error::Other)?;
    Ok(platform::send_to_installed())
}

#[tauri::command]
pub fn take_pending_paths(pending: State<'_, PendingPaths>) -> Vec<String> {
    std::mem::take(&mut *pending.0.lock())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    if let Some(engine) = app.try_state::<Engine>() {
        engine.shutdown();
    }
    app.exit(0);
}
