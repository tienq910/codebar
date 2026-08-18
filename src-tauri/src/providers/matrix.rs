//! 完整 provider 矩阵(自 CodexBar ProviderManifest 迁移,68 个;Synthetic 为上游测试专用,不迁移)。
//! 顺序:前四为 deepseek / codex / kimi / zai(用户指定),其后保持 CodeBar 既有顺序,再接 CodexBar 注册序。
//!
//! 用量抓取覆盖:codex / claude / deepseek / kimi / zai / moonshot;
//! 其余 provider 可正常接入(凭据经校验后 DPAPI 存储),用量进入「暂不支持」软降级态。

use crate::providers::{AuthKind, ProviderDescriptor};
use std::path::PathBuf;

/// (id, 显示名, 认证方式, 凭据来源说明)
const MATRIX: &[(&str, &str, AuthKind, &str)] = &[
    // ---- 用户指定置顶 ----
    ("deepseek", "DeepSeek", AuthKind::Key, "API Key(DEEPSEEK_API_KEY)"),
    ("codex", "Codex", AuthKind::Auto, "~/.codex/auth.json"),
    ("kimi", "Kimi Code", AuthKind::Key, "API Key(KIMI_CODE_API_KEY)"),
    ("zai", "z.ai / GLM", AuthKind::Key, "API Key(Z_AI_API_KEY)"),
    // ---- 既有 ----
    ("claude", "Claude", AuthKind::Auto, "Claude Code CLI"),
    ("openai", "OpenAI", AuthKind::Key, "Admin API Key"),
    ("gemini", "Gemini", AuthKind::Auto, "Gemini CLI OAuth"),
    ("cursor", "Cursor", AuthKind::Cookie, "浏览器会话 Cookie"),
    // ---- CodexBar 注册序 ----
    ("azureopenai", "Azure OpenAI", AuthKind::Key, "API Key + Endpoint"),
    ("clinepass", "ClinePass", AuthKind::Key, "API Key(CLINE_API_KEY)"),
    ("opencode", "OpenCode", AuthKind::Auto, "~/.local/share/opencode/auth.json"),
    ("opencodego", "OpenCode Go", AuthKind::Auto, "~/.local/share/opencode/auth.json"),
    ("alibaba", "Alibaba Coding Plan", AuthKind::Key, "API Key(DASHSCOPE_API_KEY)"),
    ("alibabatokenplan", "Alibaba Token Plan", AuthKind::Cookie, "浏览器会话 Cookie"),
    ("qwencloud", "Qwen Cloud", AuthKind::Cookie, "浏览器会话 Cookie"),
    ("factory", "Droid", AuthKind::Key, "API Key(FACTORY_API_KEY)"),
    ("fireworks", "Fireworks", AuthKind::Key, "API Key(FIREWORKS_API_KEY)"),
    ("antigravity", "Antigravity", AuthKind::Auto, "Antigrity OAuth 凭据"),
    ("copilot", "Copilot", AuthKind::Key, "Copilot API Token"),
    ("devin", "Devin", AuthKind::Cookie, "浏览器会话 Cookie"),
    ("minimax", "MiniMax", AuthKind::Key, "API Key(MINIMAX_API_KEY)"),
    ("manus", "Manus", AuthKind::Cookie, "浏览器会话 Cookie"),
    ("kilo", "Kilo", AuthKind::Auto, "~/.local/share/kilo/auth.json"),
    ("kiro", "Kiro", AuthKind::Auto, "Kiro CLI 会话"),
    ("vertexai", "Vertex AI", AuthKind::Auto, "gcloud ADC 凭据"),
    ("augment", "Augment", AuthKind::Auto, "Augment CLI 会话"),
    ("jetbrains", "JetBrains AI", AuthKind::Auto, "本地 IDE 配置"),
    ("moonshot", "Moonshot", AuthKind::Key, "API Key(MOONSHOT_API_KEY)"),
    ("amp", "Amp", AuthKind::Key, "API Key(AMP_API_KEY)"),
    ("t3chat", "T3 Chat", AuthKind::Cookie, "浏览器会话 Cookie"),
    ("ollama", "Ollama", AuthKind::Auto, "本地服务(OLLAMA_HOST:11434)"),
    ("openrouter", "OpenRouter", AuthKind::Key, "API Key(OPENROUTER_API_KEY)"),
    ("elevenlabs", "ElevenLabs", AuthKind::Key, "API Key(ELEVENLABS_API_KEY)"),
    ("warp", "Warp", AuthKind::Key, "API Key(WARP_API_KEY)"),
    ("windsurf", "Windsurf", AuthKind::Cookie, "浏览器会话 Cookie"),
    ("zed", "Zed", AuthKind::Auto, "~/.config/zed/settings.json"),
    ("perplexity", "Perplexity", AuthKind::Cookie, "浏览器会话 Cookie"),
    ("mimo", "Xiaomi MiMo", AuthKind::Cookie, "浏览器会话 Cookie"),
    ("doubao", "Doubao", AuthKind::Key, "API Key(ARK_API_KEY)"),
    ("sakana", "Sakana AI", AuthKind::Cookie, "浏览器会话 Cookie"),
    ("abacus", "Abacus AI", AuthKind::Cookie, "浏览器会话 Cookie"),
    ("mistral", "Mistral", AuthKind::Cookie, "浏览器会话 Cookie"),
    ("deepinfra", "DeepInfra", AuthKind::Key, "API Key(DEEPINFRA_API_KEY)"),
    ("codebuff", "Codebuff", AuthKind::Key, "API Key(CODEBUFF_API_KEY)"),
    ("crof", "Crof", AuthKind::Key, "API Key(CROF_API_KEY)"),
    ("venice", "Venice", AuthKind::Key, "API Key(VENICE_API_KEY)"),
    ("commandcode", "Command Code", AuthKind::Cookie, "浏览器会话 Cookie"),
    ("qoder", "Qoder", AuthKind::Cookie, "浏览器会话 Cookie"),
    ("stepfun", "StepFun", AuthKind::Key, "Token(STEPFUN_TOKEN)"),
    ("bedrock", "AWS Bedrock", AuthKind::Key, "Access Key ID + Secret"),
    ("grok", "Grok", AuthKind::Auto, "~/.grok/auth.json"),
    ("groq", "Groq", AuthKind::Key, "API Key(GROQ_API_KEY)"),
    ("llmproxy", "LLM Proxy", AuthKind::Key, "API Key(LLM_PROXY_API_KEY)"),
    ("litellm", "LiteLLM", AuthKind::Key, "API Key(LITELLM_API_KEY)"),
    ("deepgram", "Deepgram", AuthKind::Key, "API Key(DEEPGRAM_API_KEY)"),
    ("poe", "Poe", AuthKind::Key, "API Key(POE_API_KEY)"),
    ("chutes", "Chutes", AuthKind::Key, "API Key(CHUTES_API_KEY)"),
    ("neuralwatt", "Neuralwatt", AuthKind::Key, "API Key(NEURALWATT_API_KEY)"),
    ("clawrouter", "ClawRouter", AuthKind::Key, "API Key(CLAWROUTER_API_KEY)"),
    ("longcat", "LongCat", AuthKind::Cookie, "手动 Cookie 头"),
    ("sub2api", "sub2api", AuthKind::Key, "API Key(SUB2API_API_KEY)"),
    ("wayfinder", "Wayfinder", AuthKind::Key, "网关 URL(免认证)"),
    ("zenmux", "ZenMux", AuthKind::Key, "API Key(ZENMUX_MANAGEMENT_API_KEY)"),
    ("aiand", "ai&", AuthKind::Key, "API Key(AIAND_API_KEY)"),
    ("zoommate", "ZoomMate", AuthKind::Cookie, "浏览器会话 Cookie"),
    ("xai", "xAI", AuthKind::Key, "API Key(XAI_MANAGEMENT_API_KEY)"),
    ("notion", "Notion AI", AuthKind::Cookie, "浏览器会话 Cookie"),
    ("ibmbob", "IBM Bob", AuthKind::Key, "API Key(BOBSHELL_API_KEY)"),
];

