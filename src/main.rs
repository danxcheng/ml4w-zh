// ml4w-zh — ML4W dotfiles 汉化工具
//
// 设计原则：
//   1. 零依赖（仅依赖 curl，所有 Linux 都有）
//   2. 翻译定义从 GitHub 拉取，二进制本身不捆绑
//   3. 备份优先，改前必备份
//   4. 不搞自动构建，提醒用户自己判断

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// ─── 常量 ────────────────────────────────────────────────
const VERSION: &str = "0.2.0";
const PATCHES_URL: &str = "https://raw.githubusercontent.com/danxcheng/ml4w-zh/main/patches/patches.json";
const BACKUP_DIR_NAME: &str = ".ml4w-zh-backups";

// ─── 补丁定义（从 JSON 反序列化）────────────────────────
#[derive(Debug)]
struct PatchFile {
    path: String,
    backup: bool,
    protect: bool,
    patches: Vec<Patch>,
}

#[derive(Debug)]
struct Patch {
    old: String,
    new: String,
}

// ─── 简易 JSON 解析器（零依赖，够用就好）────────────────
fn parse_patches_json(text: &str) -> Result<Vec<PatchFile>, String> {
    let mut files = Vec::new();
    let text = text.trim();

    // 最外层必须是数组 [...]
    if !text.starts_with('[') || !text.ends_with(']') {
        return Err("patches.json 格式错误：需要 JSON 数组".into());
    }
    let inner = &text[1..text.len()-1].trim();
    if inner.is_empty() {
        return Ok(files);
    }

    // 逐对象解析（不依赖 serde，手拆大括号）
    let mut depth = 0;
    let mut start = 0;
    for (i, ch) in inner.char_indices() {
        match ch {
            '{' if depth == 0 => start = i,
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            '}' if depth == 0 => {
                let obj = &inner[start..=i];
                if let Ok(pf) = parse_one_file(obj) {
                    files.push(pf);
                }
            }
            _ => {}
        }
    }
    Ok(files)
}

fn parse_one_file(s: &str) -> Result<PatchFile, String> {
    let mut path = String::new();
    let mut backup = false;
    let mut protect = false;
    let mut patches = Vec::new();

    let inner = s.trim();
    let inner = inner.strip_prefix('{').and_then(|s| s.strip_suffix('}')).unwrap_or("");
    let inner = inner.trim();

    // 拆出顶层 key: value 对
    let mut depth = 0;
    let mut in_str = false;
    let mut key_start = None;
    let mut val_start = None;

    for (i, ch) in inner.char_indices() {
        if ch == '"' {
            in_str = !in_str;
            continue;
        }
        if in_str { continue; }

        match ch {
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            ':' if depth == 0 && key_start.is_none() => {
                key_start = Some(inner[..i].rfind(|c: char| c == ',' || c == '{')
                    .map(|p| p + 1).unwrap_or(0));
                val_start = Some(i + 1);
            }
            ',' if depth == 0 && val_start.is_some() => {
                let k = extract_string(&inner[key_start.unwrap()..val_start.unwrap()]);
                let v = inner[val_start.unwrap()..i].trim().trim_end_matches(',');
                set_field(&mut path, &mut backup, &mut protect, &k, v);
                key_start = None;
                val_start = None;
            }
            _ => {}
        }
    }
    // 最后一个字段
    if let (Some(ks), Some(vs)) = (key_start, val_start) {
        let k = extract_string(&inner[ks..vs]);
        let v = inner[vs..].trim().trim_end_matches('}').trim().trim_end_matches(',');
        set_field(&mut path, &mut backup, &mut protect, &k, v);
    }

    // 解析 patches 数组
    if let Some(arr_start) = inner.find("\"patches\"") {
        let after_colon = &inner[arr_start..];
        if let Some(arr_begin) = after_colon.find('[') {
            let arr_begin_abs = arr_start + arr_begin;
            if let Some(arr_end) = find_matching_bracket(&inner[arr_begin_abs..]) {
                let arr_body = &inner[arr_begin_abs + 1..arr_begin_abs + arr_end];
                parse_patch_array(arr_body, &mut patches);
            }
        }
    }

    if path.is_empty() {
        return Err("缺少 path 字段".into());
    }
    Ok(PatchFile { path, backup, protect, patches })
}

