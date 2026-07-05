// ml4w-zh v0.4 — ML4W dotfiles 汉化工具
//
// 流程:
//   1. 从 GitHub 或本地缓存拉 patches.json
//   2. 校验 patches.json 格式（old 字段必须存在于官方源）
//   3. 对每个文件，从 ml4w 官方源下载原版对比
//   4. 本地 == 官方 → 安全汉化
//   5. 本地 ≠ 官方 → 警告跳过
//   6. 改前备份 + 自动 PROTECTED

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const VERSION: &str = "0.4.0";
const PATCHES_URL: &str = "https://raw.githubusercontent.com/danxcheng/ml4w-zh/main/patches/patches.json";
const ML4W_RAW: &str = "https://raw.githubusercontent.com/mylinuxforwork/dotfiles";
const BACKUP_DIR: &str = ".ml4w-zh-backups";
const CACHE_FILE: &str = "patches.json";
// 缓存目录: ~/.cache/ml4w-zh/

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct PatchFile {
    path: String,
    source: String,
    backup: bool,
    protect: bool,
    patches: Vec<Patch>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
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

fn cache_dir() -> PathBuf {
    home().join(".cache/ml4w-zh")
}

fn cache_path() -> PathBuf {
    cache_dir().join(CACHE_FILE)
}

/// 下载文件，支持进度显示（对大文件用 `-#`，小文件用 `-s`）
fn download(url: &str, show_progress: bool) -> Result<String, String> {
    let mut cmd = Command::new("curl");
    if show_progress {
        cmd.args(["-L", url]);  // 自带进度条
    } else {
        cmd.args(["-sL", url]); // 静默
    }
    let out = cmd.output().map_err(|e| format!("curl 失败: {}", e))?;
    if !out.status.success() {
        return Err(format!("HTTP {}", out.status.code().unwrap_or(0)));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("编码错误: {}", e))
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
    if bak.exists() { eprintln!("  ⏭️  备份已存在: {}", file.display()); return; }
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

fn diff_text(a: &str, b: &str) -> Vec<String> {
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
    lines
}

// ─── patches.json 管理 ──────────────────────────────────

/// 从 GitHub 下载 patches.json，写入缓存
fn fetch_and_cache(show_progress: bool) -> Result<Vec<PatchFile>, String> {
    let json = download(PATCHES_URL, show_progress)?;
    fs::create_dir_all(&cache_dir()).ok();
    fs::write(&cache_path(), &json).ok();
    parse_patches(&json)
}

/// 从本地缓存读取
fn load_cached() -> Result<Vec<PatchFile>, String> {
    let path = cache_path();
    if !path.exists() {
        return Err("本地缓存不存在，先运行 `ml4w-zh --update-patches` 或 `ml4w-zh apply` 下载。".into());
    }
    let json = fs::read_to_string(&path).map_err(|e| format!("读取缓存失败: {}", e))?;
    parse_patches(&json)
}

fn parse_patches(json: &str) -> Result<Vec<PatchFile>, String> {
    serde_json::from_str(json).map_err(|e| format!("patches.json 解析失败: {}", e))
}

// ─── 校验 ────────────────────────────────────────────────

/// 校验 patches.json：
///   对每个文件的每条 patch，检查 old 字符串是否存在于官方源中
fn validate_patches(files: &[PatchFile]) -> Result<(), String> {
    eprintln!("━━━ 校验翻译定义 ━━━\n");
    let mut errors = 0;
    for pf in files {
        let url = format!("{}/{}/{}", ML4W_RAW, pf.source, pf.path);
        let official = match download(&url, false) {
            Ok(s) => s,
            Err(e) => { eprintln!("  ⚠️  无法获取官方源 ({}): {}", pf.path, e); errors += 1; continue; }
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
        return Err(format!("{} 条翻译定义校验失败，请检查 patches.json", errors));
    }
    eprintln!("\n✅ 全部校验通过");
    Ok(())
}

// ─── 核心 ────────────────────────────────────────────────

fn verify_against_source(pf: &PatchFile, local: &Path) -> Result<(String, String), String> {
    let url = format!("{}/{}/{}", ML4W_RAW, pf.source, pf.path);
    let official = download(&url, false)?;
    let local_content = fs::read_to_string(local).map_err(|e| format!("读取失败: {}", e))?;

    if local_content == official {
        Ok(("match".into(), official))
    } else if local_content.contains(&pf.patches.first().map(|p| &p.new[..]).unwrap_or("")) {
        Ok(("patched".into(), official))
    } else {
        Ok(("mismatch".into(), official))
    }
}

fn apply_one(pf: &PatchFile, dry_run: bool) -> Result<usize, String> {
    let path = expand(&pf.path);
    if !path.exists() { return Err("文件不存在".into()); }

    eprint!("  → {} ... ", pf.path);
    let (status, _official) = verify_against_source(pf, &path)?;
    match status.as_str() {
        "match" => {}
        "patched" => { eprintln!("⏭️  已汉化"); return Ok(0); }
        "mismatch" => {
            eprintln!("⚠️  与官方不一致，跳过");
            if dry_run {
                let local = fs::read_to_string(&path).unwrap_or_default();
                let d = diff_text(&_official, &local);
                for l in &d { eprintln!("{}", l); }
            }
            return Ok(0);
        }
        _ => {}
    }

    if !dry_run {
        if pf.backup { do_backup(&path); }
        if pf.protect { do_protect(&path); }
    }

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
            eprintln!("📝 将汉化 ({}处)", count);
        } else {
            fs::write(&path, &result).map_err(|e| format!("写入失败: {}", e))?;
            eprintln!("✅ 已汉化 ({}处)", count);
        }
    } else {
        eprintln!("✅ 无需修改");
    }
    Ok(count)
}

// ─── 子命令 ──────────────────────────────────────────────

fn cmd_apply(files: &[PatchFile], dry_run: bool, skip_confirm: bool) {
    eprintln!("━━━ ML4W 中文汉化 ━━━\n");
    if !skip_confirm && !dry_run && !confirm("将备份并汉化，是否继续？") {
        eprintln!("已取消"); return;
    }
    let mut total = 0; let mut skipped = 0; let mut errors = 0;
    for pf in files {
        match apply_one(pf, dry_run) {
            Ok(n) => { if n == 0 && !dry_run { skipped += 1; } total += n; }
            Err(e) => { eprintln!("  ❌ {}: {}", pf.path, e); errors += 1; }
        }
    }
    if dry_run {
        eprintln!("\n预览结束，将修改 {} 处。去掉 --dry-run 执行。", total);
    } else {
        eprintln!("\n✅ 完成！{} 处翻译 ({} 跳过, {} 错误)", total, skipped, errors);
        eprintln!("   按 Super + Ctrl + R 重载 Hyprland 配置\n");
    }
}

fn cmd_check(files: &[PatchFile]) {
    eprintln!("━━━ 翻译状态检查 ━━━\n");
    let mut ok = 0; let mut outdated = 0; let mut missing = 0; let mut updated = 0;
    for pf in files {
        let path = expand(&pf.path);
        if !path.exists() { eprintln!("  ❌ 缺失: {}", pf.path); missing += 1; continue; }
        let content = match fs::read_to_string(&path) {
            Ok(c) => c, Err(_) => { eprintln!("  ❌ 无法读取: {}", pf.path); missing += 1; continue; }
        };
        let first_new = &pf.patches.first().map(|p| &p.new[..]).unwrap_or("");
        let is_zh = !first_new.is_empty() && content.contains(first_new);

        let url = format!("{}/{}/{}", ML4W_RAW, pf.source, pf.path);
        let official = match download(&url, false) {
            Ok(s) => s, Err(_) => { eprintln!("  ⚠️  无法连接 ml4w 源: {}", pf.path); outdated += 1; continue; }
        };

        if content == official && !is_zh { eprintln!("  📝 未汉化: {}", pf.path); outdated += 1; }
        else if content == official && is_zh { eprintln!("  ✅ 已汉化: {}", pf.path); ok += 1; }
        else if content != official && is_zh { eprintln!("  🔄 需更新: {}", pf.path); updated += 1; }
        else { eprintln!("  ⚠️  自定义: {}", pf.path); outdated += 1; }
    }
    eprintln!("\n总计 {} 个文件", files.len());
    eprintln!("  ✅ 已汉化: {}", ok);
    eprintln!("  🔄 需更新: {}", updated);
    eprintln!("  📝 未汉化: {}", outdated);
    eprintln!("  ❌ 缺失:   {}", missing);
}

fn cmd_revert(_files: &[PatchFile], dry_run: bool, target: Option<&str>) {
    eprintln!("━━━ 恢复英文原版 ━━━\n");
    if !dry_run && !confirm("将从备份恢复，是否继续？") { eprintln!("已取消"); return; }

    if let Some(name) = target {
        let scan_dirs = [
            home().join(".config/hypr/scripts"), home().join(".config/ml4w/scripts"),
            home().join(".config/rofi"), home().join(".config/waybar"),
            home().join(".config/hypr/conf/keybindings"),
        ];
        for dir in &scan_dirs {
            let bak_dir = dir.join(BACKUP_DIR);
            let bak = bak_dir.join(name);
            if bak.exists() {
                let target = dir.join(name);
                if dry_run { eprintln!("  📝 将恢复: {}", target.display()); }
                else { fs::copy(&bak, &target).ok(); eprintln!("  ↩️  已恢复: {}", target.display()); }
                return;
            }
        }
        eprintln!("  ❌ 未找到备份: {}", name);
    } else {
        let scan_dirs = [
            home().join(".config/hypr/scripts"), home().join(".config/ml4w/scripts"),
            home().join(".config/rofi"), home().join(".config/waybar"),
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
                    if dry_run { eprintln!("  📝 将恢复: {}", target.display()); }
                    else { fs::copy(bak.path(), &target).ok(); eprintln!("  ↩️  已恢复: {}", target.display()); }
                    restored += 1;
                }
            }
        }
        if restored == 0 { eprintln!("  没有找到备份。"); }
        if !dry_run { eprintln!("\n✅ 完成，恢复 {} 个文件。", restored); }
    }
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

