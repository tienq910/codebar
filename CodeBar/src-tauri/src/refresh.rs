//! 刷新循环:手动 + 固定间隔 + 自适应;同一时刻只允许一批 provider 刷新。
//! 结果经 `usage://updated` 事件推给前端,并更新托盘图标 / 触发告警。

use crate::adaptive;
use crate::config::{self, Config, UsageUpdatedPayload};
use crate::models::{ProviderSnapshot, ProviderStatus};
use crate::providers;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

pub const USAGE_EVENT: &str = "usage://updated";
/// 用量告警阈值(≥ 时 toast 一次)
pub const ALERT_THRESHOLD: f64 = 90.0;

pub struct AppState {
    pub snapshots: Mutex<Vec<ProviderSnapshot>>,
    pub refreshing: AtomicBool,
    pub last_refresh: Mutex<Option<i64>>,
    /// 本次会话内 popup 最近一次打开(epoch 秒),自适应刷新依据
    pub last_popup_open: Mutex<Option<i64>>,
    /// 告警去重:window key → 上次已用百分比
    pub alert_memory: Mutex<HashMap<String, f64>>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            snapshots: Mutex::new(Vec::new()),
            refreshing: AtomicBool::new(false),
            last_refresh: Mutex::new(None),
            last_popup_open: Mutex::new(None),
            alert_memory: Mutex::new(HashMap::new()),
        }
    }
}

pub fn mark_popup_opened(app: &AppHandle) {
    let state = app.state::<AppState>();
    *state.last_popup_open.lock().unwrap() = Some(chrono::Utc::now().timestamp());
}

/// 刷新全部已接入 provider(同一时刻仅一批;内部失败不会 panic)
pub async fn refresh_all(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.refreshing.swap(true, Ordering::SeqCst) {
        return; // 已有一批在跑
    }
    emit_state(app, true);

    let cfg = config::load_config();
    let order: Vec<String> = providers::registry()
        .into_iter()
        .map(|d| d.id)
        .filter(|id| cfg.connected.contains(id))
        .collect();

    let mut snapshots = Vec::new();
    for id in &order {
        let snap = providers::fetch_snapshot(id).await;
        snapshots.push(snap);
    }
    let now = chrono::Utc::now().timestamp();
    *state.last_refresh.lock().unwrap() = Some(now);
    *state.snapshots.lock().unwrap() = snapshots.clone();

    check_alerts(app, &snapshots);
    crate::tray::update(app, &snapshots, &cfg);

    state.refreshing.store(false, Ordering::SeqCst);
    emit_state(app, false);
}

fn emit_state(app: &AppHandle, refreshing: bool) {
    let state = app.state::<AppState>();
    let providers = state.snapshots.lock().unwrap().clone();
    let payload = UsageUpdatedPayload {
        providers,
        updated_at: state.last_refresh.lock().unwrap().unwrap_or_else(|| chrono::Utc::now().timestamp()),
        refreshing,
    };
    let _ = app.emit(USAGE_EVENT, &payload);
}

/// 某窗口用量从 <90% 跨到 ≥90% 时发一次 toast
fn check_alerts(app: &AppHandle, snapshots: &[ProviderSnapshot]) {
    use tauri_plugin_notification::NotificationExt;
    let state = app.state::<AppState>();
    let mut mem = state.alert_memory.lock().unwrap();
    for snap in snapshots {
        if !matches!(snap.status, ProviderStatus::Ok) {
            continue;
        }
        for win in &snap.windows {
            let Some(used) = win.used_percent else { continue };
            let key = format!("{}:{}", snap.id, win.label);
            let prev = mem.get(&key).copied().unwrap_or(0.0);
            if used >= ALERT_THRESHOLD && prev < ALERT_THRESHOLD {
                let _ = app
                    .notification()
                    .builder()
                    .title("CodeBar 用量告警")
                    .body(format!("{} {} 已用 {}%", snap.name, win.label, used.round() as i64))
                    .show();
            }
            mem.insert(key, used);
        }
    }
}

/// 启动后台循环。每次刷新完成后重算下一次延迟:
/// - manual:不自动刷新(只靠手动命令,命令侧直接 spawn refresh_all)
/// - 固定间隔:按配置
/// - adaptive:决策表(弹窗打开时间驱动)
/// 手动刷新与循环刷新靠 `refreshing` 互斥,同一时刻仅一批。
pub fn start_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // 启动后先刷一次(拿首屏数据)
        refresh_all(&app).await;
        loop {
            let cfg = config::load_config();
            let delay = next_delay(&app, &cfg);
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
            if cfg.refresh_interval != "manual" {
                refresh_all(&app).await;
            }
        }
    });
}

fn next_delay(app: &AppHandle, cfg: &Config) -> u64 {
    if let Some(secs) = cfg.fixed_interval_secs() {
        return secs;
    }
    if cfg.refresh_interval == "manual" {
        // manual:睡长觉,只靠手动信号唤醒
        return 3600;
    }
    let state = app.state::<AppState>();
    let input = adaptive::AdaptiveInput {
        now_epoch: chrono::Utc::now().timestamp(),
        last_popup_open: *state.last_popup_open.lock().unwrap(),
    };
    let decision = adaptive::decide(&input);
    println!("[codebar] adaptive refresh: reason={} delay={}s", decision.reason, decision.delay_secs);
    decision.delay_secs
}
