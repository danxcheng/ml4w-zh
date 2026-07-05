use anyhow::{Context, Result};
use clap::Parser;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

/// ML4W-zh: Chinese localization tool for ML4W Hyprland dotfiles
#[derive(Parser)]
#[command(name = "ml4w-zh", version, about = "Apply/check/revert Chinese translations for ML4W dotfiles")]
enum Cli {
    /// Apply all translations
    Apply {
        /// Dry run — show what would change without modifying
        #[arg(long)]
        dry_run: bool,
        /// Force apply even if backups exist
        #[arg(long, short)]
        force: bool,
    },
    /// Check translation status — show which files are up-to-date vs outdated
    Check,
    /// Revert all translated files from backups
    Revert {
        /// Force revert without confirmation
        #[arg(long, short)]
        force: bool,
    },
    /// Initialize: copy original ml4w files to backup, set up PROTECTED files
    Init {
        /// Dry run
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Deserialize)]
struct Config {
    translations: Vec<FileTranslation>,
}

#[derive(Debug, Deserialize)]
struct FileTranslation {
    path: String,
    #[serde(default)]
    backup: bool,
    #[serde(default)]
    create_protected: bool,
    #[serde(default)]
    replace_all: bool,
    patches: Vec<Patch>,
}

#[derive(Debug, Deserialize)]
struct Patch {
    old: String,
    new: String,
}

fn expand_path(p: &str) -> PathBuf {
    if p.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/danxcheng".to_string());
        PathBuf::from(home).join(&p[2..])
    } else {
        PathBuf::from(p)
    }
}

fn read_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config: {}", path.display()))?;
    let cfg: Config = serde_yaml::from_str(&content)?;
    Ok(cfg)
}

fn backup_file(path: &Path) -> Result<()> {
    let backup_dir = path.parent().unwrap().join(".ml4w-zh-backups");
    fs::create_dir_all(&backup_dir)?;
    let backup_path = backup_dir.join(path.file_name().unwrap());
    if !backup_path.exists() {
        fs::copy(path, &backup_path)?;
        println!("  📦 Backed up -> {}", backup_path.display());
    }
    Ok(())
}

fn create_protected_flag(path: &Path) -> Result<()> {
    let protected_path = path.parent().unwrap().join("PROTECTED");
    if !protected_path.exists() {
        fs::write(&protected_path, "")?;
        println!("  🛡️  Created PROTECTED file: {}", protected_path.display());
    }
    Ok(())
}

fn apply_patches(
    path: &Path,
    patches: &[Patch],
    replace_all: bool,
    dry_run: bool,
) -> Result<usize> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read: {}", path.display()))?;
    let mut result = content.clone();
    let mut count = 0;

    for patch in patches {
        if replace_all {
            if result.contains(&patch.old) {
                result = result.replace(&patch.old, &patch.new);
                count += 1;
            }
        } else {
            if let Some(pos) = result.find(&patch.old) {
                result.replace_range(pos..pos + patch.old.len(), &patch.new);
                count += 1;
            }
        }
    }

    if count > 0 {
        if dry_run {
            println!(
                "  📝 Would patch {} ({} changes)",
                path.display(),
                count
            );
        } else {
            fs::write(path, &result)?;
            println!("  ✅ Patched {} ({} changes)", path.display(), count);
        }
    } else {
        println!("  ⏭️  No changes needed: {}", path.display());
    }

    Ok(count)
}

fn cmd_apply(cfg: &Config, dry_run: bool, _force: bool) -> Result<()> {
    let mut total = 0;
    for ft in &cfg.translations {
        let path = expand_path(&ft.path);
        if !path.exists() {
            println!("  ⚠️  File not found, skipping: {}", path.display());
            continue;
        }

        println!("→ {}", ft.path);

        if ft.backup && !dry_run {
            backup_file(&path)?;
        }

        if ft.create_protected && !dry_run {
            create_protected_flag(&path)?;
        }

        total += apply_patches(&path, &ft.patches, ft.replace_all, dry_run)?;
    }
    println!("\n📊 Total: {} patches applied", total);
    Ok(())
}

fn cmd_check(cfg: &Config) -> Result<()> {
    let mut ok = 0;
    let mut outdated = 0;
    let mut missing = 0;

    for ft in &cfg.translations {
        let path = expand_path(&ft.path);
        if !path.exists() {
            println!("  ❌ Missing: {}", ft.path);
            missing += 1;
            continue;
        }

        let content = fs::read_to_string(&path)?;
        let mut file_ok = true;
        for patch in &ft.patches {
            if !content.contains(&patch.new) {
                println!("  ⚠️  Outdated: {} (missing: {:?})", ft.path, patch.new);
                file_ok = false;
                outdated += 1;
                break;
            }
        }
        if file_ok {
            ok += 1;
        }
    }

    println!(
        "\n📊 Status: {} up-to-date, {} outdated, {} missing",
        ok, outdated, missing
    );
    Ok(())
}

fn cmd_revert(cfg: &Config, _force: bool) -> Result<()> {
    for ft in &cfg.translations {
        let path = expand_path(&ft.path);
        let backup_dir = path.parent().unwrap().join(".ml4w-zh-backups");
        let backup_path = backup_dir.join(path.file_name().unwrap());

        if backup_path.exists() {
            fs::copy(&backup_path, &path)?;
            println!("  ↩️  Reverted: {}", ft.path);
        } else {
            println!("  ⏭️  No backup for: {}", ft.path);
        }
    }
    println!("\n✅ Revert complete");
    Ok(())
}

fn cmd_init(cfg: &Config, dry_run: bool) -> Result<()> {
    for ft in &cfg.translations {
        let path = expand_path(&ft.path);
        if !path.exists() {
            println!("  ⚠️  Not found: {}", ft.path);
            continue;
        }

        if dry_run {
            println!("  📦 Would backup: {}", ft.path);
            if ft.create_protected {
                println!("  🛡️  Would create PROTECTED in: {}", path.parent().unwrap().display());
            }
        } else {
            if ft.backup {
                backup_file(&path)?;
            }
            if ft.create_protected {
                create_protected_flag(&path)?;
            }
            println!("  ✅ Init: {}", ft.path);
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cfg = read_config(Path::new("patches/translations.yaml"))
        .context("Could not load patches/translations.yaml. Run from project root.")?;

    match Cli::parse() {
        Cli::Apply { dry_run, force } => cmd_apply(&cfg, dry_run, force),
        Cli::Check => cmd_check(&cfg),
        Cli::Revert { force } => cmd_revert(&cfg, force),
        Cli::Init { dry_run } => cmd_init(&cfg, dry_run),
    }
}
