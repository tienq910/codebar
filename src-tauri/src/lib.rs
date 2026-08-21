//! CodeBar 入口:托盘 + 双窗口(popup/settings)+ 命令注册。

mod adaptive;
mod auth;
mod config;
mod logger;
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

/// 弹窗定位纯函数:托盘图标矩形(物理像素)上方居中。
/// 返回弹窗左上角物理坐标;(icon_x, icon_y, icon_w) = 图标矩形,popup_w 逻辑宽,
/// popup_h_px 物理高,gap 为与图标的间隙。
pub fn popup_pos_above_tray(
    icon_x: f64,
    icon_y: f64,
    icon_w: f64,
    popup_w: f64,
    popup_h_px: f64,
    scale: f64,
    gap: f64,
) -> (i32, i32) {
    let w_px = popup_w * scale;
    let icon_cx = icon_x + icon_w / 2.0;
    let x = icon_cx - w_px / 2.0;
    let y = icon_y - popup_h_px - gap;
    (x.round() as i32, y.round() as i32)
}

/// 弹窗固定高度(逻辑像素,含 12px 底部留白):空态 / 主界面。
/// 禁止在隐藏窗口上 setSize/setPosition(WebView2 命中区会与可视内容错位,
/// 导致底部按钮"看得见、点不动"),因此高度只在 show_popup 时按已接入状态设定。
const POPUP_H_EMPTY: f64 = 452.0;
const POPUP_H_MAIN: f64 = 572.0;

/// 失焦自动隐藏的宽限期:窗口刚 show 时焦点交接可能产生杂散 blur,立即隐藏会让用户点空
const BLUR_HIDE_GRACE_MS: u128 = 500;

static POPUP_SHOWN_AT: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

/// 失焦后是否允许隐藏弹窗(纯函数,便于单测):宽限期内的 blur 忽略
pub fn blur_hide_allowed(elapsed_ms: u128, grace_ms: u128) -> bool {
    elapsed_ms >= grace_ms
}

/// 托盘图标矩形 → 物理像素 (x, y, width);Logical 值按窗口 scale 换算
fn rect_to_physical(rect: &tauri::Rect, scale: f64) -> (f64, f64, f64) {
    let (x, y) = match rect.position {
        tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
        tauri::Position::Logical(p) => (p.x * scale, p.y * scale),
    };
    let w = match rect.size {
        tauri::Size::Physical(s) => s.width as f64,
        tauri::Size::Logical(s) => s.width * scale,
    };
    (x, y, w)
}

/// 显示弹窗。tray_rect = Some((x, y, w)) 时精确锚定托盘图标上方;
/// None(如第二实例唤起)时兜底走 positioner(需先 show 再定位)。
/// 初始高度按已接入状态取(空态/主界面),显示后前端会实测内容高度
/// 经 set_popup_height 精调;尺寸只在窗口可见时改动(WebView2 bounds 才可同步)。
fn show_popup(app: &AppHandle, tray_rect: Option<(f64, f64, f64)>) {
    let Some(win) = app.get_webview_window("popup") else { return };
    refresh::mark_popup_opened(app); // 记录交互,驱动自适应刷新
    *POPUP_SHOWN_AT.lock().unwrap() = Some(std::time::Instant::now());
    let logical_h = if config::load_config().connected.is_empty() {
        POPUP_H_EMPTY
    } else {
        POPUP_H_MAIN
    };
    match tray_rect {
        Some((ix, iy, iw)) => {
            let scale = win.scale_factor().unwrap_or(1.0);
            let (x, y) = popup_pos_above_tray(ix, iy, iw, 372.0, logical_h * scale, scale, 10.0);
            // 先定位再显示,避免在旧位置闪一帧
            let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
            let _ = win.show();
            // 可见后再设尺寸:wry 只在窗口可见时可靠同步 WebView2 控制器 bounds;
            // 尺寸变化后底边会动,再设一次位置保持锚定托盘上方
            let _ = win.set_size(tauri::LogicalSize::new(372.0, logical_h));
            let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
            let _ = win.set_focus();
        }
        None => {
            let _ = win.show();
            let _ = win.set_size(tauri::LogicalSize::new(372.0, logical_h));
            let _ = win.move_window(Position::TrayBottomRight);
            let _ = win.set_focus();
        }
    }
    // 尺寸诊断:真机空档/裁切问题定位用
    {
        let scale = win.scale_factor().unwrap_or(1.0);
        let inner = win.inner_size().map(|s| format!("{}x{}", s.width, s.height)).unwrap_or_default();
        let outer = win.outer_size().map(|s| format!("{}x{}", s.width, s.height)).unwrap_or_default();
        logger::log(
            logger::Level::Info,
            "window",
            &format!("popup shown: logical_h={logical_h} scale={scale} inner={inner} outer={outer}"),
        );
    }
    // 前台锁兜底:120ms 后再聚焦一次(show 时机下的 set_focus 可能被 Windows 拒绝)
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        if let Some(w) = handle.get_webview_window("popup") {
            let _ = w.set_focus();
        }
    });
}

