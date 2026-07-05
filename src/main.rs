// ml4w-zh v0.3 — ML4W dotfiles 汉化工具
//
// 流程:
//   1. 从 GitHub 拉 patches.json（翻译定义）
//   2. 对每个文件，从 ml4w 官方源下载原版对比
//   3. 本地文件 = 官方原版 → 可以安全汉化
//   4. 本地文件 ≠ 官方原版 → ml4w 更新了或用户改过 → 警告，不盲改
//   5. 改前必备份，scripts/ 下自动加 PROTECTED
//
// 依赖: serde_json (JSON 解析), curl (系统自带)

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const VERSION: &str = "0.3.0";
const PATCHES_URL: &str = "https://raw.githubusercontent.com/danxcheng/ml4w-zh/main/patches/patches.json";
const ML4W_RAW: &str = "https://raw.githubusercontent.com/mylinuxforwork/dotfiles";
const BACKUP_DIR: &str = ".ml4w-zh-backups";

// ─── 数据结构 ────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct PatchFile {
    path: String,
    source: String,
    backup: bool,
    protect: bool,
    patches: Vec<Patch>,
}

#[derive(serde::Deserialize)]
struct Patch {
    old: String,
    new: String,
}

// ─── 工具函数 ────────────────────────────────────────────

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/home/danxcheng".into()))
}

fn expand(path: &str) -> PathBuf {
    let p = path.replace('~', home().to_str().unwrap_or(""));
    if p.starts_with('/') { PathBuf::from(p) } else { home().join(&p) }
}

fn kurwa(url: &str) -> Result<String, String> {
    let out = Command::new("curl").args(["-sL", url]).output()
        .map_err(|e| format!("curl 失败: {}", e))?;
    if !out.status.success() {
        return Err(format!("HTTP {}", out.status.code().unwrap_or(0)));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("编码: {}", e))
}

fn confirm(msg: &str) -> bool {
    eprint!("{} [y/N] ", msg);
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).ok();
    matches!(s.trim().to_lowercase().as_str(), "y" | "yes")
}

fn backup_dir_for(path: &Path) -> PathBuf {
    path.parent().map(|p| p.join(BACKUP_DIR)).unwrap_or_else(|| PathBuf::from(BACKUP_DIR))
}

fn do_backup(file: &Path) {
    let dir = backup_dir_for(file);
    fs::create_dir_all(&dir).ok();
    let bak = dir.join(file.file_name().unwrap());
    if bak.exists() { return; }
    fs::copy(file, &bak).ok();
    eprintln!("  📦 已备份: {}", file.display());
}

fn do_protect(file: &Path) {
    let dir = file.parent().unwrap();
    let pf = dir.join("PROTECTED");
    if pf.exists() { return; }
    fs::write(&pf, "").ok();
    eprintln!("  🛡️  已保护: {}/", dir.file_name().unwrap_or_default().to_string_lossy());
}


fn diff_text(a: &str, b: &str) -> String {
    let mut lines = Vec::new();
    for (i, (la, lb)) in a.lines().zip(b.lines()).enumerate() {
        if la != lb {
            lines.push(format!("  L{}: -{}+{}", i+1, la, lb));
        }
    }
    let alines: Vec<&str> = a.lines().collect();
    let blines: Vec<&str> = b.lines().collect();
    if alines.len() != blines.len() {
        lines.push(format!("  行数差异: {} vs {}", alines.len(), blines.len()));
    }
    lines.join("\n")
}

// ─── 核心 ────────────────────────────────────────────────

fn fetch_patches() -> Result<Vec<PatchFile>, String> {
    let json = kurwa(PATCHES_URL)?;
    serde_json::from_str(&json).map_err(|e| format!("patches.json 解析失败: {}", e))
}

/// 下载官方文件，对比本地，返回状态
fn verify_against_source(pf: &PatchFile, local: &Path) -> Result<(String, String), String> {
    let url = format!("{}/{}/{}", ML4W_RAW, pf.source, pf.path);
    let official = kurwa(&url)?;
    let local_content = fs::read_to_string(local).map_err(|e| format!("读取本地文件失败: {}", e))?;

    if local_content == official {
        Ok(("match".into(), official))
    } else if local_content.contains(&pf.patches.first().map(|p| &p.new[..]).unwrap_or("")) {
        // 本地已经是汉化版 → 跳过
        Ok(("patched".into(), official))
    } else {
        // 不匹配 → 官方有更新
        Ok(("mismatch".into(), official))
    }
}