fn cmd_update_patches(validate: bool) {
    eprintln!("ml4w-zh v{} — 更新翻译定义...\n", VERSION);
    match fetch_and_cache(true) {
        Ok(files) => {
            eprintln!("✅ 已下载 {} 个文件的翻译定义", files.len());
            eprintln!("   缓存: {}", cache_path().display());
            if validate {
                eprintln!();
                if let Err(e) = validate_patches(&files) {
                    eprintln!("\n❌ 校验失败: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => { eprintln!("❌ 下载失败: {}", e); std::process::exit(1); }
    }
}

// ─── TOML 定义生成 ────────────────────────────────────────

#[derive(serde::Deserialize)]
struct DefFile { file: Vec<DefEntry> }

#[derive(serde::Deserialize)]
struct DefEntry {
    path: String, source: String, backup: bool, protect: bool, patch: Vec<DefPatch>,
}

#[derive(serde::Deserialize)]
struct DefPatch { old: String, new: String }

fn cmd_generate() {
    eprintln!("━━━ 生成 patches.json ━━━\n");
    let def_dir = PathBuf::from("/run/host/home/danxcheng/项目/ml4w-zh/patches/definitions");
    if !def_dir.is_dir() { eprintln!("❌ 未找到定义目录: {}", def_dir.display()); std::process::exit(1); }

    let mut all_files: Vec<PatchFile> = Vec::new();
    let mut entries = fs::read_dir(&def_dir).unwrap();
    while let Some(entry) = entries.next() {
        let entry = entry.unwrap();
        let path = entry.path();
        if !path.extension().map_or(false, |e| e == "toml") { continue; }
        let content = fs::read_to_string(&path).unwrap();
        let def: DefFile = match toml::from_str(&content) {
            Ok(d) => d,
            Err(e) => { eprintln!("❌ 解析 {} 失败: {}", path.display(), e); std::process::exit(1); }
        };
        for fe in &def.file {
            let patches: Vec<Patch> = fe.patch.iter().map(|p| Patch { old: p.old.clone(), new: p.new.clone() }).collect();
            all_files.push(PatchFile { path: fe.path.clone(), source: fe.source.clone(), backup: fe.backup, protect: fe.protect, patches });
        }
        eprintln!("  ✅ {} ({} 文件)", path.file_name().unwrap().to_string_lossy(), def.file.len());
    }
    let json = serde_json::to_string_pretty(&all_files).unwrap();
    let out_path = PathBuf::from("/run/host/home/danxcheng/项目/ml4w-zh/patches/patches.json");
    fs::write(&out_path, &json).unwrap();
    let total = all_files.iter().map(|f| f.patches.len()).sum::<usize>();
    eprintln!("\n✅ 已生成 — {} 文件, {} 翻译", all_files.len(), total);
}

// ─── CLI ─────────────────────────────────────────────────

fn help() {
    eprintln!("\
ml4w-zh v{} — ML4W dotfiles 汉化工具

用法: ml4w-zh <命令> [选项]

命令:
  apply             从 ml4w 官方源校验后汉化（默认）
  check             检查汉化状态，对比官方源
  revert [文件名]    恢复全部或指定文件（离线）
  init              仅备份 + PROTECTED
  update-patches    下载最新翻译定义到本地缓存
  generate          从 TOML 定义生成 patches.json（开发者用）

选项:
  --dry-run         预览，不修改
  --offline         使用本地缓存，不下载
  --validate        下载后校验翻译定义（与 update-patches 联用）
  -y                跳过确认
  -h                帮助

缓存: {}
项目: https://github.com/danxcheng/ml4w-zh
", VERSION, cache_path().display());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("apply");
    let dry_run = args.contains(&"--dry-run".to_string());
    let skip_confirm = args.contains(&"-y".to_string());
    let offline = args.contains(&"--offline".to_string());
    let do_validate = args.contains(&"--validate".to_string());

    if args.contains(&"-h".to_string()) || cmd == "help" { help(); return; }
    if args.contains(&"-V".to_string()) { eprintln!("ml4w-zh v{}", VERSION); return; }

    if cmd == "revert" {
        let target = args.get(2).map(|s| s.as_str());
        cmd_revert(&[], dry_run, target);
        return;
    }
    if cmd == "update-patches" { cmd_update_patches(do_validate); return; }
    if cmd == "generate" { cmd_generate(); return; }

    let files = if offline {
        eprintln!("ml4w-zh v{} — 使用本地缓存\n", VERSION);
        load_cached()
    } else {
        eprintln!("ml4w-zh v{} — 正在获取翻译定义...\n", VERSION);
        fetch_and_cache(false)
    };

    let files = match files {
        Ok(f) => f,
        Err(e) => { eprintln!("❌ {}", e); std::process::exit(1); }
    };

    match cmd {
        "apply" | "install" => cmd_apply(&files, dry_run, skip_confirm),
        "check" => cmd_check(&files),
        "init" => cmd_init(&files, dry_run),
        _ => { eprintln!("未知命令: {}", cmd); help(); }
    }
}
