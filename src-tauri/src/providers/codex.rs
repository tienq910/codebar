//! Codex provider
//! - 凭据:`~/.codex/auth.json`(CODEX_HOME 可覆盖),tokens.access_token / refresh_token / account_id
//! - 刷新:距 last_refresh > 8 天且带 refresh_token → POST https://auth.openai.com/oauth/token
//! - 用量:GET https://chatgpt.com/backend-api/wham/usage
//!   (Bearer + ChatGPT-Account-Id;解析 rate_limit.primary_window/secondary_window)
//! 参考 demo/CodexBar CodexOAuthUsageFetcher.swift / CodexTokenRefresher.swift。

use crate::auth::merge_json_file;
use crate::models::{label_for_window_seconds, ProviderSnapshot, ProviderStatus, UsageWindow};
use crate::providers::http_client;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::path::PathBuf;

const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
/// token 刷新阈值:超过 8 天未刷新则先刷新再抓取
const REFRESH_AFTER_SECS: i64 = 8 * 24 * 3600;

// ------------------------------------------------------------ 凭据

#[derive(Debug, Clone, Default)]
pub struct CodexCredentials {
    pub access_token: String,
    pub refresh_token: String,
    pub account_id: String,
    pub last_refresh: Option<DateTime<Utc>>,
}

impl CodexCredentials {
    pub fn has_token(&self) -> bool {
        !self.access_token.is_empty()
    }
    pub fn needs_refresh(&self, now: DateTime<Utc>) -> bool {
        match self.last_refresh {
            Some(t) => (now - t).num_seconds() > REFRESH_AFTER_SECS,
            None => true,
        }
    }
}

pub fn codex_home() -> PathBuf {
    if let Ok(home) = std::env::var("CODEX_HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home);
        }
    }
    let base = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default();
    PathBuf::from(base).join(".codex")
}

pub fn auth_json_path() -> PathBuf {
    codex_home().join("auth.json")
}

#[derive(Deserialize)]
struct AuthFile {
    #[serde(default)]
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: String,
    #[serde(default)]
    tokens: AuthTokens,
    #[serde(default)]
    last_refresh: String,
}

#[derive(Deserialize, Default)]
struct AuthTokens {
    #[serde(default, alias = "accessToken")]
    access_token: String,
    #[serde(default, alias = "refreshToken")]
    refresh_token: String,
    #[serde(default, alias = "accountId")]
    account_id: String,
}

pub fn load_credentials() -> Option<CodexCredentials> {
    let text = std::fs::read_to_string(auth_json_path()).ok()?;
    let file: AuthFile = serde_json::from_str(&text).ok()?;
    Some(CodexCredentials {
        // API key 模式:OPENAI_API_KEY 充当 access_token(无刷新)
        access_token: if file.tokens.access_token.is_empty() {
            file.openai_api_key
        } else {
            file.tokens.access_token
        },
        refresh_token: file.tokens.refresh_token,
        account_id: file.tokens.account_id,
        last_refresh: DateTime::parse_from_rfc3339(&file.last_refresh)
            .ok()
            .map(|t| t.with_timezone(&Utc)),
    })
}

async fn refresh_token_if_needed(creds: &mut CodexCredentials) -> Result<(), String> {
    let now = Utc::now();
    if !creds.needs_refresh(now) || creds.refresh_token.is_empty() {
        return Ok(());
    }
    crate::logger::log(crate::logger::Level::Info, "providers", "codex: refreshing oauth token");
    let client = http_client();
    let resp = client
        .post(TOKEN_URL)
        .json(&serde_json::json!({
            "client_id": CLIENT_ID,
            "grant_type": "refresh_token",
            "refresh_token": creds.refresh_token,
            "scope": "openid profile email",
        }))
        .send()
        .await
        .map_err(|e| format!("网络错误:{e}"))?;
    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(match status {
            401 => "token 已过期,请运行 codex 重新登录".to_string(),
            _ => format!("刷新失败 HTTP {status}:{}", body.chars().take(200).collect::<String>()),
        });
    }
    #[derive(Deserialize)]
    struct TokenResp {
        #[serde(default)]
        access_token: String,
        #[serde(default)]
        refresh_token: String,
    }
    let tokens: TokenResp = resp.json().await.map_err(|e| format!("响应解析失败:{e}"))?;
    creds.access_token = if tokens.access_token.is_empty() {
        creds.access_token.clone()
    } else {
        tokens.access_token
    };
    if !tokens.refresh_token.is_empty() {
        creds.refresh_token = tokens.refresh_token.clone();
    }
    creds.last_refresh = Some(Utc::now());
    crate::logger::log(crate::logger::Level::Info, "providers", "codex: token refreshed + written back");
    // 回写 auth.json(保留未知字段)
    merge_json_file(
        &auth_json_path(),
        &serde_json::json!({
            "tokens": {
                "access_token": creds.access_token,
                "refresh_token": creds.refresh_token,
                "account_id": creds.account_id,
            },
            "last_refresh": creds.last_refresh.unwrap().to_rfc3339(),
        }),
    )?;
    Ok(())
}

