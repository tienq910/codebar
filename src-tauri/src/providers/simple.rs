//! 密钥类/网页会话类 provider:
//! - openai:API key 校验(GET /v1/models);用量暂无公开稳定端点 → 接入后「暂无数据」
//! - deepseek:API key 校验(GET /models)+ 余额抓取(GET /user/balance)
//! - cursor:Cookie 格式校验(mod.rs);无公开用量端点 → 「暂无数据」
//! - gemini:自动识别 CLI 凭据;用量抓取暂未实现 → 「暂无数据」

use crate::auth::SecretsStore;
use crate::models::{ProviderSnapshot, ProviderStatus, UsageWindow};
use crate::providers::{http_client, ProviderDescriptor};

pub fn stale_snapshot(base: ProviderSnapshot, reason: &str) -> ProviderSnapshot {
    ProviderSnapshot { status: ProviderStatus::Stale(reason.into()), ..base }
}

pub fn no_data_snapshot(desc: ProviderDescriptor) -> ProviderSnapshot {
    ProviderSnapshot {
        id: desc.id.clone(),
        name: desc.name.clone(),
        plan: None,
        status: ProviderStatus::Stale("已接入,该工具暂不支持用量查询".into()),
        windows: vec![],
        cost: None,
        updated_at: chrono::Utc::now().timestamp(),
    }
}

// ------------------------------------------------------------ openai

pub async fn verify_openai_key(key: &str) -> Result<(), String> {
    let key = key.trim();
    if !key.starts_with("sk-") || key.len() < 20 {
        return Err("密钥格式不正确(应以 sk- 开头)".into());
    }
    let resp = http_client()
        .get("https://api.openai.com/v1/models")
        .bearer_auth(key)
        .send()
        .await
        .map_err(|e| format!("网络错误:{e}"))?;
    match resp.status().as_u16() {
        200 => Ok(()),
        401 => Err("401 — 服务端拒绝该密钥".into()),
        403 => Err("403 — 密钥无权限".into()),
        code => Err(format!("服务端错误 HTTP {code}")),
    }
}

// ------------------------------------------------------------ deepseek

pub async fn verify_deepseek_key(key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.len() < 20 {
        return Err("密钥长度过短".into());
    }
    let resp = http_client()
        .get("https://api.deepseek.com/models")
        .bearer_auth(key)
        .send()
        .await
        .map_err(|e| format!("网络错误:{e}"))?;
    match resp.status().as_u16() {
        200 => Ok(()),
        401 => Err("401 — 服务端拒绝该密钥".into()),
        code => Err(format!("服务端错误 HTTP {code}")),
    }
}

#[derive(serde::Deserialize, Debug)]
struct BalanceResponse {
    #[serde(default)]
    balance_infos: Vec<BalanceInfo>,
}

#[derive(serde::Deserialize, Debug)]
struct BalanceInfo {
    #[serde(default)]
    currency: String,
    #[serde(default)]
    total_balance: String,
}

/// 余额 → 展示窗口(百分比未知,note 展示金额)
pub fn parse_balance(text: &str) -> Option<UsageWindow> {
    let resp: BalanceResponse = serde_json::from_str(text).ok()?;
    let info = resp.balance_infos.first()?;
    let symbol = match info.currency.as_str() {
        "CNY" => "¥",
        "USD" => "$",
        other => return Some(UsageWindow {
            label: "余额".into(),
            used_percent: None,
            reset_at: None,
            window_seconds: None,
            note: Some(format!("余额 {} {}", info.total_balance, other)),
            pace: None,
        }),
    };
    Some(UsageWindow {
        label: "余额".into(),
        used_percent: None,
        reset_at: None,
        window_seconds: None,
        note: Some(format!("余额 {symbol}{}", info.total_balance)),
        pace: None,
    })
}

pub async fn fetch_deepseek() -> ProviderSnapshot {
    let now = chrono::Utc::now().timestamp();
    let base = ProviderSnapshot {
        id: "deepseek".into(),
        name: "DeepSeek".into(),
        plan: Some("按量".into()),
        status: ProviderStatus::Ok,
        windows: vec![],
        cost: None,
        updated_at: now,
    };
    let Some(key) = SecretsStore::new().get("deepseek") else {
        return stale_snapshot(base, "密钥丢失,请重新接入");
    };
    let resp = match http_client()
        .get("https://api.deepseek.com/user/balance")
        .bearer_auth(key)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return stale_snapshot(base, &format!("网络错误:{e}")),
    };
    match resp.status().as_u16() {
        200 => {}
        401 => {
            return ProviderSnapshot {
                status: ProviderStatus::Error("401 — 密钥已失效".into()),
                ..base
            }
        }
        code => return stale_snapshot(base, &format!("服务端错误 HTTP {code}")),
    }
    let text = resp.text().await.unwrap_or_default();
    match parse_balance(&text) {
        Some(win) => ProviderSnapshot { windows: vec![win], ..base },
        None => stale_snapshot(base, "余额响应解析失败"),
    }
}

