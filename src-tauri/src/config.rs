//! exe 同级 `data/config.json` 读写(便携:不写 %APPDATA%)。
//! 测试/开发可用环境变量 `CODEBAR_DATA_DIR` 覆盖数据目录。

use crate::models::ProviderSnapshot;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const THEMES: [&str; 3] = ["hardhacker", "mocha", "latte"];
/// 刷新间隔选项;字符串既做持久化又做 UI 值
pub const INTERVALS: [&str; 7] = ["adaptive", "manual", "1m", "2m", "5m", "15m", "30m"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub theme: String,
    /// "adaptive" | "manual" | "1m" | "2m" | "5m" | "15m" | "30m"
    pub refresh_interval: String,
    /// 开机自启(默认关;开启才写注册表 Run 键)
    pub autostart: bool,
    /// 已接入 provider id 列表
    pub connected: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "hardhacker".into(),
            refresh_interval: "adaptive".into(),
            autostart: false,
            connected: Vec::new(),
        }
    }
}

impl Config {
    pub fn sanitize(&mut self) {
        if !THEMES.contains(&self.theme.as_str()) {
            self.theme = "hardhacker".into();
        }
        if !INTERVALS.contains(&self.refresh_interval.as_str()) {
            self.refresh_interval = "adaptive".into();
        }
    }

    /// 固定间隔模式的间隔秒数;manual 返回 None
    pub fn fixed_interval_secs(&self) -> Option<u64> {
        match self.refresh_interval.as_str() {
            "1m" => Some(60),
            "2m" => Some(120),
            "5m" => Some(300),
            "15m" => Some(900),
            "30m" => Some(1800),
            _ => None,
        }
    }
}

/// 数据目录:优先 `CODEBAR_DATA_DIR` 环境变量;否则 exe 同级 `data/`。
/// (tauri dev 时 exe 在 target/debug 下,数据落在那里,同样满足「不写系统位置」。)
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CODEBAR_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    exe_dir.join("data")
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn load_config() -> Config {
    let mut cfg: Config = fs::read_to_string(config_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    cfg.sanitize();
    cfg
}

/// 原子写:先写临时文件再 rename,避免断电/崩溃留下半截配置
pub fn save_config(cfg: &Config) -> std::io::Result<()> {
    let dir = data_dir();
    fs::create_dir_all(&dir)?;
    let path = config_path();
    let tmp = dir.join(".config.json.tmp");
    let text = serde_json::to_string_pretty(cfg).expect("serialize config");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

// ------------------------------------------------------------
// 事件负载
// ------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageUpdatedPayload {
    /// 按 registry 顺序排列
    pub providers: Vec<ProviderSnapshot>,
    pub updated_at: i64,
    pub refreshing: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigUpdatedPayload {
    pub theme: String,
    pub refresh_interval: String,
    pub autostart: bool,
    pub connected: Vec<String>,
}

/// 测试互斥:多个测试都要改写 `CODEBAR_DATA_DIR`,必须串行
#[cfg(test)]
pub static DATA_DIR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("codebar-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn config_roundtrip_and_sanitize() {
        let _lock = DATA_DIR_LOCK.lock().unwrap();
        let dir = temp_dir();
        std::env::set_var("CODEBAR_DATA_DIR", &dir);

        save_config(&Config {
            theme: "mocha".into(),
            refresh_interval: "5m".into(),
            autostart: false,
            connected: vec!["codex".into()],
        })
        .unwrap();
        let loaded = load_config();
        assert_eq!(loaded.theme, "mocha");
        assert_eq!(loaded.connected, vec!["codex".to_string()]);

        // 非法值回落
        save_config(&Config {
            theme: "neon".into(),
            refresh_interval: "3m".into(),
            autostart: true,
            connected: vec![],
        })
        .unwrap();
        let loaded = load_config();
        assert_eq!(loaded.theme, "hardhacker");
        assert_eq!(loaded.refresh_interval, "adaptive");
        assert!(loaded.autostart);

        std::env::remove_var("CODEBAR_DATA_DIR");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fixed_intervals() {
        let mut cfg = Config::default();
        assert_eq!(cfg.fixed_interval_secs(), None); // adaptive
        cfg.refresh_interval = "2m".into();
        assert_eq!(cfg.fixed_interval_secs(), Some(120));
        cfg.refresh_interval = "manual".into();
        assert_eq!(cfg.fixed_interval_secs(), None);
    }
}
