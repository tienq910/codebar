//! CodeBar 入口:托盘 + 双窗口(popup/settings)+ 命令注册。

mod adaptive;
mod auth;
mod config;
mod models;
mod providers;
mod refresh;
mod tray;

use crate::auth::SecretsStore;
use crate::config::ConfigUpdatedPayload;
use crate::refresh::AppState;
use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_positioner::{Position, WindowExt};

const CONFIG_EVENT: &str = "config://updated";

// ------------------------------------------------------------ 窗口控制

fn show_popup(app: &AppHandle) {
    let Some(win) = app.get_webview_window("popup") else { return };
    refresh::mark_popup_opened(app); // 记录交互,驱动自适应刷新
    let _ = win.move_window(Position::TrayBottomRight);
    let _ = win.show();
    let _ = win.set_focus();
}

fn hide_popup(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("popup") {
        let _ = win.hide();
    }
}

fn toggle_popup(app: &AppHandle) {
    let visible = app
        .get_webview_window("popup")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    if visible {
        hide_popup(app);
    } else {
        show_popup(app);
    }
}

fn open_settings_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

fn emit_config_updated(app: &AppHandle) {
    let cfg = config::load_config();
    let _ = app.emit(
        CONFIG_EVENT,
        ConfigUpdatedPayload {
            theme: cfg.theme.clone(),
            refresh_interval: cfg.refresh_interval.clone(),
            autostart: cfg.autostart,
            connected: cfg.connected.clone(),
        },
    );
}

fn add_connected(id: &str) {
    let mut cfg = config::load_config();
    if !cfg.connected.iter().any(|c| c == id) {
        cfg.connected.push(id.to_string());
        let _ = config::save_config(&cfg);
    }
}

fn remove_connected(id: &str) {
    let mut cfg = config::load_config();
    cfg.connected.retain(|c| c != id);
    let _ = config::save_config(&cfg);
}

// ------------------------------------------------------------ 命令

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStatePayload {
    theme: String,
    refresh_interval: String,
    autostart: bool,
    connected: Vec<String>,
    providers: Vec<providers::ProviderDescriptor>,
    snapshots: Vec<models::ProviderSnapshot>,
    refreshing: bool,
    last_refresh: Option<i64>,
    data_dir: String,
    version: String,
}

#[tauri::command]
fn get_state(app: AppHandle) -> AppStatePayload {
    let cfg = config::load_config();
    let (snapshots, refreshing, last_refresh) = {
        let state = app.state::<AppState>();
        let snapshots = state.snapshots.lock().unwrap().clone();
        let refreshing = state.refreshing.load(std::sync::atomic::Ordering::SeqCst);
        let last_refresh = *state.last_refresh.lock().unwrap();
        (snapshots, refreshing, last_refresh)
    };
    AppStatePayload {
        theme: cfg.theme,
        refresh_interval: cfg.refresh_interval,
        autostart: cfg.autostart,
        connected: cfg.connected,
        providers: providers::registry(),
        snapshots,
        refreshing,
        last_refresh,
        data_dir: config::data_dir().display().to_string(),
        version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
async fn refresh_now(app: AppHandle) {
    refresh::refresh_all(&app).await;
}

#[tauri::command]
fn scan_cli(id: String) -> providers::ScanResult {
    providers::scan_cli(&id)
}

/// 接入:Auto = 扫描本机凭据;Key/Cookie = 校验后 DPAPI 落 secrets.bin
#[tauri::command]
async fn connect_provider(
    app: AppHandle,
    id: String,
    credential: Option<String>,
) -> Result<(), String> {
    let desc = providers::descriptor(&id).ok_or("未知 provider")?;
    match desc.auth {
        providers::AuthKind::Auto => {
            let scan = providers::scan_cli(&id);
            if scan.found && scan.valid {
                add_connected(&id);
            } else if scan.found {
                return Err("找到凭据文件但内容不可用,请重新登录对应 CLI".into());
            } else {
                return Err("未找到本机凭据".into());
            }
        }
        providers::AuthKind::Key | providers::AuthKind::Cookie => {
            let cred = credential.unwrap_or_default().trim().to_string();
            if cred.is_empty() {
                return Err("请输入凭据".into());
            }
            providers::verify_connect(&id, &cred).await?;
            SecretsStore::new().set(&id, &cred);
            add_connected(&id);
        }
    }
    emit_config_updated(&app);
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        refresh::refresh_all(&handle).await;
    });
    Ok(())
}

