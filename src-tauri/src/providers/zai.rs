//! z.ai / GLM provider(API key 模式)
//! 端点:GET {base}/api/monitor/usage/quota/limit,头 Authorization: Bearer <Z_AI_API_KEY>
//! base 默认 https://api.z.ai(Z_AI_API_HOST 可覆盖;CN 区 open.bigmodel.cn)。
//! 响应 limits[]:TOKENS/CREDIT limit 按窗口升序 → 小的为 session 窗、大的为周窗;
//! percentage 直接为 0-100;TIME_LIMIT(unit=5,number=1)→ MCP 月度额外窗口。
//! 参考 demo/CodexBar Resources/Plugins/zai.js。

use crate::models::{label_for_window_seconds, ProviderSnapshot, ProviderStatus, UsageWindow};
use crate::providers::{http_client, simple};
use serde::Deserialize;

pub fn base_url() -> String {
    std::env::var("Z_AI_API_HOST")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "https://api.z.ai".to_string())
        .trim_end_matches('/')
        .to_string()
}

#[derive(Debug, Deserialize)]
pub struct QuotaResponse {
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub data: Option<QuotaData>,
}

#[derive(Debug, Deserialize)]
pub struct QuotaData {
    #[serde(default, rename = "planName")]
    pub plan_name: Option<String>,
    #[serde(default)]
    pub limits: Vec<LimitItem>,
}

#[derive(Debug, Deserialize)]
pub struct LimitItem {
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    /// 1=天 3=时 5=分 6=周
    #[serde(default)]
    pub unit: Option<i64>,
    #[serde(default)]
    pub number: Option<i64>,
    #[serde(default)]
    pub percentage: Option<f64>,
    #[serde(default)]
    pub usage: Option<f64>,
    #[serde(default)]
    pub current_value: Option<f64>,
    #[serde(default)]
    pub remaining: Option<f64>,
    /// unix 毫秒
    #[serde(default, rename = "nextResetTime")]
    pub next_reset_time: Option<i64>,
}

fn window_minutes(unit: i64, number: i64) -> i64 {
    let factor = match unit {
        1 => 1440,
        3 => 60,
        5 => 1,
        6 => 10080,
        _ => 60,
    };
    factor * number.max(1)
}

fn to_window(item: &LimitItem, label: String) -> UsageWindow {
    // percent:优先 percentage;缺失时 used/usage 重算
    let used = item
        .current_value
        .or_else(|| item.usage.zip(item.remaining).map(|(u, r)| (u - r).max(0.0)));
    let percent = item
        .percentage
        .or_else(|| used.zip(item.usage.filter(|u| *u > 0.0)).map(|(u, total)| u / total * 100.0));
    let secs = window_minutes(item.unit.unwrap_or(3), item.number.unwrap_or(5)) * 60;
    UsageWindow {
        label,
        used_percent: percent.map(|p| p.clamp(0.0, 100.0)),
        reset_at: item.next_reset_time.map(|ms| ms / 1000),
        window_seconds: Some(secs),
        note: None,
        pace: None,
    }
}

/// 解析(纯函数,单测覆盖):(方案, 窗口列表)
pub fn parse_usage(text: &str) -> Result<(Option<String>, Vec<UsageWindow>), String> {
    let resp: QuotaResponse = serde_json::from_str(text).map_err(|e| format!("响应解析失败:{e}"))?;
    if resp.success == Some(false) {
        return Err("服务端返回失败".into());
    }
    let now = chrono::Utc::now().timestamp();
    let data = resp.data.ok_or("响应中没有 data")?;
    let mut quota_limits: Vec<&LimitItem> = data
        .limits
        .iter()
        .filter(|l| matches!(l.kind.as_deref(), Some("TOKENS_LIMIT") | Some("CREDIT_LIMIT")))
        .collect();
    quota_limits.sort_by_key(|l| window_minutes(l.unit.unwrap_or(3), l.number.unwrap_or(1)));

    let mut windows = Vec::new();
    if let Some(session) = quota_limits.first() {
        let secs = window_minutes(session.unit.unwrap_or(3), session.number.unwrap_or(5)) * 60;
        windows.push(to_window(session, label_for_window_seconds(secs)));
    }
    if let Some(weekly) = quota_limits.last() {
        if quota_limits.len() > 1 {
            let secs = window_minutes(weekly.unit.unwrap_or(6), weekly.number.unwrap_or(1)) * 60;
            windows.push(to_window(weekly, label_for_window_seconds(secs)));
        }
    }
    // TIME_LIMIT(unit=5,number=1)→ MCP 月度
    if let Some(mcp) = data.limits.iter().find(|l| l.kind.as_deref() == Some("TIME_LIMIT") && l.unit == Some(5) && l.number == Some(1)) {
        let mut w = to_window(mcp, "MCP 月度".into());
        w.window_seconds = Some(43200 * 60);
        windows.push(w);
    }
    if windows.is_empty() {
        return Err("响应中没有用量窗口".into());
    }
    for w in &mut windows {
        w.pace = w.pace_against(now);
    }
    Ok((data.plan_name.filter(|p| !p.is_empty()), windows))
}