fn hide_popup(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("popup") {
        let _ = win.hide();
    }
}

fn toggle_popup(app: &AppHandle, tray_rect: Option<(f64, f64, f64)>) {
    let visible = app
        .get_webview_window("popup")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    if visible {
        logger::log(logger::Level::Info, "tray", "tray click → hide popup");
        hide_popup(app);
    } else {
        match tray_rect {
            Some((x, y, w)) => logger::log(
                logger::Level::Info,
                "tray",
                &format!("tray click → show popup (icon rect {x:.0},{y:.0} w{w:.0})"),
            ),
            None => logger::log(logger::Level::Info, "tray", "tray click → show popup (no rect, fallback)"),
        }
        show_popup(app, tray_rect);
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
            logger::log(
                logger::Level::Info,
                "providers",
                &format!("connect {id} (auto): found={} valid={} path={}", scan.found, scan.valid, scan.path.unwrap_or_default()),
            );
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
            match providers::verify_connect(&id, &cred).await {
                Ok(()) => logger::log(logger::Level::Info, "providers", &format!("connect {id} ({:?}): verify ok", desc.auth)),
                Err(e) => {
                    logger::log(logger::Level::Warn, "providers", &format!("connect {id} ({:?}): verify failed: {e}", desc.auth));
                    return Err(e);
                }
            }
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
    logger::log(logger::Level::Info, "providers", &format!("disconnect {id}"));
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
    logger::log(logger::Level::Info, "config", &format!("theme → {theme}"));
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
    logger::log(logger::Level::Info, "config", &format!("refresh_interval → {interval}"));
    let mut cfg = config::load_config();
    cfg.refresh_interval = interval;
    config::save_config(&cfg).map_err(|e| e.to_string())?;
    emit_config_updated(&app);
    Ok(())
}

/// 开机自启:唯一允许写注册表的开关(默认关)
#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    logger::log(logger::Level::Info, "config", &format!("autostart → {enabled}"));
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

/// 前端日志通道(写入 data/codebar.log 的 [ui] 分类)
#[tauri::command]
fn debug_log(message: String) {
    logger::log(logger::Level::Info, "ui", &message);
}

/// 在资源管理器中打开数据目录(含日志文件)
#[tauri::command]
fn open_log_dir(app: AppHandle) -> Result<(), String> {
    let dir = config::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    logger::log(logger::Level::Info, "app", "open log dir requested");
    use tauri_plugin_opener::OpenerExt as _;
    app.opener()
        .reveal_item_in_dir(&dir)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    open_settings_impl(&app);
}

/// 前端实测内容高度 → 精调弹窗高度。
/// 仅在窗口可见时应用(隐藏窗口布局未完成,量高不可靠;可见窗口 resize 安全),
/// 底边锚定不动(贴住托盘上方)。
#[tauri::command]
fn set_popup_height(app: AppHandle, height: f64) {
    let Some(win) = app.get_webview_window("popup") else { return };
    if !win.is_visible().unwrap_or(false) {
        return;
    }
    let h = height.clamp(212.0, 572.0);
    let scale = win.scale_factor().unwrap_or(1.0);
    if let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) {
        let _ = win.set_size(tauri::LogicalSize::new(372.0, h));
        let _ = win.set_position(tauri::PhysicalPosition::new(
            pos.x,
            pos.y + size.height as i32 - (h * scale).round() as i32,
        ));
        logger::log(
            logger::Level::Info,
            "window",
            &format!("popup height → {h} (measured {height:.0})"),
        );
    }
}

fn open_settings_impl(app: &AppHandle) {
    logger::log(logger::Level::Info, "window", "open_settings");
    // 先显式隐藏弹窗:避免"设置窗抢焦点 → popup blur → 隐藏"与按钮 invoke 的时序竞态
    hide_popup(app);
    match app.get_webview_window("settings") {
        Some(win) => {
            let _ = win.unminimize();
            // 重新居中:虚拟机/外接显示器分辨率变化后,创建时的居中坐标可能已把窗口放到屏幕外
            let _ = win.center();
            let _ = win.show();
            let _ = win.set_focus();
            logger::log(logger::Level::Info, "window", "settings window shown");
        }
        None => {
            // 极端兜底:窗口丢失时按配置重建
            let cfg = app
                .config()
                .app
                .windows
                .iter()
                .find(|w| w.label == "settings")
                .cloned();
            match cfg {
                Some(cfg) => match tauri::WebviewWindowBuilder::from_config(app, &cfg) {
                    Ok(builder) => match builder.build() {
                        Ok(win) => {
                            let _ = win.show();
                            let _ = win.set_focus();
                            logger::log(logger::Level::Warn, "window", "settings window recreated+shown");
                        }
                        Err(e) => logger::log(logger::Level::Error, "window", &format!("settings recreate build failed: {e}")),
                    },
                    Err(e) => logger::log(logger::Level::Error, "window", &format!("settings recreate from_config failed: {e}")),
                },
                None => logger::log(logger::Level::Error, "window", "settings window config missing"),
            }
        }
    }
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    logger::log(logger::Level::Info, "app", "quit_app");
    app.exit(0);
    // 兜底:退出流程若被清理卡住,500ms 后强制退出
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(500));
        std::process::exit(0);
    });
}