// ------------------------------------------------------------ moonshot

pub fn moonshot_base_url() -> String {
    match std::env::var("MOONSHOT_REGION").as_deref() {
        Ok("china") => "https://api.moonshot.cn".to_string(),
        _ => "https://api.moonshot.ai".to_string(),
    }
}

pub async fn verify_moonshot_key(key: &str) -> Result<(), String> {
    if key.len() < 16 {
        return Err("密钥长度过短".into());
    }
    let resp = http_client()
        .get(format!("{}/v1/users/me/balance", moonshot_base_url()))
        .bearer_auth(key)
        .send()
        .await
        .map_err(|e| format!("网络错误:{e}"))?;
    match resp.status().as_u16() {
        200 => Ok(()),
        401 => Err("401 — 服务端拒绝该密钥".into()),
        code => Err(format!("服务端错误 HTTP {code}")),
    }
}

#[derive(serde::Deserialize, Debug)]
struct MoonshotBalance {
    #[serde(default)]
    code: i64,
    #[serde(default)]
    status: Option<bool>,
    #[serde(default)]
    data: Option<MoonshotData>,
}

#[derive(serde::Deserialize, Debug)]
struct MoonshotData {
    #[serde(default)]
    available_balance: Option<serde_json::Value>,
}

/// 余额(balanceOnly,与 CodexBar Moonshot 一致:只有余额无窗口)
pub fn parse_moonshot_balance(text: &str) -> Option<String> {
    let resp: MoonshotBalance = serde_json::from_str(text).ok()?;
    if resp.code != 0 || resp.status != Some(true) {
        return None;
    }
    let bal = resp
        .data?
        .available_balance
        .and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))?;
    Some(format!("余额 ${bal:.2}"))
}

pub async fn fetch_moonshot() -> ProviderSnapshot {
    let now = chrono::Utc::now().timestamp();
    let base = ProviderSnapshot {
        id: "moonshot".into(),
        name: "Moonshot".into(),
        plan: Some("按量".into()),
        status: ProviderStatus::Ok,
        windows: vec![],
        cost: None,
        updated_at: now,
    };
    let Some(key) = SecretsStore::new().get("moonshot") else {
        return stale_snapshot(base, "密钥丢失,请重新接入");
    };
    let resp = match http_client()
        .get(format!("{}/v1/users/me/balance", moonshot_base_url()))
        .bearer_auth(key)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return stale_snapshot(base, &format!("网络错误:{e}")),
    };
    match resp.status().as_u16() {
        200 => {}
        401 => {
            return ProviderSnapshot { status: ProviderStatus::Error("401 — 密钥已失效".into()), ..base }
        }
        code => return stale_snapshot(base, &format!("服务端错误 HTTP {code}")),
    }
    let text = resp.text().await.unwrap_or_default();
    match parse_moonshot_balance(&text) {
        Some(note) => ProviderSnapshot {
            windows: vec![UsageWindow {
                label: "余额".into(),
                used_percent: None,
                reset_at: None,
                window_seconds: None,
                note: Some(note),
                pace: None,
            }],
            ..base
        },
        None => stale_snapshot(base, "余额响应解析失败"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balance_parsing() {
        let w = parse_balance(
            r#"{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"110.00"}]}"#,
        )
        .unwrap();
        assert_eq!(w.note.as_deref(), Some("余额 ¥110.00"));
        assert!(w.used_percent.is_none());

        let w = parse_balance(
            r#"{"balance_infos":[{"currency":"USD","total_balance":"12.5"}]}"#,
        )
        .unwrap();
        assert_eq!(w.note.as_deref(), Some("余额 $12.5"));

        assert!(parse_balance("{}").is_none());
        assert!(parse_balance("bad").is_none());
    }

    #[test]
    fn moonshot_balance_parsing() {
        let note = parse_moonshot_balance(
            r#"{"code":0,"scode":"ok","status":true,"data":{"available_balance":12.34,"voucher_balance":0,"cash_balance":12.34}}"#,
        );
        assert_eq!(note.as_deref(), Some("余额 $12.34"));
        // code 非 0 / status false → None
        assert!(parse_moonshot_balance(r#"{"code":1,"status":false,"data":{"available_balance":1}}"#).is_none());
        assert!(parse_moonshot_balance("{}").is_none());
        assert!(parse_moonshot_balance("bad").is_none());
    }

    #[test]
    fn no_data_snapshot_shape() {
        use crate::providers::AuthKind;
        let d = ProviderDescriptor {
            id: "cursor".into(),
            name: "Cursor".into(),
            auth: AuthKind::Cookie,
            hint: "浏览器会话 Cookie".into(),
        };
        let s = no_data_snapshot(d);
        assert!(matches!(s.status, ProviderStatus::Stale(_)));
        assert!(s.windows.is_empty());
    }
}
