// ml4w-zh — CLI 定义与子命令分发

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::patch::{self, fetch_and_cache, load_cached, validate_patches, Patch, PatchFile};
use crate::util::{self, confirm};

#[derive(Parser)]
#[command(name = "ml4w-zh", version, about = "ML4W dotfiles 汉化工具")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 从 ml4w 官方源校验后汉化
    Apply {
        /// 预览，不修改
        #[arg(long)]
        dry_run: bool,
        /// 跳过确认
        #[arg(short)]
        yes: bool,
    },
    /// 检查汉化状态
    Check {
        /// 使用本地缓存，不联网
        #[arg(long)]
        offline: bool,
    },
    /// 恢复备份
    Revert {
        /// 指定文件名（可选，不指定则恢复全部）
        file: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
    /// 仅备份 + PROTECTED
    Init {
        #[arg(long)]
        dry_run: bool,
    },
    /// 下载最新翻译定义
    UpdatePatches {
        /// 下载后校验
        #[arg(long)]
        validate: bool,
    },
    /// 从 TOML 定义生成 patches.json（开发者用）
    Generate,
}

/// 程序入口：解析 CLI 参数并分发子命令
pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Apply { dry_run, yes } => {
            let files = fetch_and_cache(false)?;
            cmd_apply(&files, dry_run, yes)
        }
        Command::Check { offline } => {
            let files = if offline {
                load_cached()
            } else {
                fetch_and_cache(false)
            }?;
            cmd_check(&files);
            Ok(())
        }
        Command::Revert { file, dry_run } => cmd_revert(dry_run, file.as_deref()),
        Command::Init { dry_run } => {
            let files = fetch_and_cache(false)?;
            cmd_init(&files, dry_run)
        }
        Command::UpdatePatches { validate } => cmd_update_patches(validate),
        Command::Generate => cmd_generate(),
    }
}

// ─── 子命令实现 ─────────────────────────────────────────────

#[allow(clippy::unnecessary_wraps)]
fn cmd_apply(files: &[PatchFile], dry_run: bool, skip_confirm: bool) -> Result<()> {
    eprintln!("━━━ ML4W 中文汉化 ━━━\n");
    if !skip_confirm && !dry_run && !confirm("将备份并汉化，是否继续？") {
        eprintln!("已取消");
        return Ok(());
    }
    let mut total = 0;
    let mut skipped = 0;
    let mut errors = 0;
    for pf in files {
        match patch::apply_one(pf, dry_run) {
            Ok(n) => {
                if n == 0 && !dry_run {
                    skipped += 1;
                }
                total += n;
            }
            Err(e) => {
                eprintln!("  ❌ {}: {}", pf.path, e);
                errors += 1;
            }
        }
    }
    if dry_run {
        eprintln!("\n预览结束，将修改 {total} 处。去掉 --dry-run 执行。");
    } else {
        eprintln!("\n✅ 完成！{total} 处翻译 ({skipped} 跳过, {errors} 错误)");
        eprintln!("   按 Super + Ctrl + R 重载 Hyprland 配置\n");
    }
    Ok(())
}

fn cmd_check(files: &[PatchFile]) {
    eprintln!("━━━ 翻译状态检查 ━━━\n");
    let mut ok = 0;
    let mut outdated = 0;
    let mut missing = 0;
    let mut updated = 0;
    for pf in files {
        let path = util::expand(&pf.path);
        if !path.exists() {
            eprintln!("  ❌ 缺失: {}", pf.path);
            missing += 1;
            continue;
        }
        let content = if let Ok(c) = fs::read_to_string(&path) { c } else {
            eprintln!("  ❌ 无法读取: {}", pf.path);
            missing += 1;
            continue;
        };
        let first_new = pf.patches.first().map_or("", |p| &p.new);
        let is_zh = !first_new.is_empty() && content.contains(first_new);

        let url = format!("{}/{}", patch::ML4W_RAW, pf.source);
        let official = if let Ok(s) = util::download(&url, false) { s } else {
            eprintln!("  ⚠️  无法连接 ml4w 源: {}", pf.path);
            outdated += 1;
            continue;
        };

        if content == official && !is_zh {
            eprintln!("  📝 未汉化: {}", pf.path);
            outdated += 1;
        } else if content == official && is_zh {
            eprintln!("  ✅ 已汉化: {}", pf.path);
            ok += 1;
        } else if content != official && is_zh {
            eprintln!("  🔄 需更新: {}", pf.path);
            updated += 1;
        } else {
            eprintln!("  ⚠️  自定义: {}", pf.path);
            outdated += 1;
        }
    }
    eprintln!("\n总计 {} 个文件", files.len());
    eprintln!("  ✅ 已汉化: {ok}");
    eprintln!("  🔄 需更新: {updated}");
    eprintln!("  📝 未汉化: {outdated}");
    eprintln!("  ❌ 缺失:   {missing}");
}

