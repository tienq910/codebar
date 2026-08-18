//! 数据模型:与前端 `src/shared/types.ts` 对齐(serde camelCase)。

use serde::{Deserialize, Serialize};

/// 单个 provider 的用量快照
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSnapshot {
    pub id: String,
    pub name: String,
    /// 方案标签,如 "Plus" / "Max"
    pub plan: Option<String>,
    pub status: ProviderStatus,
    pub windows: Vec<UsageWindow>,
    pub cost: Option<Cost>,
    /// 快照生成时间(epoch 秒)
    pub updated_at: i64,
}

/// ok = 数据正常;stale = 已接入但暂无数据/数据过期(软降级);error = 凭据/网络失败
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum ProviderStatus {
    Ok,
    Stale(String),
    Error(String),
}

/// 一个用量窗口(如 5 小时窗口 / 每周窗口 / 余额)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub label: String,
    /// 已用百分比(0-100);余额类窗口可能没有
    pub used_percent: Option<f64>,
    /// 重置时间(epoch 秒)
    pub reset_at: Option<i64>,
    /// 窗口总时长(秒),用于节奏推算
    pub window_seconds: Option<i64>,
    /// 附加说明行,如 "余额 ¥110.00"
    pub note: Option<String>,
    /// 刷新时计算的节奏(超前/落后);仅当可推算
    pub pace: Option<PaceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaceInfo {
    /// 如 "超前 +12%" / "落后 -9%" / "持平"
    pub text: String,
    /// 超前(消耗快于时间进度)时为 true,进度条转 warn→err 渐变
    pub hot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Cost {
    pub today_usd: Option<f64>,
    pub month_usd: Option<f64>,
    pub note: Option<String>,
}

impl UsageWindow {
    /// 节奏推算(纯函数):比较「已用百分比」与「窗口已流逝时间百分比」。
    /// |差| < 2 视为持平;超前为 hot。缺少任一要素时返回 None。
    pub fn pace_against(&self, now_epoch: i64) -> Option<PaceInfo> {
        let used = self.used_percent?;
        let reset_at = self.reset_at?;
        let window = self.window_seconds?;
        if window <= 0 {
            return None;
        }
        let start = reset_at - window;
        if now_epoch <= start {
            return Some(PaceInfo { text: "持平".into(), hot: false });
        }
        let elapsed = (now_epoch - start) as f64 / window as f64;
        let delta = used - elapsed * 100.0;
        if delta.abs() < 2.0 {
            return Some(PaceInfo { text: "持平".into(), hot: false });
        }
        if delta > 0.0 {
            Some(PaceInfo { text: format!("超前 +{}%", delta.round() as i64), hot: true })
        } else {
            Some(PaceInfo { text: format!("落后 -{}%", delta.round().abs() as i64), hot: false })
        }
    }
}

/// 汇总行展示「用量最紧张窗口」:取已用百分比最高者;没有百分比时取第一个窗口。
pub fn worst_window(windows: &[UsageWindow]) -> Option<&UsageWindow> {
    windows
        .iter()
        .filter(|w| w.used_percent.is_some())
        .max_by(|a, b| {
            a.used_percent
                .partial_cmp(&b.used_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .or_else(|| windows.first())
}

/// 窗口秒数 → 展示标签(5 小时窗口 / 每周窗口 / X 小时窗口 / 每月窗口)
pub fn label_for_window_seconds(seconds: i64) -> String {
    const DAY: i64 = 86_400;
    const WEEK: i64 = 604_800;
    if (17_000..DAY).contains(&seconds) {
        let h = (seconds + 3599) / 3600;
        if h == 5 { "5 小时窗口".into() } else { format!("{h} 小时窗口") }
    } else if (DAY..=WEEK * 2).contains(&seconds) {
        "每周窗口".into()
    } else if seconds > WEEK * 2 {
        "每月窗口".into()
    } else {
        "用量窗口".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(used: Option<f64>, reset_at: Option<i64>, window: Option<i64>) -> UsageWindow {
        UsageWindow {
            label: "w".into(),
            used_percent: used,
            reset_at,
            window_seconds: window,
            note: None,
            pace: None,
        }
    }

    #[test]
    fn pace_ahead_when_burning_faster_than_time() {
        // 窗口 5h,已过 1h(20%),已用 32% → 超前 +12%
        let now = 1_000_000;
        let w = win(Some(32.0), Some(now + 4 * 3600), Some(5 * 3600));
        let pace = w.pace_against(now).unwrap();
        assert!(pace.hot);
        assert_eq!(pace.text, "超前 +12%");
    }

    #[test]
    fn pace_behind_and_even() {
        let now = 1_000_000;
        // 已过 50%,已用 41% → 落后 -9%
        let w = win(Some(41.0), Some(now + 1800), Some(3600));
        assert_eq!(w.pace_against(now).unwrap().text, "落后 -9%");
        // 已过 50%,已用 50% → 持平
        let w = win(Some(50.0), Some(now + 1800), Some(3600));
        assert!(!w.pace_against(now).unwrap().hot);
        // 缺窗口时长 → None
        let w = win(Some(50.0), Some(now + 1800), None);
        assert!(w.pace_against(now).is_none());
    }

    #[test]
    fn worst_window_picks_max_used() {
        let ws = vec![
            win(Some(10.0), None, None),
            win(Some(68.0), None, None),
            win(Some(34.0), None, None),
        ];
        assert_eq!(worst_window(&ws).unwrap().used_percent, Some(68.0));
        // 全无百分比时回落第一个
        let ws = vec![win(None, None, None), win(None, None, None)];
        assert_eq!(worst_window(&ws).unwrap().label, "w");
    }

    #[test]
    fn window_labels() {
        assert_eq!(label_for_window_seconds(18_000), "5 小时窗口");
        assert_eq!(label_for_window_seconds(604_800), "每周窗口");
        assert_eq!(label_for_window_seconds(2_592_000), "每月窗口");
    }
}