// ------------------------------------------------------------ 入口

#[cfg(test)]
mod tests {
    use super::{blur_hide_allowed, popup_pos_above_tray};

    #[test]
    fn blur_within_grace_keeps_popup() {
        // 刚 show 完 120ms 内失焦:不隐藏(杂散 blur)
        assert!(!blur_hide_allowed(120, 500));
    }

    #[test]
    fn blur_after_grace_hides_popup() {
        // show 后 2s 失焦:正常隐藏
        assert!(blur_hide_allowed(2000, 500));
    }
    #[test]
    fn popup_anchors_above_tray_icon() {
        // 1080p、100% 缩放:图标位于 (1830, 1052),宽 24;弹窗 372×420,间隙 10
        let (x, y) = popup_pos_above_tray(1830.0, 1052.0, 24.0, 372.0, 420.0, 1.0, 10.0);
        assert_eq!(x, 1830 + 12 - 186); // 图标中心 - 半宽 = 1656
        assert_eq!(y, 1052 - 420 - 10); // 622
    }

    #[test]
    fn popup_honors_dpi_scale() {
        // 150% 缩放:物理坐标按 scale 换算
        let (x, y) = popup_pos_above_tray(2745.0, 1578.0, 36.0, 372.0, 630.0, 1.5, 10.0);
        // 图标中心 2763 - 372*1.5/2 = 2763-279 = 2484
        assert_eq!(x, 2484);
        assert_eq!(y, 1578 - 630 - 10);
    }

    #[test]
    fn popup_near_left_edge_clamps_nowhere_negative() {
        // 图标极靠左:允许 x 为负(多显示器虚拟坐标),不额外 clamp
        let (x, _) = popup_pos_above_tray(0.0, 100.0, 24.0, 372.0, 200.0, 1.0, 10.0);
        assert_eq!(x, 12 - 186);
    }
}

#[cfg_attr(mobile, tauri_mobile_entry_point)]
pub fn run() {    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 第二实例启动:唤起已有实例的弹窗(无托盘矩形,走兜底定位)
            logger::log(logger::Level::Info, "app", "second instance launched → show popup");
            show_popup(app, None);
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
            set_popup_height,
            quit_app,
            debug_log,
            open_log_dir
        ])
        .setup(|app| {
            let cfg = config::load_config();
            logger::log(
                logger::Level::Info,
                "app",
                &format!(
                    "CodeBar v{} 启动 · theme={} interval={} autostart={} connected=[{}]",
                    app.package_info().version,
                    cfg.theme,
                    cfg.refresh_interval,
                    cfg.autostart,
                    cfg.connected.join(",")
                ),
            );

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
                        logger::log(logger::Level::Info, "tray", "menu: refresh");
                        let handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            refresh::refresh_all(&handle).await;
                        });
                    }
                    "settings" => {
                        logger::log(logger::Level::Info, "tray", "menu: settings");
                        open_settings_impl(app);
                    }
                    "quit" => {
                        logger::log(logger::Level::Info, "tray", "menu: quit");
                        app.exit(0);
                        std::thread::spawn(|| {
                            std::thread::sleep(std::time::Duration::from_millis(500));
                            std::process::exit(0);
                        });
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        // 用托盘图标矩形精确锚定弹窗(positioner 对隐藏窗口定位失败,
                        // 会导致弹窗浮在屏幕中间)
                        let scale = tray
                            .app_handle()
                            .get_webview_window("popup")
                            .and_then(|w| w.scale_factor().ok())
                            .unwrap_or(1.0);
                        toggle_popup(tray.app_handle(), Some(rect_to_physical(&rect, scale)));
                    }
                })
                .build(app)?;

            refresh::start_loop(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // popup 失焦自动隐藏(带宽限期:刚 show 时的杂散 blur 不隐藏)
            if window.label() == "popup" {
                match event {
                    tauri::WindowEvent::Focused(true) => {
                        logger::log(logger::Level::Info, "window", "popup focused");
                    }
                    tauri::WindowEvent::Focused(false) => {
                        let elapsed = POPUP_SHOWN_AT
                            .lock()
                            .unwrap()
                            .map(|t| t.elapsed().as_millis())
                            .unwrap_or(u128::MAX);
                        if blur_hide_allowed(elapsed, BLUR_HIDE_GRACE_MS) {
                            logger::log(
                                logger::Level::Info,
                                "window",
                                &format!("popup blurred after {elapsed}ms → hide"),
                            );
                            let _ = window.hide();
                        } else {
                            logger::log(
                                logger::Level::Info,
                                "window",
                                &format!("popup blurred within grace ({elapsed}ms) → keep visible"),
                            );
                        }
                    }
                    _ => {}
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running codebar");
}