pub fn all_descriptors() -> Vec<ProviderDescriptor> {
    MATRIX
        .iter()
        .map(|(id, name, auth, hint)| ProviderDescriptor {
            id: (*id).to_string(),
            name: (*name).to_string(),
            auth: *auth,
            hint: (*hint).to_string(),
        })
        .collect()
}

fn home() -> PathBuf {
    let base = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default();
    PathBuf::from(base)
}

/// 自动识别的凭据文件路径(存在即可接入)
pub fn scan_path(id: &str) -> Option<PathBuf> {
    let h = home();
    let p: PathBuf = match id {
        "codex" => crate::providers::codex::codex_home().join("auth.json"),
        "claude" => crate::providers::claude::credentials_path(),
        "gemini" => h.join(".gemini").join("oauth_creds.json"),
        "grok" => h.join(".grok").join("auth.json"),
        "kilo" => h.join(".local").join("share").join("kilo").join("auth.json"),
        "opencode" | "opencodego" => h.join(".local").join("share").join("opencode").join("auth.json"),
        "zed" => h.join(".config").join("zed").join("settings.json"),
        "vertexai" => std::env::var("CLOUDSDK_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| h.join(".config").join("gcloud"))
            .join("application_default_credentials.json"),
        "antigravity" => h.join(".codexbar").join("oauth_creds.json"),
        "jetbrains" => std::env::var("APPDATA")
            .map(|d| PathBuf::from(d).join("JetBrains"))
            .unwrap_or_else(|_| h.join(".config").join("JetBrains")),
        _ => return None,
    };
    Some(p)
}

/// 本地服务探测(Ollama):默认 127.0.0.1:11434,OLLAMA_HOST 可覆盖
fn probe_ollama() -> bool {
    let host = std::env::var("OLLAMA_HOST")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "127.0.0.1:11434".to_string());
    let addr = host.trim_start_matches("http://").trim_end_matches('/');
    let addr = if addr.contains(':') { addr.to_string() } else { format!("{addr}:11434") };
    std::net::TcpStream::connect_timeout(
        &addr.parse().unwrap_or_else(|_| "127.0.0.1:11434".parse().unwrap()),
        std::time::Duration::from_millis(600),
    )
    .is_ok()
}

