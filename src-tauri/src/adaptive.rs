//! 自适应刷新策略(纯函数,参照 CodexBar docs/refresh-loop.md 的决策表简化版):
//!
//! | 条件(距上次弹窗打开) | 间隔 |
//! |---|---|
//! | ≤ 5 分钟(近期有交互) | 2 分钟 |
//! | ≤ 1 小时             | 5 分钟 |
//! | 1–4 小时             | 15 分钟 |
//! | 无记录 或 > 4 小时    | 30 分钟 |
//!
//! 结果恒在 2–30 分钟内;不读取时钟本身,now 由调用方注入。

/// 决策输入
#[derive(Debug, Clone, Copy)]
pub struct AdaptiveInput {
    pub now_epoch: i64,
    /// 上次弹窗打开时间(epoch 秒);None = 本次会话未打开过
    pub last_popup_open: Option<i64>,
}

/// 决策结果:下次刷新延迟(秒) + 稳定原因(用于日志)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveDecision {
    pub delay_secs: u64,
    pub reason: &'static str,
}

pub fn decide(input: &AdaptiveInput) -> AdaptiveDecision {
    let Some(opened) = input.last_popup_open else {
        return AdaptiveDecision { delay_secs: 1800, reason: "longIdle" };
    };
    let elapsed = input.now_epoch - opened;
    if elapsed < 0 {
        // 时钟回拨:按最近交互处理
        AdaptiveDecision { delay_secs: 120, reason: "recentInteraction" }
    } else if elapsed <= 5 * 60 {
        AdaptiveDecision { delay_secs: 120, reason: "recentInteraction" }
    } else if elapsed <= 60 * 60 {
        AdaptiveDecision { delay_secs: 300, reason: "warm" }
    } else if elapsed <= 4 * 60 * 60 {
        AdaptiveDecision { delay_secs: 900, reason: "idle" }
    } else {
        AdaptiveDecision { delay_secs: 1800, reason: "longIdle" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_000_000;

    #[test]
    fn table() {
        let cases: Vec<(Option<i64>, u64, &'static str)> = vec![
            (None, 1800, "longIdle"),
            (Some(NOW - 60), 120, "recentInteraction"),
            (Some(NOW - 5 * 60), 120, "recentInteraction"),
            (Some(NOW - 5 * 60 - 1), 300, "warm"),
            (Some(NOW - 3600), 300, "warm"),
            (Some(NOW - 3601), 900, "idle"),
            (Some(NOW - 4 * 3600), 900, "idle"),
            (Some(NOW - 4 * 3600 - 1), 1800, "longIdle"),
            // 时钟回拨视为近期交互
            (Some(NOW + 300), 120, "recentInteraction"),
        ];
        for (opened, delay, reason) in cases {
            let d = decide(&AdaptiveInput { now_epoch: NOW, last_popup_open: opened });
            assert_eq!((d.delay_secs, d.reason), (delay, reason), "opened={opened:?}");
        }
    }
}
