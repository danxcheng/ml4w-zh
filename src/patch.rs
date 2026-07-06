// ml4w-zh — 补丁定义与校验逻辑

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::util::{cache_dir, cache_path, download, expand, diff_text, do_backup, do_protect};

pub const PATCHES_URL: &str =
    "https://raw.githubusercontent.com/danxcheng/ml4w-zh/master/patches/patches.json";
pub const ML4W_RAW: &str = "https://raw.githubusercontent.com/mylinuxforwork/dotfiles";

/// 一个文件的补丁定义
#[derive(Debug, Deserialize, Serialize)]
pub struct PatchFile {
    pub path: String,
    pub source: String,
    pub backup: bool,
    pub protect: bool,
    pub patches: Vec<Patch>,
}

/// 单条替换规则
#[derive(Debug, Deserialize, Serialize)]
pub struct Patch {
    pub old: String,
    pub new: String,
}

/// 从 GitHub 下载 patches.json 并写入缓存
pub fn fetch_and_cache(show_progress: bool) -> Result<Vec<PatchFile>> {
    let json = download(PATCHES_URL, show_progress)?;
    fs::create_dir_all(cache_dir()).context("创建缓存目录失败")?;
    fs::write(cache_path(), &json).context("写入缓存失败")?;
    parse_patches(&json)
}

/// 从本地缓存读取 patches.json
pub fn load_cached() -> Result<Vec<PatchFile>> {
    let path = cache_path();
    if !path.exists() {
        anyhow::bail!("本地缓存不存在，先运行 `ml4w-zh update-patches` 或 `ml4w-zh apply` 下载。");
    }
    let json = fs::read_to_string(&path).context("读取缓存失败")?;
    parse_patches(&json)
}

fn parse_patches(json: &str) -> Result<Vec<PatchFile>> {
    serde_json::from_str(json).context("patches.json 解析失败")
}

/// 校验 patches.json：检查每条 patch 的 old 字段是否存在于官方源
pub fn validate_patches(files: &[PatchFile]) -> Result<()> {
    eprintln!("━━━ 校验翻译定义 ━━━\n");
    let mut errors = 0;
    for pf in files {
        let url = format!("{ML4W_RAW}/{}", pf.source);
        let official = match download(&url, false) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("  ⚠️  无法获取官方源 ({}): {}", pf.path, e);
                errors += 1;
                continue;
            }
        };
        for p in &pf.patches {
            if !official.contains(&p.old) {
                eprintln!("  ❌ 官方源中未找到: {}", pf.path);
                eprintln!("     old: {}", &p.old[..p.old.len().min(80)]);
                errors += 1;
            }
        }
        if errors == 0 {
            eprintln!("  ✅ {} ({} 条校验通过)", pf.path, pf.patches.len());
        }
    }
    if errors > 0 {
        anyhow::bail!("{errors} 条翻译定义校验失败，请检查 patches.json");
    }
    eprintln!("\n✅ 全部校验通过");
    Ok(())
}

/// 对比本地文件 vs 官方源，返回状态和官方内容
///
/// 返回 (状态, 官方内容)，状态为 `"match"`、`"patched"`、`"mismatch"` 之一。
pub fn verify_against_source(pf: &PatchFile, local: &Path) -> Result<(String, String)> {
    let url = format!("{ML4W_RAW}/{}", pf.source);
    let official = download(&url, false)?;
    let local_content = fs::read_to_string(local).context("读取本地文件失败")?;

    if local_content == official {
        Ok(("match".into(), official))
    } else if local_content.contains(pf.patches.first().map_or("", |p| &p.new)) {
        Ok(("patched".into(), official))
    } else {
        Ok(("mismatch".into(), official))
    }
}

/// 对单个文件执行汉化（或预览）
///
/// 返回替换次数。返回 `Ok(0)` 表示无需修改或已经汉化。
pub fn apply_one(pf: &PatchFile, dry_run: bool) -> Result<usize> {
    let path = expand(&pf.path);
    if !path.exists() {
        anyhow::bail!("文件不存在: {}", pf.path);
    }

    eprint!("  → {} ... ", pf.path);
    let (status, official) = verify_against_source(pf, &path)?;
    match status.as_str() {
        "patched" => {
            eprintln!("⏭️  已汉化");
            return Ok(0);
        }
        "mismatch" => {
            eprintln!("⚠️  与官方不一致，跳过");
            if dry_run {
                let local = fs::read_to_string(&path).unwrap_or_default();
                let d = diff_text(&official, &local);
                for l in &d {
                    eprintln!("{l}");
                }
            }
            return Ok(0);
        }
        _ => {}
    }

    if !dry_run {
        if pf.backup {
            do_backup(&path);
        }
        if pf.protect {
            do_protect(&path);
        }
    }

    let content = fs::read_to_string(&path).context("读取失败")?;
    let mut result = content.clone();
    let mut count = 0;
    for p in &pf.patches {
        if result.contains(&p.new) {
            continue;
        }
        result = result.replace(&p.old, &p.new);
        count += 1;
    }

    if count > 0 {
        if dry_run {
            eprintln!("📝 将汉化 ({count}处)");
        } else {
            fs::write(&path, &result).context("写入失败")?;
            eprintln!("✅ 已汉化 ({count}处)");
        }
    } else {
        eprintln!("✅ 无需修改");
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_patches_valid_json() {
        let json = r#"[
            {
                "path": "test",
                "source": "test",
                "backup": true,
                "protect": false,
                "patches": [
                    {"old": "hello", "new": "你好"}
                ]
            }
        ]"#;
        let result = parse_patches(json);
        assert!(result.is_ok());
        let files = result.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].patches.len(), 1);
        assert_eq!(files[0].patches[0].old, "hello");
        assert_eq!(files[0].patches[0].new, "你好");
    }

    #[test]
    fn test_parse_patches_empty_array() {
        let result = parse_patches("[]");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_parse_patches_missing_field() {
        let json = r#"[
            {"path": "test", "source": "test"}
        ]"#;
        let result = parse_patches(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_patches_invalid_json() {
        let result = parse_patches("{invalid");
        assert!(result.is_err());
    }
}