/// 矩阵内 auto 类 provider 的通用扫描;返回 None 表示该 id 无扫描路径(走 not found)
pub fn scan_auto(id: &str) -> Option<super::ScanResult> {
    if id == "ollama" {
        return Some(super::ScanResult {
            found: probe_ollama(),
            path: Some("http://127.0.0.1:11434".into()),
            valid: probe_ollama(),
        });
    }
    let path = scan_path(id)?;
    if !path.exists() {
        return Some(super::ScanResult { found: false, path: Some(path.display().to_string()), valid: false });
    }
    // 目录型凭据(jetbrains)存在即可用;文件型做宽松 token 启发式
    let valid = if path.is_dir() {
        std::fs::read_dir(&path).map(|mut d| d.next().is_some()).unwrap_or(false)
    } else {
        std::fs::read_to_string(&path)
            .map(|t| t.to_lowercase().contains("token") || t.to_lowercase().contains("apikey") || t.to_lowercase().contains("api_key"))
            .unwrap_or(false)
    };
    Some(super::ScanResult { found: true, path: Some(path.display().to_string()), valid })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_order_top_four() {
        let ids: Vec<&str> = MATRIX.iter().map(|(id, ..)| *id).collect();
        assert_eq!(&ids[..4], &["deepseek", "codex", "kimi", "zai"], "置顶顺序必须为 deepseek/codex/kimi/zai");
    }

    #[test]
    fn matrix_complete_and_unique() {
        let ids: Vec<&str> = MATRIX.iter().map(|(id, ..)| *id).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "id 必须唯一");
        // CodexBar 69 个,去掉测试专用 Synthetic → 68
        assert_eq!(ids.len(), 68, "全量矩阵应为 68 个(69 - Synthetic)");
        // 关键 provider 都在
        for must in ["claude", "openai", "gemini", "cursor", "moonshot", "grok", "copilot", "ibmbob"] {
            assert!(ids.contains(&must), "缺少 {must}");
        }
    }
}
