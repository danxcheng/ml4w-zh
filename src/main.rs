// ml4w-zh v0.5 — ML4W dotfiles 汉化工具
//
// 流程:
//   1. 从 GitHub 或本地缓存拉 patches.json
//   2. 校验 patches.json 格式（old 字段必须存在于官方源）
//   3. 对每个文件，从 ml4w 官方源下载原版对比
//   4. 本地 == 官方 → 安全汉化
//   5. 本地 ≠ 官方 → 警告跳过
//   6. 改前备份 + 自动 PROTECTED

#![allow(clippy::pedantic)]

mod cli;
mod error;
mod patch;
mod util;

fn main() {
    if let Err(e) = cli::run() {
        eprintln!("❌ {e:#}");
        std::process::exit(1);
    }
}