fn cmd_revert(dry_run: bool, target: Option<&str>) -> Result<()> {
    eprintln!("━━━ 恢复英文原版 ━━━\n");
    if !dry_run && !confirm("将从备份恢复，是否继续？") {
        eprintln!("已取消");
        return Ok(());
    }

    let scan_dirs = [
        util::home().join(".config/hypr/scripts"),
        util::home().join(".config/ml4w/scripts"),
        util::home().join(".config/rofi"),
        util::home().join(".config/waybar"),
        util::home().join(".config/hypr/conf/keybindings"),
    ];

    if let Some(name) = target {
        for dir in &scan_dirs {
            let bak_dir = dir.join(util::BACKUP_DIR);
            let bak = bak_dir.join(name);
            if bak.exists() {
                let target_path = dir.join(name);
                if dry_run {
                    eprintln!("  📝 将恢复: {}", target_path.display());
                } else {
                    fs::copy(&bak, &target_path).ok();
                    eprintln!("  ↩️  已恢复: {}", target_path.display());
                }
                return Ok(());
            }
        }
        eprintln!("  ❌ 未找到备份: {name}");
    } else {
        let mut restored = 0;
        for dir in &scan_dirs {
            let bak_dir = dir.join(util::BACKUP_DIR);
            if !bak_dir.is_dir() {
                continue;
            }
            for entry in fs::read_dir(&bak_dir).ok().into_iter().flatten() {
                let entry = entry.ok().filter(|e| e.path().is_file());
                if let Some(bak) = entry {
                    let target_path = dir.join(bak.file_name());
                    if dry_run {
                        eprintln!("  📝 将恢复: {}", target_path.display());
                    } else {
                        fs::copy(bak.path(), &target_path).ok();
                        eprintln!("  ↩️  已恢复: {}", target_path.display());
                    }
                    restored += 1;
                }
            }
        }
        if restored == 0 {
            eprintln!("  没有找到备份。");
        }
        if !dry_run {
            eprintln!("\n✅ 完成，恢复 {restored} 个文件。");
        }
    }
    Ok(())
}

fn cmd_init(files: &[PatchFile], dry_run: bool) -> Result<()> {
    eprintln!("━━━ 初始化（备份 + PROTECTED）━━━\n");
    for pf in files {
        let path = util::expand(&pf.path);
        if !path.exists() {
            continue;
        }
        if dry_run {
            if pf.backup {
                eprintln!("  📦 将备份: {}", pf.path);
            }
            if pf.protect {
                eprintln!("  🛡️  将创建 PROTECTED");
            }
        } else {
            if pf.backup {
                util::do_backup(&path);
            }
            if pf.protect {
                util::do_protect(&path);
            }
        }
    }
    if !dry_run {
        eprintln!("\n✅ 初始化完成");
    }
    Ok(())
}

fn cmd_update_patches(validate: bool) -> Result<()> {
    eprintln!("━━━ 更新翻译定义... ━━━\n");
    let files = fetch_and_cache(true)?;
    eprintln!("✅ 已下载 {} 个文件的翻译定义", files.len());
    eprintln!("   缓存: {}", util::cache_path().display());
    if validate {
        eprintln!();
        validate_patches(&files)?;
    }
    Ok(())
}

// ─── TOML 定义生成（开发者用） ──────────────────────────────

#[derive(serde::Deserialize)]
struct DefFile {
    file: Vec<DefEntry>,
}

#[derive(serde::Deserialize)]
struct DefEntry {
    path: String,
    source: String,
    backup: bool,
    protect: bool,
    patch: Vec<DefPatch>,
}

#[derive(serde::Deserialize)]
struct DefPatch {
    old: String,
    new: String,
}

fn cmd_generate() -> Result<()> {
    eprintln!("━━━ 生成 patches.json ━━━\n");
    let def_dir = definitions_dir();
    if !def_dir.is_dir() {
        anyhow::bail!("未找到定义目录: {}", def_dir.display());
    }

    let mut all_files: Vec<PatchFile> = Vec::new();
    let entries = fs::read_dir(&def_dir).context("读取定义目录失败")?;
    for entry in entries {
        let entry = entry.context("读取目录条目失败")?;
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "toml") {
            continue;
        }
        let content = fs::read_to_string(&path).context("读取定义文件失败")?;
        let def: DefFile = toml::from_str(&content)
            .with_context(|| format!("解析 {} 失败", path.display()))?;
        for fe in &def.file {
            let patches: Vec<Patch> = fe
                .patch
                .iter()
                .map(|p| Patch {
                    old: p.old.clone(),
                    new: p.new.clone(),
                })
                .collect();
            all_files.push(PatchFile {
                path: fe.path.clone(),
                source: fe.source.clone(),
                backup: fe.backup,
                protect: fe.protect,
                patches,
            });
        }
        eprintln!(
            "  ✅ {} ({} 文件)",
            path.file_name()
                .map_or("unknown", |n| n.to_str().unwrap_or("unknown")),
            def.file.len()
        );
    }
    let json = serde_json::to_string_pretty(&all_files).context("序列化失败")?;
    let out_path = patches_output_path();
    fs::write(&out_path, &json).with_context(|| format!("写入 {} 失败", out_path.display()))?;
    let total: usize = all_files.iter().map(|f| f.patches.len()).sum();
    eprintln!("\n✅ 已生成 — {} 文件, {} 翻译", all_files.len(), total);
    Ok(())
}

/// 返回定义目录路径：优先 `$ML4W_ZH_ROOT/patches/definitions`，否则从 `CARGO_MANIFEST_DIR` 推导
fn definitions_dir() -> PathBuf {
    if let Ok(root) = std::env::var("ML4W_ZH_ROOT") {
        PathBuf::from(root).join("patches/definitions")
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("patches/definitions")
    }
}

/// 返回输出路径：优先 `$ML4W_ZH_ROOT/patches/patches.json`，否则从 `CARGO_MANIFEST_DIR` 推导
fn patches_output_path() -> PathBuf {
    if let Ok(root) = std::env::var("ML4W_ZH_ROOT") {
        PathBuf::from(root).join("patches/patches.json")
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("patches/patches.json")
    }
}
