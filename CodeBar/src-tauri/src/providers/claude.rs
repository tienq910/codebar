//! Claude provider
//! - 凭据:`~/.claude/.credentials.json`(CLAUDE_CONFIG_DIR 可覆盖)
//!   claudeAiOAuth.{accessToken, refreshToken, expiresAt(毫秒), rateLimitTier}
//! - 刷新:过期前 5 分钟内 → POST https://platform.claude.com/v1/oauth/token
//! - 用量:GET https://api.anthropic.com/api/oauth/usage
//!   (Bearer + anthropic-beta: oauth-2025-04-20;解析 five_hour/seven_day,
//!    utilization 即 0-100 百分比,resets_at 为 ISO8601)
//! 参考 demo/CodexBar ClaudeOAuthUsageFetcher.swift / ClaudeOAuthCredentials.swift。

use crate::auth::merge_json_file;
use crate::models::{ProviderSnapshot, ProviderStatus, UsageWindow};
use crate::providers::http_client;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::path::PathBuf;

const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const BETA_HEADER: &str = "oauth-2025-04-20";
const USER_AGENT: &str = "claude-code/2.1.0";
/// 过期前 5 分钟即刷新
const REFRESH_MARGIN_SECS: i64 = 300;

const FIVE_HOUR_SECS: i64 = 5 * 3600;
const SEVEN_DAY_SECS: i64 = 7 * 24 * 3600;

// ------------------------------------------------------------ 凭据

#[derive(Debug, Clone, Default)]
pub struct ClaudeCredentials {
    pub access_token: String,
    pub refresh_token: String,
    /// epoch 秒
    pub expires_at: Option<i64>,
    pub rate_limit_tier: Option<String>,
    pub subscription_type: Option<String>,
}

impl ClaudeCredentials {
    pub fn has_token(&self) -> bool {
        !self.access_token.is_empty()
    }
}

pub fn claude_config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CLAUDE_CONFIG_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    let base = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default();
    PathBuf::from(base).join(".claude")
}

pub fn credentials_path() -> PathBuf {
    claude_config_dir().join(".credentials.json")
}

#[derive(Deserialize, Default)]
struct CredentialsFile {
    #[serde(default, rename = "claudeAiOAuth", alias = "claudeAiOauth")]
    oauth: Oauth,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Oauth {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    expires_at: Option<serde_json::Value>,
    #[serde(default)]
    rate_limit_tier: Option<String>,
    #[serde(default)]
    subscription_type: Option<String>,
}

fn parse_epoch_ms(v: &serde_json::Value) -> Option<i64> {
    let ms = v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))?;
    Some((ms / 1000.0) as i64)
}

pub fn load_credentials() -> Option<ClaudeCredentials> {
    let text = std::fs::read_to_string(credentials_path()).ok()?;
    let file: CredentialsFile = serde_json::from_str(&text).ok()?;
    Some(ClaudeCredentials {
        access_token: file.oauth.access_token,
        refresh_token: file.oauth.refresh_token,
        expires_at: file.oauth.expires_at.as_ref().and_then(parse_epoch_ms),
        rate_limit_tier: file.oauth.rate_limit_tier,
        subscription_type: file.oauth.subscription_type,
    })
}

async fn refresh_token_if_needed(creds: &mut ClaudeCredentials) -> Result<(), String> {
    let now = Utc::now().timestamp();
    let needs = match creds.expires_at {
        Some(exp) => exp - now < REFRESH_MARGIN_SECS,
        None => true,
    };
    if !needs || creds.refresh_token.is_empty() {
        return Ok(());
    }
    crate::logger::log(crate::logger::Level::Info, "providers", "claude: refreshing oauth token");
    let resp = http_client()
        .post(TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .query(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", creds.refresh_token.as_str()),
            ("client_id", CLIENT_ID),
        ])
        .send()
        .await
        .map_err(|e| format!("网络错误:{e}"))?;
    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        return Err(match status {
            401 => "token 已过期,请运行 claude 重新登录".to_string(),
            _ => format!("刷新失败 HTTP {status}"),
        });
    }
    #[derive(Deserialize)]
    struct TokenResp {
        #[serde(default)]
        access_token: String,
        #[serde(default)]
        refresh_token: String,
        #[serde(default)]
        expires_in: Option<i64>,
    }
    let tokens: TokenResp = resp.json().await.map_err(|e| format!("响应解析失败:{e}"))?;
    if !tokens.access_token.is_empty() {
        creds.access_token = tokens.access_token.clone();
    }
    if !tokens.refresh_token.is_empty() {
        creds.refresh_token = tokens.refresh_token.clone();
    }
    creds.expires_at = Some(now + tokens.expires_in.unwrap_or(3600));
    merge_json_file(
        &credentials_path(),
        &serde_json::json!({
            "claudeAiOAuth": {
                "accessToken": creds.access_token,
                "refreshToken": creds.refresh_token,
                "expiresAt": creds.expires_at.unwrap() * 1000,
            }
        }),
    )?;
    Ok(())
}

// ------------------------------------------------------------ 响应解析

#[derive(Debug, Deserialize)]
pub struct UsageResponse {
    #[serde(default)]
    five_hour: Option<OAuthWindow>,
    #[serde(default)]
    seven_day: Option<OAuthWindow>,
}

