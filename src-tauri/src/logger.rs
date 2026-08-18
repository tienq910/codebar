//! 操作日志:exe 同级 `data/codebar.log`(便携,不写系统目录)。
//!
//! - 分级 INFO/WARN/ERROR + 模块分类(`[app] [tray] [refresh] [providers] [config] [ui] …`)
//! - 超过 512KB 轮转为 `codebar.log.old`(覆盖旧轮转文件)
//! - 隐私红线:**绝不**记录密钥/Cookie/token 内容;凭据文件路径允许(排查扫描问题必需)

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

const MAX_BYTES: u64 = 512 * 1024;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

pub fn log(level: Level, category: &str, message: &str) {
    let dir = crate::config::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("codebar.log");
    maybe_rotate(&path, &dir);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(
            f,
            "[{} {:5}] [{}] {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            level.as_str(),
            category,
            message
        );
    }
}

/// 超过 MAX_BYTES 时轮转为 .old(日志频率低,每次写入检查一次 stat 开销可忽略)
fn maybe_rotate(path: &Path, dir: &Path) {
    let Ok(meta) = std::fs::metadata(path) else { return };
    if meta.len() > MAX_BYTES {
        let old = dir.join("codebar.log.old");
        let _ = std::fs::remove_file(&old);
        let _ = std::fs::rename(path, &old);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_writes_and_appends() {
        let _lock = crate::config::DATA_DIR_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = std::env::temp_dir().join(format!("codebar-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("CODEBAR_DATA_DIR", &dir);

        log(Level::Info, "test", "第一行");
        log(Level::Warn, "test", "第二行");
        let content = std::fs::read_to_string(dir.join("codebar.log")).unwrap();
        assert!(content.contains("INFO ] [test] 第一行"), "实际内容: {content}");
        assert!(content.contains("WARN ] [test] 第二行"), "实际内容: {content}");

        std::env::remove_var("CODEBAR_DATA_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotation_moves_big_file_to_old() {
        let _lock = crate::config::DATA_DIR_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = std::env::temp_dir().join(format!("codebar-log-rot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("CODEBAR_DATA_DIR", &dir);
        std::fs::create_dir_all(&dir).unwrap();

        // 预置超限文件 → 下一次写入触发轮转
        std::fs::write(dir.join("codebar.log"), vec![b'x'; (MAX_BYTES + 1) as usize]).unwrap();
        log(Level::Info, "test", "触发轮转");
        assert!(dir.join("codebar.log.old").exists(), "旧文件应被轮转");
        let content = std::fs::read_to_string(dir.join("codebar.log")).unwrap();
        assert!(content.contains("触发轮转"), "新文件应只有新日志");

        std::env::remove_var("CODEBAR_DATA_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