fn apply_one(pf: &PatchFile, dry_run: bool) -> Result<usize, String> {
    let path = expand(&pf.path);
    if !path.exists() { return Err("文件不存在".into()); }

    // 对比官方源
    let (status, official) = verify_against_source(pf, &path)?;
    match status.as_str() {
        "match" => {} // 可以汉化
        "patched" => {
            eprintln!("  ⏭️  已汉化: {}", pf.path);
            return Ok(0);
        }
        "mismatch" => {
            let local = fs::read_to_string(&path).unwrap_or_default();
            eprintln!("  ⚠️  {} 与官方版本不一致！", pf.path);
            eprintln!("     可能 ml4w 已更新，跳过此文件。");
            if dry_run {
                let d = diff_text(&official, &local);
                if !d.is_empty() { eprintln!("     差异:\n{}", d); }
            }
            return Ok(0);
        }
        _ => {}
    }

    // 备份 + PROTECTED
    if !dry_run {
        if pf.backup { do_backup(&path); }
        if pf.protect { do_protect(&path); }
    }

    // 逐条替换
    let content = fs::read_to_string(&path).map_err(|e| format!("读取失败: {}", e))?;
    let mut result = content.clone();
    let mut count = 0;
    for p in &pf.patches {
        if result.contains(&p.new) { continue; }
        result = result.replace(&p.old, &p.new);
        count += 1;
    }

    if count > 0 {
        if dry_run {
            eprintln!("  📝 将汉化: {} ({}处)", pf.path, count);
        } else {
            fs::write(&path, &result).map_err(|e| format!("写入失败: {}", e))?;
            eprintln!("  ✅ 已汉化: {} ({}处)", pf.path, count);
        }
    }
    Ok(count)
}

// ─── 命令 ────────────────────────────────────────────────

fn cmd_apply(files: &[PatchFile], dry_run: bool, skip_confirm: bool) {
    eprintln!("━━━ ML4W 中文汉化 ━━━\n");
    if !skip_confirm && !dry_run && !confirm("将备份并汉化，是否继续？") {
        eprintln!("已取消"); return;
    }

    let mut total = 0; let skipped = 0; let mut errors = 0;
    for pf in files {
        match apply_one(pf, dry_run) {
            Ok(n) => total += n,
            Err(e) => { eprintln!("  ❌ {}: {}", pf.path, e); errors += 1; }
        }
    }
    if dry_run {
        eprintln!("\n预览结束，将修改 {} 处。去掉 --dry-run 执行。", total);
    } else {
        eprintln!("\n✅ 完成！{} 处翻译已应用 (跳过 {}，错误 {})", total, skipped, errors);
        eprintln!("   按 Super + Ctrl + R 重载 Hyprland 配置\n");
    }
}

fn cmd_check(files: &[PatchFile]) {
    eprintln!("━━━ 翻译状态检查 ━━━\n");
    let mut ok = 0; let mut outdated = 0; let mut missing = 0; let mut updated = 0;

    for pf in files {
        let path = expand(&pf.path);
        if !path.exists() { eprintln!("  ❌ 文件缺失: {}", pf.path); missing += 1; continue; }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c, Err(_) => { eprintln!("  ❌ 无法读取: {}", pf.path); missing += 1; continue; }
        };

        // 检查是否已汉化
        let first_new = &pf.patches.first().map(|p| &p.new[..]).unwrap_or("");
        let is_zh = !first_new.is_empty() && content.contains(first_new);

        // 从官方源验证
        let url = format!("{}/{}/{}", ML4W_RAW, pf.source, pf.path);
        let official = match kurwa(&url) {
            Ok(s) => s, Err(_) => { eprintln!("  ⚠️  无法连接 ml4w 源: {}", pf.path); outdated += 1; continue; }
        };

        if content == official && !is_zh {
            eprintln!("  📝 未汉化，官方一致: {}", pf.path);
            outdated += 1;
        } else if content == official && is_zh {
            eprintln!("  ✅ 已汉化: {}", pf.path);
            ok += 1;
        } else if content != official && is_zh {
            eprintln!("  🔄 已汉化但官方已更新: {}", pf.path);
            eprintln!("     （运行 apply 重新汉化）");
            updated += 1;
        } else {
            eprintln!("  ⚠️  本地与官方不一致（用户自定义？）: {}", pf.path);
            outdated += 1;
        }
    }
    eprintln!("\n总计 {} 个文件", files.len());
    eprintln!("  ✅ 已汉化且最新: {}", ok);
    eprintln!("  🔄 已汉化但需更新: {}", updated);
    eprintln!("  📝 未汉化: {}", outdated);
    eprintln!("  ❌ 缺失:   {}", missing);
}