// ------------------------------------------------------------ 响应解析

#[derive(Debug, Deserialize)]
pub struct UsageResponse {
    #[serde(default, rename = "plan_type")]
    pub plan_type: Option<String>,
    #[serde(default, rename = "rate_limit")]
    pub rate_limit: Option<RateLimit>,
    #[serde(default)]
    pub credits: Option<Credits>,
}

#[derive(Debug, Deserialize)]
pub struct RateLimit {
    #[serde(default, rename = "primary_window")]
    pub primary_window: Option<Window>,
    #[serde(default, rename = "secondary_window")]
    pub secondary_window: Option<Window>,
}

#[derive(Debug, Deserialize)]
pub struct Window {
    #[serde(default, rename = "used_percent")]
    pub used_percent: Option<f64>,
    #[serde(default, rename = "reset_at")]
    pub reset_at: Option<i64>,
    #[serde(default, rename = "limit_window_seconds")]
    pub limit_window_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct Credits {
    #[serde(default, rename = "has_credits")]
    pub has_credits: bool,
    #[serde(default)]
    pub unlimited: bool,
    #[serde(default)]
    pub balance: Option<serde_json::Value>,
}

/// 解析(纯函数,单测覆盖)。返回 (方案标签, 窗口列表, credits 备注)
pub fn parse_usage(text: &str) -> Result<(Option<String>, Vec<UsageWindow>, Option<String>), String> {
    let resp: UsageResponse = serde_json::from_str(text).map_err(|e| format!("响应解析失败:{e}"))?;
    let now = Utc::now().timestamp();
    let mut windows = Vec::new();
    if let Some(rl) = &resp.rate_limit {
        for w in [&rl.primary_window, &rl.secondary_window].into_iter().flatten() {
            let window_seconds = w.limit_window_seconds;
            let label = window_seconds.map(label_for_window_seconds).unwrap_or_else(|| "用量窗口".into());
            let mut win = UsageWindow {
                label,
                used_percent: w.used_percent.map(|p| p.clamp(0.0, 100.0)),
                reset_at: w.reset_at,
                window_seconds,
                note: None,
                pace: None,
            };
            win.pace = win.pace_against(now);
            windows.push(win);
        }
    }
    if windows.is_empty() {
        return Err("响应中没有用量窗口".into());
    }
    let plan = resp
        .plan_type
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .filter(|p| !p.is_empty());
    let credit_note = resp.credits.and_then(|c| {
        let bal = c
            .balance
            .and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok())));
        if c.unlimited {
            Some("Credits 无限".into())
        } else if c.has_credits {
            bal.map(|b| format!("Credits ${b:.2}"))
        } else {
            None
        }
    });
    Ok((plan, windows, credit_note))
}

// ------------------------------------------------------------ 抓取