#[derive(Debug, Deserialize)]
struct OAuthWindow {
    #[serde(default)]
    utilization: Option<f64>,
    #[serde(default)]
    resets_at: Option<String>,
}

fn parse_iso8601(s: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp())
}

/// 解析(纯函数,单测覆盖)。返回 (方案标签, 窗口)
pub fn parse_usage(text: &str) -> Result<(Option<String>, Vec<UsageWindow>), String> {
    let resp: UsageResponse = serde_json::from_str(text).map_err(|e| format!("响应解析失败:{e}"))?;
    let now = Utc::now().timestamp();
    let mut windows = Vec::new();
    let items = [
        (resp.five_hour.as_ref(), "5 小时窗口", FIVE_HOUR_SECS),
        (resp.seven_day.as_ref(), "每周窗口", SEVEN_DAY_SECS),
    ];
    for (w, label, secs) in items {
        let Some(w) = w else { continue };
        let mut win = UsageWindow {
            label: label.to_string(),
            // utilization 本身就是 0-100 刻度(CodexBar 同款处理)
            used_percent: w.utilization.map(|u| u.clamp(0.0, 100.0)),
            reset_at: w.resets_at.as_deref().and_then(parse_iso8601),
            window_seconds: Some(secs),
            note: None,
            pace: None,
        };
        win.pace = win.pace_against(now);
        windows.push(win);
    }
    if windows.is_empty() {
        return Err("响应中没有用量窗口".into());
    }
    Ok((None, windows))
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

// ------------------------------------------------------------ 抓取

pub async fn fetch() -> ProviderSnapshot {
    let now = Utc::now().timestamp();
    let base = ProviderSnapshot {
        id: "claude".into(),
        name: "Claude".into(),
        plan: None,
        status: ProviderStatus::Ok,
        windows: vec![],
        cost: None,
        updated_at: now,
    };
    let Some(mut creds) = load_credentials() else {
        return simple::stale_snapshot(base, "未找到 ~/.claude/.credentials.json,请重新接入");
    };
    if !creds.has_token() {
        return simple::stale_snapshot(base, "凭据文件中没有 token,请运行 claude 登录");
    }
    if let Err(e) = refresh_token_if_needed(&mut creds).await {
        return simple::stale_snapshot(base, &e);
    }
    let resp = match http_client()
        .get(USAGE_URL)
        .bearer_auth(&creds.access_token)
        .header("anthropic-beta", BETA_HEADER)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return simple::stale_snapshot(base, &format!("网络错误:{e}")),
    };
    match resp.status().as_u16() {
        200 => {}
        401 | 403 => {
            return ProviderSnapshot {
                status: ProviderStatus::Error("token 失效,请运行 claude 重新登录".into()),
                ..base
            }
        }
        429 => return simple::stale_snapshot(base, "服务端限流,请稍后刷新重试"),
        code => return simple::stale_snapshot(base, &format!("服务端错误 HTTP {code}")),
    }
    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => return simple::stale_snapshot(base, &format!("读取响应失败:{e}")),
    };
    match parse_usage(&text) {
        Ok((_, windows)) => {
            let plan = creds
                .subscription_type
                .as_deref()
                .or(creds.rate_limit_tier.as_deref())
                .filter(|s| !s.is_empty())
                .map(capitalize);
            ProviderSnapshot { plan, status: ProviderStatus::Ok, windows, ..base }
        }
        Err(e) => simple::stale_snapshot(base, &e),
    }
}

use super::simple;

// ------------------------------------------------------------ 测试

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实响应样例(demo/CodexBar Tests/ClaudeUsageTests.swift L671-677 同形)
    const FIXTURE: &str = r#"{
        "five_hour":      { "utilization": 9,  "resets_at": "2025-12-23T16:00:00.000Z" },
        "seven_day":      { "utilization": 4,  "resets_at": "2025-12-29T23:00:00.000Z" },
        "seven_day_opus": { "utilization": 1 }
    }"#;

    #[test]
    fn parse_two_windows() {
        let (plan, windows) = parse_usage(FIXTURE).unwrap();
        assert!(plan.is_none());
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].label, "5 小时窗口");
        assert_eq!(windows[0].used_percent, Some(9.0));
        assert_eq!(windows[0].reset_at, Some(1_766_505_600));
        assert_eq!(windows[1].label, "每周窗口");
        assert_eq!(windows[1].used_percent, Some(4.0));
    }

    #[test]
    fn parse_defensive() {
        assert!(parse_usage("{}").is_err());
        assert!(parse_usage("not json").is_err());
        // 只有一个窗口也能出
        let (_, windows) =
            parse_usage(r#"{"seven_day": {"utilization": 42}}"#).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].used_percent, Some(42.0));
        assert!(windows[0].reset_at.is_none());
    }

    #[test]
    fn epoch_ms_parsing() {
        assert_eq!(parse_epoch_ms(&serde_json::json!(1766505600000i64)), Some(1_766_505_600));
        assert_eq!(parse_epoch_ms(&serde_json::json!("1766505600000")), Some(1_766_505_600));
        assert_eq!(parse_epoch_ms(&serde_json::json!(null)), None);
    }
}