fn extract_string(s: &str) -> String {
    let s = s.trim();
    if let Some(start) = s.find('"') {
        if let Some(end) = s[start+1..].find('"') {
            return s[start+1..start+1+end].to_string();
        }
    }
    s.trim().trim_end_matches(':').trim().to_string()
}

fn set_field(path: &mut String, backup: &mut bool, protect: &mut bool,
              key: &str, val: &str) {
    match key {
        "path" => *path = val.trim_matches('"').to_string(),
        "backup" => *backup = val.trim() == "true",
        "protect" => *protect = val.trim() == "true",
        _ => {}
    }
}

fn find_matching_bracket(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 { return Some(i); }
            }
            _ => {}
        }
    }
    None
}

fn parse_patch_array(s: &str, out: &mut Vec<Patch>) {
    let mut depth = 0;
    let mut start = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '{' if depth == 0 => start = i,
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            '}' if depth == 0 => {
                let obj = &s[start..=i];
                let old = extract_json_str(obj, "old");
                let new = extract_json_str(obj, "new");
                if !old.is_empty() || !new.is_empty() {
                    out.push(Patch { old, new });
                }
            }
            _ => {}
        }
    }
}

fn extract_json_str(s: &str, key: &str) -> String {
    if let Some(pos) = s.find(&format!("\"{}\"", key)) {
        let after = &s[pos + key.len() + 2..];
        if let Some(col) = after.find(':') {
            let after_colon = after[col + 1..].trim();
            if after_colon.starts_with('"') {
                if let Some(end) = after_colon[1..].find('"') {
                    return after_colon[1..1 + end].to_string();
                }
            }
        }
    }
    String::new()
}

// ─── 路径展开 ────────────────────────────────────────────
fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/home/danxcheng".into()))
}

fn expand(path: &str) -> PathBuf {
    let p = path.replace("~", home().to_str().unwrap_or(""));
    if p.starts_with('/') { PathBuf::from(p) }
    else { home().join(&p) }
}

fn backup_dir_for(path: &Path) -> PathBuf {
    path.parent().unwrap_or(Path::new(".")).join(BACKUP_DIR_NAME)
}

// ─── 核心功能 ────────────────────────────────────────────
fn download_patches(url: &str) -> Result<String, String> {
    let output = Command::new("curl")
        .args(["-sL", url])
        .output()
        .map_err(|e| format!("无法执行 curl: {}", e))?;
    if !output.status.success() {
        return Err(format!("curl 下载失败 (exit: {:?})", output.status.code()));
    }
    String::from_utf8(output.stdout).map_err(|e| format!("编码错误: {}", e))
}

