//! Provider 注册表:每 provider 一个描述符;用量抓取分发到各模块。
//!
//! 覆盖(与原型 CONNECTABLE 一致):
//! - codex / claude / gemini:自动识别本机 CLI 凭据(OAuth)
//! - openai / deepseek:API 密钥
//! - cursor:网页会话 Cookie
//!
//! 用量数据:codex / claude / deepseek 有真实抓取;其余接入后进入「暂无数据」软降级态。

pub mod claude;
pub mod codex;
pub mod simple;

use crate::models::{ProviderSnapshot, ProviderStatus};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthKind {
    Auto,
    Key,
    Cookie,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDescriptor {
    pub id: String,
    pub name: String,
    pub auth: AuthKind,
    /// 凭据来源说明(展示用)
    pub hint: String,
}

pub fn registry() -> Vec<ProviderDescriptor> {
    vec![
        ProviderDescriptor {
            id: "codex".into(),
            name: "Codex".into(),
            auth: AuthKind::Auto,
            hint: "~/.codex/auth.json".into(),
        },
        ProviderDescriptor {
            id: "claude".into(),
            name: "Claude".into(),
            auth: AuthKind::Auto,
            hint: "Claude Code CLI".into(),
        },
        ProviderDescriptor {
            id: "gemini".into(),
            name: "Gemini".into(),
            auth: AuthKind::Auto,
            hint: "Gemini CLI OAuth".into(),
        },
        ProviderDescriptor {
            id: "openai".into(),
            name: "OpenAI".into(),
            auth: AuthKind::Key,
            hint: "Admin API Key".into(),
        },
        ProviderDescriptor {
            id: "deepseek".into(),
            name: "DeepSeek".into(),
            auth: AuthKind::Key,
            hint: "API Key".into(),
        },
        ProviderDescriptor {
            id: "cursor".into(),
            name: "Cursor".into(),
            auth: AuthKind::Cookie,
            hint: "浏览器会话 Cookie".into(),
        },
    ]
}

pub fn descriptor(id: &str) -> Option<ProviderDescriptor> {
    registry().into_iter().find(|d| d.id == id)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub found: bool,
    /// 找到的凭据文件路径(回显给用户)
    pub path: Option<String>,
    /// 凭据文件内容是否为可用的形状
    pub valid: bool,
}

/// 自动识别:扫描本机 CLI 凭据文件
pub fn scan_cli(id: &str) -> ScanResult {
    let path: Option<PathBuf> = match id {
        "codex" => {
            let home = codex::codex_home();
            Some(home.join("auth.json"))
        }
        "claude" => Some(claude::credentials_path()),
        "gemini" => std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok().map(|h| {
            let mut p = PathBuf::from(h);
            p.push(".gemini");
            p.push("oauth_creds.json");
            p
        }),
        _ => None,
    };
    let Some(path) = path else {
        return ScanResult { found: false, path: None, valid: false };
    };
    if !path.exists() {
        return ScanResult { found: false, path: Some(path.display().to_string()), valid: false };
    }
    let valid = match id {
        "codex" => codex::load_credentials().map(|c| c.has_token()).unwrap_or(false),
        "claude" => claude::load_credentials().map(|c| c.has_token()).unwrap_or(false),
        _ => std::fs::read_to_string(&path)
            .map(|t| t.contains("access_token") || t.contains("accessToken"))
            .unwrap_or(false),
    };
    ScanResult { found: true, path: Some(path.display().to_string()), valid }
}

/// 接入校验:key 类走真实 HTTP 验证;cookie 类做格式校验;auto 类由 scan_cli 决定。
/// 返回 Err(message) 表示校验失败(前端展示错误态)。
pub async fn verify_connect(id: &str, credential: &str) -> Result<(), String> {
    match id {
        "openai" => simple::verify_openai_key(credential).await,
        "deepseek" => simple::verify_deepseek_key(credential).await,
        "cursor" => {
            let v = credential.trim();
            if !v.contains('=') || v.len() < 10 {
                Err("需要合法的 Cookie 头(形如 session=…; …)".into())
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

/// 抓取单个 provider 的用量快照(永不 panic;失败 → Error/Stale 状态快照)
pub async fn fetch_snapshot(id: &str) -> ProviderSnapshot {
    let desc = match descriptor(id) {
        Some(d) => d,
        None => {
            return ProviderSnapshot {
                id: id.to_string(),
                name: id.to_string(),
                plan: None,
                status: ProviderStatus::Error("未知 provider".into()),
                windows: vec![],
                cost: None,
                updated_at: chrono::Utc::now().timestamp(),
            }
        }
    };
    match id {
        "codex" => codex::fetch().await,
        "claude" => claude::fetch().await,
        "deepseek" => simple::fetch_deepseek().await,
        _ => simple::no_data_snapshot(desc),
    }
}

/// HTTP 客户端(全局复用,rustls)
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("CodeBar")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("build http client")
}
