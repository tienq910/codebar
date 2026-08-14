//! 密钥存储:`data/secrets.bin`
//! - Windows:整包 JSON 经 DPAPI(CryptProtectData)加密,绑定本机本用户
//!   → exe 拷到别的机器/用户后解密失败,按空密钥处理(密钥安全失效,符合便携预期)
//! - 非 Windows(开发机):加 `DEVPLAIN1` 前缀明文存储,便于 `tauri dev` 调试
//!
//! 另含 OAuth token 刷新后的凭据文件回写工具。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::config::data_dir;

const DEV_PLAIN_MAGIC: &[u8] = b"CODEBAR-DEVPLAIN1\n";

#[derive(Serialize, Deserialize, Default, Debug)]
struct SecretsMap {
    #[serde(flatten)]
    map: HashMap<String, String>,
}

pub struct SecretsStore {
    path: PathBuf,
}

impl SecretsStore {
    pub fn new() -> Self {
        SecretsStore { path: data_dir().join("secrets.bin") }
    }

    /// 读取全部密钥;文件不存在或解密失败(换机/换用户)时返回空表并清掉坏文件
    pub fn load(&self) -> HashMap<String, String> {
        let Ok(blob) = fs::read(&self.path) else {
            return HashMap::new();
        };
        let json = decrypt(&blob);
        match json {
            Ok(json) => serde_json::from_slice::<SecretsMap>(&json)
                .map(|s| s.map)
                .unwrap_or_default(),
            Err(_) => {
                // DPAPI 解不开 = 不是这台机器/这个用户加密的 → 安全失效,重新开始
                let _ = fs::remove_file(&self.path);
                HashMap::new()
            }
        }
    }

    pub fn save(&self, map: &HashMap<String, String>) -> std::io::Result<()> {
        let dir = self.path.parent().unwrap();
        fs::create_dir_all(dir)?;
        let json = serde_json::to_vec(&SecretsMap { map: map.clone() }).expect("serialize secrets");
        let blob = encrypt(&json);
        let tmp = dir.join(".secrets.bin.tmp");
        fs::write(&tmp, blob)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    pub fn set(&self, id: &str, secret: &str) {
        let mut map = self.load();
        map.insert(id.to_string(), secret.to_string());
        let _ = self.save(&map);
    }

    pub fn get(&self, id: &str) -> Option<String> {
        self.load().get(id).cloned()
    }

    pub fn remove(&self, id: &str) {
        let mut map = self.load();
        if map.remove(id).is_some() {
            let _ = self.save(&map);
        }
    }
}

impl Default for SecretsStore {
    fn default() -> Self {
        Self::new()
    }
}

// ------------------------------------------------------------ 加解密

#[cfg(windows)]
fn encrypt(plain: &[u8]) -> Vec<u8> {
    dpapi::protect(plain).unwrap_or_else(|e| {
        eprintln!("[codebar] DPAPI protect failed: {e}");
        plain.to_vec()
    })
}

#[cfg(windows)]
fn decrypt(blob: &[u8]) -> Result<Vec<u8>, String> {
    dpapi::unprotect(blob)
}

#[cfg(not(windows))]
fn encrypt(plain: &[u8]) -> Vec<u8> {
    let mut out = DEV_PLAIN_MAGIC.to_vec();
    out.extend_from_slice(plain);
    out
}

#[cfg(not(windows))]
fn decrypt(blob: &[u8]) -> Result<Vec<u8>, String> {
    if let Some(rest) = blob.strip_prefix(DEV_PLAIN_MAGIC) {
        Ok(rest.to_vec())
    } else {
        // Windows DPAPI blob 在非 Windows 上解不开
        Err("DPAPI blob on non-windows host".into())
    }
}

#[cfg(windows)]
mod dpapi {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
    };

    fn blob_from(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB { cbData: bytes.len() as u32, pbData: bytes.as_ptr() as *mut u8 }
    }

    fn take_blob(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        unsafe {
            let data = Vec::from(std::slice::from_raw_parts(blob.pbData, blob.cbData as usize));
            let _ = LocalFree(Some(HLOCAL(blob.pbData as *mut core::ffi::c_void)));
            data
        }
    }

    pub fn protect(plain: &[u8]) -> Result<Vec<u8>, windows::core::Error> {
        unsafe {
            let mut out = CRYPT_INTEGER_BLOB::default();
            CryptProtectData(
                &blob_from(plain),
                None,
                None,
                None,
                None,
                0,
                &mut out,
            )?;
            Ok(take_blob(out))
        }
    }

    pub fn unprotect(blob: &[u8]) -> Result<Vec<u8>, String> {
        unsafe {
            let mut out = CRYPT_INTEGER_BLOB::default();
            CryptUnprotectData(
                &blob_from(blob),
                None,
                None,
                None,
                None,
                0,
                &mut out,
            )
            .map_err(|e| format!("CryptUnprotectData failed: {e}"))?;
            Ok(take_blob(out))
        }
    }
}

// ------------------------------------------------------------ OAuth 凭据文件回写

/// 将 token 刷新结果合并回 CLI 凭据文件(保留未知字段,pretty + sorted keys)
pub fn merge_json_file(path: &std::path::Path, patch: &serde_json::Value) -> Result<(), String> {
    let mut root: serde_json::Value = fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    merge_into(&mut root, patch);
    let text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
}

fn merge_into(target: &mut serde_json::Value, patch: &serde_json::Value) {
    match (target, patch) {
        (serde_json::Value::Object(t), serde_json::Value::Object(p)) => {
            for (k, v) in p {
                merge_into(t.entry(k.clone()).or_insert(serde_json::Value::Null), v);
            }
        }
        (t, p) => *t = p.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_roundtrip() {
        let _lock = crate::config::DATA_DIR_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("codebar-sec-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        std::env::set_var("CODEBAR_DATA_DIR", &dir);

        let store = SecretsStore::new();
        store.set("openai", "sk-test-1234567890");
        assert_eq!(store.get("openai").as_deref(), Some("sk-test-1234567890"));
        store.set("deepseek", "sk-another");
        assert_eq!(store.get("deepseek").as_deref(), Some("sk-another"));
        store.remove("openai");
        assert!(store.get("openai").is_none());

        std::env::remove_var("CODEBAR_DATA_DIR");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupted_blob_resets_to_empty() {
        let _lock = crate::config::DATA_DIR_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("codebar-sec-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        std::env::set_var("CODEBAR_DATA_DIR", &dir);

        let store = SecretsStore::new();
        fs::write(dir.join("secrets.bin"), b"\x00\x01garbage-not-encrypted").unwrap();
        assert!(store.load().is_empty());
        assert!(!dir.join("secrets.bin").exists()); // 坏文件已清理

        std::env::remove_var("CODEBAR_DATA_DIR");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_preserves_unknown_fields() {
        let dir = std::env::temp_dir().join(format!("codebar-merge-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("auth.json");
        fs::write(&path, r#"{"OPENAI_API_KEY":"x","tokens":{"access_token":"old","refresh_token":"r","account_id":"a"},"last_refresh":"2026-01-01"}"#).unwrap();
        merge_json_file(
            &path,
            &serde_json::json!({"tokens": {"access_token": "new"}, "last_refresh": "2026-08-15"}),
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["tokens"]["access_token"], "new");
        assert_eq!(v["tokens"]["refresh_token"], "r"); // 保留
        assert_eq!(v["OPENAI_API_KEY"], "x"); // 保留
        let _ = fs::remove_dir_all(&dir);
    }
}