pub async fn fetch() -> ProviderSnapshot {
    let now = chrono::Utc::now().timestamp();
    let base = ProviderSnapshot {
        id: "zai".into(),
        name: "z.ai / GLM".into(),
        plan: None,
        status: ProviderStatus::Ok,
        windows: vec![],
        cost: None,
        updated_at: now,
    };
    let Some(key) = crate::auth::SecretsStore::new().get("zai") else {
        return simple::stale_snapshot(base, "密钥丢失,请重新接入");
    };
    let resp = match http_client()
        .get(format!("{}/api/monitor/usage/quota/limit", base_url()))
        .bearer_auth(&key)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return simple::stale_snapshot(base, &format!("网络错误:{e}")),
    };
    match resp.status().as_u16() {
        200 => {}
        401 | 403 => {
            return ProviderSnapshot { status: ProviderStatus::Error("401 — 密钥已失效".into()), ..base }
        }
        code => return simple::stale_snapshot(base, &format!("服务端错误 HTTP {code}")),
    }
    let text = resp.text().await.unwrap_or_default();
    match parse_usage(&text) {
        Ok((plan, windows)) => ProviderSnapshot { plan, windows, ..base },
        Err(e) => simple::stale_snapshot(base, &e),
    }
}

/// 接入校验:直接用粘贴的 key 调一次配额端点
pub async fn verify_key(key: &str) -> Result<(), String> {
    if key.len() < 16 {
        return Err("密钥长度过短".into());
    }
    let resp = http_client()
        .get(format!("{}/api/monitor/usage/quota/limit", base_url()))
        .bearer_auth(key)
        .send()
        .await
        .map_err(|e| format!("网络错误:{e}"))?;
    match resp.status().as_u16() {
        200 => Ok(()),
        401 | 403 => Err("401 — 服务端拒绝该密钥".into()),
        code => Err(format!("服务端错误 HTTP {code}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 响应样例(结构对齐 zai.js 解析字段)
    const FIXTURE: &str = r#"{
        "success": true, "code": 200, "msg": "",
        "data": {
            "planName": "Max",
            "limits": [
                {"type": "TOKENS_LIMIT", "unit": 3, "number": 5, "percentage": 42,
                 "usage": 1000000, "currentValue": 420000, "remaining": 580000,
                 "nextResetTime": 1760000000000},
                {"type": "TIME_LIMIT", "unit": 5, "number": 1, "percentage": 10,
                 "usage": 1, "currentValue": 0, "remaining": 1, "nextResetTime": 1761000000000},
                {"type": "TOKENS_LIMIT", "unit": 6, "number": 1, "percentage": 30,
                 "usage": 5000000, "currentValue": 1500000, "remaining": 3500000,
                 "nextResetTime": 1762000000000}
            ]
        }
    }"#;

    #[test]
    fn parse_session_weekly_mcp() {
        let (plan, windows) = parse_usage(FIXTURE).unwrap();
        assert_eq!(plan.as_deref(), Some("Max"));
        assert_eq!(windows.len(), 3);
        // session(5h)在前,weekly 在后,MCP 第三
        assert_eq!(windows[0].label, "5 小时窗口");
        assert_eq!(windows[0].used_percent, Some(42.0));
        assert_eq!(windows[0].reset_at, Some(1_760_000_000));
        assert_eq!(windows[1].label, "每周窗口");
        assert_eq!(windows[1].used_percent, Some(30.0));
        assert_eq!(windows[2].label, "MCP 月度");
    }

    #[test]
    fn parse_percent_fallback_and_defensive() {
        // percentage 缺失 → currentValue/usage 重算(25%)
        let (plan, windows) = parse_usage(
            r#"{"success":true,"data":{"planName":"","limits":[
                {"type":"CREDIT_LIMIT","unit":3,"number":5,"usage":400,"currentValue":100,"remaining":300,"nextResetTime":1760000000000}]}}"#,
        )
        .unwrap();
        assert!(plan.is_none());
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].used_percent, Some(25.0));
        // 空/坏响应
        assert!(parse_usage("{}").is_err());
        assert!(parse_usage("bad").is_err());
        assert!(parse_usage(r#"{"success":true,"data":{"limits":[]}}"#).is_err());
        assert!(parse_usage(r#"{"success":false}"#).is_err());
    }
}