fn cmd_revert(_files: &[PatchFile], dry_run: bool) {
    eprintln!("━━━ 恢复英文原版 ━━━\n");
    if !dry_run && !confirm("将从备份恢复，是否继续？") { eprintln!("已取消"); return; }

    // 扫描所有 .ml4w-zh-backups 目录
    let scan_dirs = [
        home().join(".config/hypr/scripts"),
        home().join(".config/ml4w/scripts"),
        home().join(".config/rofi"),
        home().join(".config/waybar"),
        home().join(".config/hypr/conf/keybindings"),
    ];

    let mut restored = 0;
    for dir in &scan_dirs {
        let bak_dir = dir.join(BACKUP_DIR);
        if !bak_dir.is_dir() { continue; }
        for entry in fs::read_dir(&bak_dir).ok().into_iter().flatten() {
            let entry = entry.ok().filter(|e| e.path().is_file());
            if let Some(bak) = entry {
                let target = dir.join(bak.file_name());
                if dry_run {
                    eprintln!("  📝 将恢复: {}", target.display());
                } else {
                    fs::copy(bak.path(), &target).ok();
                    eprintln!("  ↩️  已恢复: {}", target.display());
                }
                restored += 1;
            }
        }
    }
    if restored == 0 { eprintln!("  没有找到备份。"); }
    if !dry_run { eprintln!("\n✅ 完成，恢复 {} 个文件。", restored); }
}

fn cmd_init(files: &[PatchFile], dry_run: bool) {
    eprintln!("━━━ 初始化（备份 + PROTECTED）━━━\n");
    for pf in files {
        let path = expand(&pf.path);
        if !path.exists() { continue; }
        if dry_run {
            if pf.backup { eprintln!("  📦 将备份: {}", pf.path); }
            if pf.protect { eprintln!("  🛡️  将创建 PROTECTED"); }
        } else {
            if pf.backup { do_backup(&path); }
            if pf.protect { do_protect(&path); }
        }
    }
    if !dry_run { eprintln!("\n✅ 初始化完成"); }
}

// ─── CLI ─────────────────────────────────────────────────

fn help() {
    eprintln!("\
ml4w-zh v{} — ML4W dotfiles 汉化工具

用法: ml4w-zh <命令> [选项]

命令:
  apply      从 ml4w 官方源校验后汉化（默认）
  check      检查汉化状态，对比官方源
  revert     从备份恢复英文（离线可用）
  init       仅备份 + PROTECTED

选项:
  --dry-run  预览，不修改
  -y         跳过确认
  -h         帮助

原理:
  1. 拉 patches.json（翻译定义）
  2. 对每个文件，从 ml4w GitHub 下载原版对比
  3. 本地 = 官方 → 安全汉化
  4. 本地 ≠ 官方 → 警告，不盲改（ml4w 可能已更新）
  5. revert 离线工作，扫描备份目录

项目: https://github.com/danxcheng/ml4w-zh
", VERSION);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("apply");
    let dry_run = args.contains(&"--dry-run".to_string());
    let skip_confirm = args.contains(&"-y".to_string());

    if args.contains(&"-h".to_string()) || cmd == "help" { help(); return; }
    if args.contains(&"-V".to_string()) { eprintln!("ml4w-zh v{}", VERSION); return; }

    // revert 离线工作，不需要下载 patches
    if cmd == "revert" {
        cmd_revert(&[], dry_run);
        return;
    }

    // 下载翻译定义
    eprintln!("ml4w-zh v{} — 正在获取翻译定义...", VERSION);
    let files = match fetch_patches() {
        Ok(f) => f,
        Err(e) => { eprintln!("❌ 获取 patches.json 失败: {}", e); std::process::exit(1); }
    };
    eprintln!("   已加载 {} 个文件的翻译定义\n", files.len());

    match cmd {
        "apply" | "install" => cmd_apply(&files, dry_run, skip_confirm),
        "check" => cmd_check(&files),
        "init" => cmd_init(&files, dry_run),
        _ => { eprintln!("未知命令: {}", cmd); help(); }
    }
}
