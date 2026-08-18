//! Kimi Code provider(API key 模式)
//! 端点:GET {base}/coding/v1/usages,头 Authorization: Bearer <KIMI_CODE_API_KEY>
//! base 默认 https://api.kimi.com,可用 KIMI_CODE_BASE_URL 覆盖。
//! 响应:usage{} = 周配额(limit/used/reset_time);limits[] = 速率窗(5 小时)。
//! 参考 demo/CodexBar Kimi/KimiUsageFetcher.swift + KimiUsageSnapshot.swift。

use crate::models::{ProviderSnapshot, ProviderStatus, UsageWindow};
use crate::providers::{http_client, simple};
use serde::Deserialize;

pub fn base_url() -> String {
    std::env::var("KIMI_CODE_BASE_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "https://api.kimi.com".to_string())
        .trim_end_matches('/')
        .to_string()
}

#[derive(Debug, Deserialize)]
pub struct UsagesResponse {
    #[serde(default)]
    pub usage: Option<Quota>,
    #[serde(default)]
    pub limits: Vec<LimitEntry>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Quota {
    #[serde(default)]
    pub limit: Option<serde_json::Value>,
    #[serde(default)]
    pub used: Option<serde_json::Value>,
    #[serde(default)]
    pub remaining: Option<serde_json::Value>,
    /// usage{} 为 reset_time;limits[].detail 为 resetTime
    #[serde(default, rename = "reset_time", alias = "resetTime")]
    pub reset_time: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct LimitEntry {
    #[serde(default)]
    pub window: WindowSpec,
    #[serde(default)]
    pub detail: Quota,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WindowSpec {
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub time_unit: Option<String>,
}

fn as_f64(v: &Option<serde_json::Value>) -> Option<f64> {
    v.as_ref()
        .and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(|s| s.trim().parse().ok())))
}

/// timeUnit → 分钟倍率(TIME_UNIT_MINUTE/HOUR/DAY)
fn unit_minutes(unit: &Option<String>) -> i64 {
    match unit.as_deref().unwrap_or("") {
        u if u.ends_with("HOUR") => 60,
        u if u.ends_with("DAY") => 1440,
        _ => 1,
    }
}

fn parse_iso(s: &Option<String>) -> Option<i64> {
    let s = s.as_ref()?;
    let t = s.trim();
    // 兼容无时区后缀的 ISO(按 UTC 处理)
    chrono::DateTime::parse_from_rfc3339(t)
        .map(|d| d.timestamp())
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S%.f").map(|n| n.and_utc().timestamp())
        })
        .ok()
}

fn quota_window(q: &Quota, window_seconds: i64, label: String, now: i64) -> Option<UsageWindow> {
    let limit = as_f64(&q.limit)?;
    let used = as_f64(&q.used).or_else(|| as_f64(&q.remaining).map(|rem| limit - rem))?;
    if limit <= 0.0 {
        return None;
    }
    let mut win = UsageWindow {
        label,
        used_percent: Some((used / limit * 100.0).clamp(0.0, 100.0)),
        reset_at: parse_iso(&q.reset_time),
        window_seconds: Some(window_seconds),
        note: Some(format!("{}/{} requests", used.round() as i64, limit.round() as i64)),
        pace: None,
    };
    win.pace = win.pace_against(now);
    Some(win)
}

/// 解析(纯函数,单测覆盖):(速率窗, 周窗)
pub fn parse_usage(text: &str) -> Result<(UsageWindow, UsageWindow), String> {
    let resp: UsagesResponse = serde_json::from_str(text).map_err(|e| format!("响应解析失败:{e}"))?;
    let now = chrono::Utc::now().timestamp();
    // 周配额(Kimi 固定 7 天窗口)
    let weekly = resp
        .usage
        .as_ref()
        .and_then(|q| quota_window(q, 7 * 86400, "每周窗口".into(), now))
        .ok_or("响应中没有周配额")?;
    // 速率窗(limits[0],默认 5 小时)
    let rate = resp
        .limits
        .first()
        .and_then(|l| {
            let secs = l.window.duration.unwrap_or(5) * unit_minutes(&l.window.time_unit) * 60;
            quota_window(&l.detail, secs, crate::models::label_for_window_seconds(secs), now)
        })
        .ok_or("响应中没有速率窗口")?;
    Ok((rate, weekly))
}

pub async fn fetch() -> ProviderSnapshot {
    let now = chrono::Utc::now().timestamp();
    let base = ProviderSnapshot {
        id: "kimi".into(),
        name: "Kimi Code".into(),
        plan: Some("Code".into()),
        status: ProviderStatus::Ok,
        windows: vec![],
        cost: None,
        updated_at: now,
    };
    let Some(key) = crate::auth::SecretsStore::new().get("kimi") else {
        return simple::stale_snapshot(base, "密钥丢失,请重新接入");
    };
    let resp = match http_client()
        .get(format!("{}/coding/v1/usages", base_url()))
        .bearer_auth(&key)
        .header("Accept", "application/json")
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
        Ok((rate, weekly)) => ProviderSnapshot { windows: vec![rate, weekly], ..base },
        Err(e) => simple::stale_snapshot(base, &e),
    }
}

/// 接入校验:直接用粘贴的 key 调一次用量端点
pub async fn verify_key(key: &str) -> Result<(), String> {
    if key.len() < 16 {
        return Err("密钥长度过短".into());
    }
    let resp = http_client()
        .get(format!("{}/coding/v1/usages", base_url()))
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

    /// 响应样例(结构对齐 KimiUsageSnapshot 解析字段)
    const FIXTURE: &str = r#"{
        "usage": {"limit": "60", "used": "12", "remaining": "48", "reset_time": "2026-08-21T10:00:00Z"},
        "limits": [
            {"window": {"duration": 5, "timeUnit": "TIME_UNIT_HOUR"},
             "detail": {"limit": "30", "used": "21", "resetTime": "2026-08-15T12:00:00Z"}}
        ]
    }"#;

    #[test]
    fn parse_two_windows() {
        let (rate, weekly) = parse_usage(FIXTURE).unwrap();
        assert_eq!(rate.label, "5 小时窗口");
        assert_eq!(rate.used_percent, Some(70.0));
        assert_eq!(rate.reset_at, Some(1_786_795_200)); // 2026-08-15T12:00:00Z
        assert!(rate.pace.is_some());
        assert_eq!(weekly.label, "每周窗口");
        assert_eq!(weekly.used_percent, Some(20.0));
        assert_eq!(weekly.note.as_deref(), Some("12/60 requests"));
    }

    #[test]
    fn parse_defensive() {
        assert!(parse_usage("{}").is_err());
        assert!(parse_usage("bad").is_err());
        // 只有周配额、无速率窗 → 报错(不 panic)
        assert!(parse_usage(r#"{"usage":{"limit":"10","used":"1","reset_time":"2026-08-21T10:00:00Z"}}"#).is_err());
        // used 缺失时用 limit-remaining 推算
        let (_, weekly) = parse_usage(
            r#"{"usage":{"limit":"10","remaining":"7","reset_time":"2026-08-21T10:00:00Z"},
                "limits":[{"window":{"duration":5,"timeUnit":"TIME_UNIT_HOUR"},"detail":{"limit":"4","used":"1","resetTime":"2026-08-15T12:00:00Z"}}]}"#,
        )
        .unwrap();
        assert_eq!(weekly.used_percent, Some(30.0));
    }
}