fn confirm(msg: &str) -> bool {
    eprint!("{} [y/N] ", msg);
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

fn backup(file: &Path) -> Result<(), String> {
    let dir = backup_dir_for(file);
    fs::create_dir_all(&dir).map_err(|e| format!("创建备份目录失败: {}", e))?;
    let bak = dir.join(file.file_name().unwrap());
    if bak.exists() {
        eprintln!("  ⏭️  已有备份: {}", bak.display());
        return Ok(());
    }
    fs::copy(file, &bak).map_err(|e| format!("备份失败 {}: {}", file.display(), e))?;
    eprintln!("  📦 已备份: {}", file.display());
    Ok(())
}

fn create_protected(file: &Path) -> Result<(), String> {
    let dir = file.parent().unwrap();
    let pf = dir.join("PROTECTED");
    if pf.exists() { return Ok(()); }
    fs::write(&pf, "").map_err(|e| format!("创建 PROTECTED 失败: {}", e))?;
    eprintln!("  🛡️  已保护: {}/", dir.file_name().unwrap_or_default().to_string_lossy());
    Ok(())
}

fn apply_patches(file: &Path, patches: &[Patch], dry_run: bool) -> Result<usize, String> {
    let content = fs::read_to_string(file)
        .map_err(|e| format!("读取失败 {}: {}", file.display(), e))?;
    let mut result = content.clone();
    let mut count = 0;

    for p in patches {
        if result.contains(&p.new) { continue; } // 已汉化
        if !result.contains(&p.old) {
            eprintln!("  ⚠️  未找到原文 '{}' 于 {}", &p.old[..p.old.len().min(40)], file.display());
            continue;
        }
        result = result.replace(&p.old, &p.new);
        count += 1;
    }

    if count > 0 {
        if dry_run {
            eprintln!("  📝 将汉化: {} ({} 处)", file.display(), count);
        } else {
            fs::write(file, &result).map_err(|e| format!("写入失败 {}: {}", file.display(), e))?;
            eprintln!("  ✅ 已汉化: {} ({} 处)", file.display(), count);
        }
    }
    Ok(count)
}

fn check_file(file: &Path, patches: &[Patch]) -> (bool, usize) {
    let content = match fs::read_to_string(file) {
        Ok(c) => c,
        Err(_) => return (false, 0),
    };
    let mut applied = 0;
    for p in patches {
        if content.contains(&p.new) { applied += 1; }
    }
    (applied == patches.len(), applied)
}

// ─── 子命令 ──────────────────────────────────────────────
fn cmd_apply(patch_files: &[PatchFile], dry_run: bool, skip_confirm: bool) {
    eprintln!("━━━ ML4W 中文汉化 ━━━\n");

    if !skip_confirm && !confirm("将备份并汉化相关文件，是否继续？") {
        eprintln!("已取消");
        return;
    }

    let mut total = 0;
    for pf in patch_files {
        let path = expand(&pf.path);
        if !path.exists() {
            eprintln!("  ⚠️  文件不存在，跳过: {}", path.display());
            continue;
        }

        if pf.backup && !dry_run { let _ = backup(&path); }
        if pf.protect && !dry_run { let _ = create_protected(&path); }

        match apply_patches(&path, &pf.patches, dry_run) {
            Ok(n) => total += n,
            Err(e) => eprintln!("  ❌ {}", e),
        }
    }

    if dry_run {
        eprintln!("\n预览结束，实际修改 {} 处。去掉 --dry-run 执行。", total);
    } else {
        eprintln!("\n✅ 完成！共处理 {} 处翻译。", total);
        eprintln!("   按 Super + Ctrl + R 重载配置。");
        eprintln!("\n━━━ 注意事项 ━━━");
        eprintln!("  • ml4w 更新后可能需要重新运行本工具");
        eprintln!("  • 备份文件在 ~/.config/hypr/scripts/.ml4w-zh-backups/");
        eprintln!("  • 运行 `ml4w-zh revert` 可恢复英文原版");
        eprintln!("  • 技术力有限，不保证覆盖所有界面。欢迎 PR！\n");
    }
}

fn cmd_check(patch_files: &[PatchFile]) {
    eprintln!("━━━ 翻译状态检查 ━━━\n");
    let mut ok = 0;
    let mut outdated = 0;
    let mut missing = 0;
    let mut total_patches = 0;

    for pf in patch_files {
        let path = expand(&pf.path);
        total_patches += pf.patches.len();
        if !path.exists() {
            eprintln!("  ❌ 文件缺失: {}", pf.path);
            missing += 1;
            continue;
        }
        let (done, count) = check_file(&path, &pf.patches);
        if done {
            ok += 1;
        } else {
            eprintln!("  ⚠️  未完全汉化: {} ({}/{})", pf.path, count, pf.patches.len());
            outdated += 1;
        }
    }

    eprintln!("\n总计 {} 个文件，{} 条翻译", patch_files.len(), total_patches);
    eprintln!("  ✅ 已汉化: {}", ok);
    eprintln!("  ⚠️  未完全: {}", outdated);
    eprintln!("  ❌ 缺失:   {}", missing);
}

fn cmd_revert(patch_files: &[PatchFile], dry_run: bool) {
    eprintln!("━━━ 恢复英文原版 ━━━\n");
    if !dry_run && !confirm("将从备份恢复原文，是否继续？") {
        eprintln!("已取消");
        return;
    }

    let mut restored = 0;
    for pf in patch_files {
        let path = expand(&pf.path);
        let dir = backup_dir_for(&path);
        let bak = dir.join(path.file_name().unwrap());
        if !bak.exists() { continue; }

        if dry_run {
            eprintln!("  📝 将恢复: {}", path.display());
        } else {
            fs::copy(&bak, &path).ok();
            eprintln!("  ↩️  已恢复: {}", path.display());
        }
        restored += 1;
    }

    if restored == 0 {
        eprintln!("  没有找到备份文件。");
    }
    eprintln!("\n✅ 完成，恢复 {} 个文件。", restored);
}

fn cmd_init(patch_files: &[PatchFile], dry_run: bool) {
    eprintln!("━━━ 初始化（备份 + PROTECTED）━━━\n");
    for pf in patch_files {
        let path = expand(&pf.path);
        if !path.exists() { continue; }
        if pf.backup && !dry_run { let _ = backup(&path); }
        if pf.protect && !dry_run { let _ = create_protected(&path); }
    }
    eprintln!("\n✅ 初始化完成");
}

fn print_welcome() {
    eprintln!("\
╔══════════════════════════════════════╗
║       ml4w-zh  v{}                    ║
║   ML4W Hyprland 中文汉化工具         ║
╚══════════════════════════════════════╝
", VERSION);
    eprintln!("注意：本工具技术力有限，作者懒得搞自动构建。");
    eprintln!("推荐用法：ml4w 默认配置下运行 `ml4w-zh apply` 直接替换。");
    eprintln!("有问题/改进 → https://github.com/danxcheng/ml4w-zh\n");
}

fn print_usage() {
    eprintln!("\
用法: ml4w-zh <命令> [选项]

命令:
  apply       下载翻译并汉化（默认）
  check       检查哪些文件已汉化
  revert      从备份恢复英文原版
  init        仅备份 + 创建 PROTECTED 文件

选项:
  --dry-run   预览，不实际修改
  -y          跳过确认
  -h          显示此帮助
  -V          显示版本

示例:
  ml4w-zh                 一键汉化
  ml4w-zh --dry-run       预览
  ml4w-zh check           检查状态
  ml4w-zh revert          恢复英文
");
}

// ─── 入口 ────────────────────────────────────────────────
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("apply");
    let dry_run = args.contains(&"--dry-run".to_string());
    let skip_confirm = args.contains(&"-y".to_string());

    if args.contains(&"-h".to_string()) || args.contains(&"--help".to_string()) {
        print_welcome();
        print_usage();
        return;
    }
    if args.contains(&"-V".to_string()) {
        eprintln!("ml4w-zh v{}", VERSION);
        return;
    }
    if cmd == "help" {
        print_usage();
        return;
    }

    // 下载补丁定义
    print_welcome();
    if cmd != "revert" && cmd != "init" {
        eprint!("正在下载翻译定义... ");
        match download_patches(PATCHES_URL) {
            Ok(json) => {
                eprintln!("OK ({})", json.len());
                match parse_patches_json(&json) {
                    Ok(files) => {
                        match cmd {
                            "apply" | "install" => cmd_apply(&files, dry_run, skip_confirm),
                            "check" => cmd_check(&files),
                            "init" => cmd_init(&files, dry_run),
                            _ => print_usage(),
                        }
                    }
                    Err(e) => {
                        eprintln!("解析失败: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("失败!");
                eprintln!("错误: {}", e);
                eprintln!("请检查网络连接，或手动下载:");
                eprintln!("  curl -sL {} -o /tmp/patches.json", PATCHES_URL);
                std::process::exit(1);
            }
        }
    } else {
        // revert/init 不需要下载补丁，但需要补丁定义找文件
        match download_patches(PATCHES_URL) {
            Ok(json) => {
                match parse_patches_json(&json) {
                    Ok(files) => {
                        match cmd {
                            "revert" => cmd_revert(&files, dry_run),
                            "init" => cmd_init(&files, dry_run),
                            _ => print_usage(),
                        }
                    }
                    Err(e) => { eprintln!("解析失败: {}", e); std::process::exit(1); }
                }
            }
            Err(e) => {
                eprintln!("下载补丁定义失败: {}", e);
                eprintln!("revert 需要补丁定义来找备份文件，请确保网络可用。");
                std::process::exit(1);
            }
        }
    }
}
