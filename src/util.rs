// ml4w-zh — 工具函数（下载、路径展开、备份、差异对比）

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

pub const BACKUP_DIR: &str = ".ml4w-zh-backups";
pub const CACHE_FILE: &str = "patches.json";

/// 获取用户 HOME 目录
#[must_use]
pub fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/home/danxcheng".into()))
}

/// 展开路径中的 `~` 并处理相对路径
#[must_use]
pub fn expand(path: &str) -> PathBuf {
    let p = path.replace('~', home().to_str().unwrap_or(""));
    if p.starts_with('/') {
        PathBuf::from(p)
    } else {
        home().join(&p)
    }
}

/// 缓存目录: ~/.cache/ml4w-zh/
#[must_use]
pub fn cache_dir() -> PathBuf {
    home().join(".cache/ml4w-zh")
}

/// 缓存文件路径
#[must_use]
pub fn cache_path() -> PathBuf {
    cache_dir().join(CACHE_FILE)
}

/// 下载文件，支持进度显示
///
/// `show_progress` 为 `true` 时显示 curl 进度条，否则静默下载。
pub fn download(url: &str, show_progress: bool) -> Result<String> {
    let mut cmd = Command::new("curl");
    if show_progress {
        cmd.args(["-L", url]);
    } else {
        cmd.args(["-sL", url]);
    }
    let out = cmd.output().with_context(|| format!("curl 失败: {url}"))?;
    if !out.status.success() {
        let code = out.status.code().unwrap_or(0);
        anyhow::bail!("HTTP {code}");
    }
    String::from_utf8(out.stdout).context("响应编码错误")
}

/// 用户确认提示，返回是否确认
#[must_use]
pub fn confirm(msg: &str) -> bool {
    eprint!("{msg} [y/N] ");
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).ok();
    matches!(s.trim().to_lowercase().as_str(), "y" | "yes")
}

/// 计算文件的备份目录（<父目录>/.ml4w-zh-backups）
#[must_use]
pub fn backup_dir_for(path: &Path) -> PathBuf {
    path.parent().map_or_else(|| PathBuf::from(BACKUP_DIR), |p| p.join(BACKUP_DIR))
}

/// 备份文件到 `.ml4w-zh-backups/`
pub fn do_backup(file: &Path) {
    let dir = backup_dir_for(file);
    fs::create_dir_all(&dir).ok();
    let name = file.file_name().unwrap_or(file.as_os_str());
    let bak = dir.join(name);
    if bak.exists() {
        eprintln!("  ⏭️  备份已存在: {}", file.display());
        return;
    }
    fs::copy(file, &bak).ok();
    eprintln!("  📦 已备份: {}", file.display());
}

/// 在文件所在目录创建 `PROTECTED` 标记文件
pub fn do_protect(file: &Path) {
    let dir = match file.parent() {
        Some(d) => d,
        None => return,
    };
    let pf = dir.join("PROTECTED");
    if pf.exists() {
        return;
    }
    fs::write(&pf, "").ok();
    eprintln!(
        "  🛡️  已保护: {}/",
        dir.file_name().unwrap_or_default().to_string_lossy()
    );
}

/// 逐行对比两个文本，返回差异描述
#[must_use]
pub fn diff_text(a: &str, b: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for (i, (la, lb)) in a.lines().zip(b.lines()).enumerate() {
        if la != lb {
            lines.push(format!("  L{}: -{}+{}", i + 1, la, lb));
        }
    }
    let alines: Vec<&str> = a.lines().collect();
    let blines: Vec<&str> = b.lines().collect();
    if alines.len() != blines.len() {
        lines.push(format!(
            "  行数差异: {} vs {}",
            alines.len(),
            blines.len()
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        let home = home();
        let result = expand("~/some/path");
        assert_eq!(result, home.join("some/path"));
    }

    #[test]
    fn test_expand_absolute() {
        let result = expand("/etc/passwd");
        assert_eq!(result, PathBuf::from("/etc/passwd"));
    }

    #[test]
    fn test_expand_relative() {
        let home = home();
        let result = expand("some/relative/path");
        assert_eq!(result, home.join("some/relative/path"));
    }

    #[test]
    fn test_backup_dir_for_subdir() {
        let path = PathBuf::from("/home/user/.config/hypr/hyprland.conf");
        let result = backup_dir_for(&path);
        assert_eq!(
            result,
            PathBuf::from("/home/user/.config/hypr/.ml4w-zh-backups")
        );
    }

    #[test]
    fn test_backup_dir_for_root() {
        let path = PathBuf::from("/file");
        let result = backup_dir_for(&path);
        assert_eq!(result, PathBuf::from("/.ml4w-zh-backups"));
    }

    #[test]
    fn test_diff_text_identical() {
        let result = diff_text("hello\nworld", "hello\nworld");
        assert!(result.is_empty());
    }

    #[test]
    fn test_diff_text_different() {
        let result = diff_text("hello\nworld", "hello\nrust");
        assert!(!result.is_empty());
        assert!(result.iter().any(|l| l.contains("world") && l.contains("rust")));
    }

    #[test]
    fn test_diff_text_length_mismatch() {
        let result = diff_text("a\nb\nc", "a\nb");
        assert!(result.iter().any(|l| l.contains("行数差异")));
    }
}
