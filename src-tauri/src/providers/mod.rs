//! Provider 注册表与分发。完整矩阵见 matrix.rs(自 CodexBar 迁移,68 个)。
//!
//! 用量抓取:codex / claude / deepseek / kimi / zai / moonshot;
//! 其余 provider 可接入(凭据校验 + DPAPI 存储),用量进入「暂不支持」软降级态。

pub mod claude;
pub mod codex;
pub mod kimi;
pub mod matrix;
pub mod simple;
pub mod zai;

use crate::models::{ProviderSnapshot, ProviderStatus};
use serde::Serialize;

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
    matrix::all_descriptors()
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

/// 自动识别:扫描本机凭据(codex/claude 用各自的凭据加载器校验,其余走矩阵扫描)
pub fn scan_cli(id: &str) -> ScanResult {
    match id {
        "codex" => {
            let path = codex::auth_json_path();
            if !path.exists() {
                return ScanResult { found: false, path: Some(path.display().to_string()), valid: false };
            }
            ScanResult {
                found: true,
                path: Some(path.display().to_string()),
                valid: codex::load_credentials().map(|c| c.has_token()).unwrap_or(false),
            }
        }
        "claude" => {
            let path = claude::credentials_path();
            if !path.exists() {
                return ScanResult { found: false, path: Some(path.display().to_string()), valid: false };
            }
            ScanResult {
                found: true,
                path: Some(path.display().to_string()),
                valid: claude::load_credentials().map(|c| c.has_token()).unwrap_or(false),
            }
        }
        // 其余 auto 类 provider(kimi 不在此列:它是 key 模式)
        _ => match matrix::scan_auto(id) {
            Some(r) => r,
            None => ScanResult { found: false, path: None, valid: false },
        },
    }
}

/// 接入校验:有真实端点的 key provider 实际调用一次;其余 key/cookie 做格式校验。
/// 返回 Err(message) 表示校验失败(前端展示错误态)。
pub async fn verify_connect(id: &str, credential: &str) -> Result<(), String> {
    let cred = credential.trim();
    match id {
        "openai" => simple::verify_openai_key(cred).await,
        "deepseek" => simple::verify_deepseek_key(cred).await,
        "kimi" => kimi::verify_key(cred).await,
        "zai" => zai::verify_key(cred).await,
        "moonshot" => simple::verify_moonshot_key(cred).await,
        "cursor" => {
            if !cred.contains('=') || cred.len() < 10 {
                Err("需要合法的 Cookie 头(形如 session=…; …)".into())
            } else {
                Ok(())
            }
        }
        _ if descriptor(id).map(|d| d.auth == AuthKind::Key).unwrap_or(false) => {
            if cred.len() < 16 {
                Err("密钥长度过短".into())
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
        "kimi" => kimi::fetch().await,
        "zai" => zai::fetch().await,
        "moonshot" => simple::fetch_moonshot().await,
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