pub async fn fetch() -> ProviderSnapshot {
    let now = Utc::now().timestamp();
    let base = ProviderSnapshot {
        id: "codex".into(),
        name: "Codex".into(),
        plan: None,
        status: ProviderStatus::Ok,
        windows: vec![],
        cost: None,
        updated_at: now,
    };
    let Some(mut creds) = load_credentials() else {
        return crate::providers::simple::stale_snapshot(base, "未找到 ~/.codex/auth.json,请重新接入");
    };
    if !creds.has_token() {
        return crate::providers::simple::stale_snapshot(base, "凭据文件中没有 token,请运行 codex 登录");
    }
    if let Err(e) = refresh_token_if_needed(&mut creds).await {
        return crate::providers::simple::stale_snapshot(base, &e);
    }
    let mut req = http_client().get(USAGE_URL).bearer_auth(&creds.access_token);
    if !creds.account_id.is_empty() {
        req = req.header("ChatGPT-Account-Id", &creds.account_id);
    }
    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => return crate::providers::simple::stale_snapshot(base, &format!("网络错误:{e}")),
    };
    match resp.status().as_u16() {
        200..=299 => {}
        401 | 403 => {
            return ProviderSnapshot { status: ProviderStatus::Error("token 失效,请运行 codex 重新登录".into()), ..base }
        }
        code => {
            let body = resp.text().await.unwrap_or_default();
            return crate::providers::simple::stale_snapshot(
                base,
                &format!("服务端错误 HTTP {code}:{}", body.chars().take(120).collect::<String>()),
            );
        }
    }
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => return crate::providers::simple::stale_snapshot(base, &format!("读取响应失败:{e}")),
    };
    match parse_usage(&text) {
        Ok((plan, windows, note)) => {
            let cost = note.map(|n| crate::models::Cost { today_usd: None, month_usd: None, note: Some(n) });
            ProviderSnapshot { plan, status: ProviderStatus::Ok, windows, cost, ..base }
        }
        Err(e) => crate::providers::simple::stale_snapshot(base, &e),
    }
}

// ------------------------------------------------------------ 测试

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实响应样例(demo/CodexBar Tests/CodexOAuthTests.swift L201-216)
    const FIXTURE: &str = r#"{
        "rate_limit": {
            "primary_window":   { "used_percent": 22, "reset_at": 1766948068, "limit_window_seconds": 18000 },
            "secondary_window": { "used_percent": 43, "reset_at": 1767407914, "limit_window_seconds": 604800 }
        }
    }"#;

    const FIXTURE_FULL: &str = r#"{
        "account_id": "acc-123",
        "plan_type": "plus",
        "rate_limit": {
            "primary_window":   { "used_percent": 68, "reset_at": 1766948068, "limit_window_seconds": 18000 },
            "secondary_window": { "used_percent": 34, "reset_at": 1767407914, "limit_window_seconds": 604800 }
        },
        "credits": { "has_credits": true, "unlimited": false, "balance": "12.5" }
    }"#;

    #[test]
    fn parse_two_windows() {
        let (plan, windows, note) = parse_usage(FIXTURE).unwrap();
        assert_eq!(plan, None);
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].label, "5 小时窗口");
        assert_eq!(windows[0].used_percent, Some(22.0));
        assert_eq!(windows[0].reset_at, Some(1766948068));
        assert_eq!(windows[1].label, "每周窗口");
        assert_eq!(windows[1].used_percent, Some(43.0));
        assert!(note.is_none());
    }

    #[test]
    fn parse_plan_and_credits() {
        let (plan, windows, note) = parse_usage(FIXTURE_FULL).unwrap();
        assert_eq!(plan.as_deref(), Some("Plus"));
        assert_eq!(windows.len(), 2);
        assert_eq!(note.as_deref(), Some("Credits $12.50"));
    }

    #[test]
    fn parse_defensive() {
        // 空响应 / 缺窗口 → 软降级错误
        assert!(parse_usage("{}").is_err());
        assert!(parse_usage("not json").is_err());
        // rate_limit 存在但窗口缺失 → 报错而不是 panic
        assert!(parse_usage(r#"{"rate_limit":{}}"#).is_err());
        // 窗口字段缺 used_percent 也能构造(used=None)
        let (_, windows, _) =
            parse_usage(r#"{"rate_limit":{"primary_window":{"reset_at":1766948068,"limit_window_seconds":18000}}}"#)
                .unwrap();
        assert_eq!(windows[0].used_percent, None);
    }

    #[test]
    fn needs_refresh_after_8_days() {
        let now = Utc::now();
        let mut c = CodexCredentials { access_token: "a".into(), ..Default::default() };
        assert!(c.needs_refresh(now)); // 无 last_refresh
        c.last_refresh = Some(now - chrono::Duration::seconds(REFRESH_AFTER_SECS - 60));
        assert!(!c.needs_refresh(now));
        c.last_refresh = Some(now - chrono::Duration::seconds(REFRESH_AFTER_SECS + 60));
        assert!(c.needs_refresh(now));
    }
}