#[tauri::command]
fn disconnect_provider(app: AppHandle, id: String) {
    remove_connected(&id);
    SecretsStore::new().remove(&id);
    // 快照里去掉该 provider 并广播
    let snaps = {
        let state = app.state::<AppState>();
        let mut snaps = state.snapshots.lock().unwrap();
        snaps.retain(|s| s.id != id);
        snaps.clone()
    };
    emit_config_updated(&app);
    let cfg = config::load_config();
    tray::update(&app, &snaps, &cfg);
    let _ = app.emit(
        refresh::USAGE_EVENT,
        config::UsageUpdatedPayload {
            providers: snaps,
            updated_at: chrono::Utc::now().timestamp(),
            refreshing: false,
        },
    );
}

#[tauri::command]
fn set_theme(app: AppHandle, theme: String) -> Result<(), String> {
    if !config::THEMES.contains(&theme.as_str()) {
        return Err("未知主题".into());
    }
    let mut cfg = config::load_config();
    cfg.theme = theme;
    config::save_config(&cfg).map_err(|e| e.to_string())?;
    emit_config_updated(&app);
    let snaps = app.state::<AppState>().snapshots.lock().unwrap().clone();
    tray::update(&app, &snaps, &cfg);
    Ok(())
}

#[tauri::command]
fn set_refresh_interval(app: AppHandle, interval: String) -> Result<(), String> {
    if !config::INTERVALS.contains(&interval.as_str()) {
        return Err("未知刷新间隔".into());
    }
    let mut cfg = config::load_config();
    cfg.refresh_interval = interval;
    config::save_config(&cfg).map_err(|e| e.to_string())?;
    emit_config_updated(&app);
    Ok(())
}

/// 开机自启:唯一允许写注册表的开关(默认关)
#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut cfg = config::load_config();
    cfg.autostart = enabled;
    config::save_config(&cfg).map_err(|e| e.to_string())?;
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch.enable().map_err(|e| format!("写入自启失败:{e}"))?;
    } else {
        let _ = autolaunch.disable();
    }
    emit_config_updated(&app);
    Ok(())
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    open_settings_window(&app);
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

// ------------------------------------------------------------ 入口

#[cfg_attr(mobile, tauri_mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 第二实例启动:唤起已有实例的弹窗
            show_popup(app);
        }))
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_state,
            refresh_now,
            scan_cli,
            connect_provider,
            disconnect_provider,
            set_theme,
            set_refresh_interval,
            set_autostart,
            open_settings,
            quit_app
        ])
        .setup(|app| {
            let cfg = config::load_config();

            // 托盘:三段用量条图标(初始按已接入状态),右键菜单,左键开合弹窗
            let refresh_item = MenuItem::with_id(app, "refresh", "刷新", true, None::<&str>)?;
            let settings_item = MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出 CodeBar", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&refresh_item, &settings_item, &quit_item])?;
            let icon =
                tray::render_icon(&[], tray::accent_rgb(&cfg.theme), cfg.connected.is_empty());
            TrayIconBuilder::with_id("codebar-main")
                .icon(icon)
                .tooltip("CodeBar")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "refresh" => {
                        let handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            refresh::refresh_all(&handle).await;
                        });
                    }
                    "settings" => open_settings_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_popup(tray.app_handle());
                    }
                })
                .build(app)?;

            refresh::start_loop(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // popup 失焦自动隐藏
            if window.label() == "popup" {
                if let tauri::WindowEvent::Focused(false) = event {
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running codebar");
}
