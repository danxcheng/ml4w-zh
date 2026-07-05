# ml4w-zh

ML4W Hyprland dotfiles 汉化工具。**仅测试于 Arch Linux + ml4w 默认配置。**

## 运行逻辑

```
                                    ┌─ 拉 patches.json（翻译定义）
                                    │   https://github.com/danxcheng/ml4w-zh/patches/patches.json
                                    │
ml4w-zh apply ──────────────────────┼─ 对每个文件：
                                    │   ├─ 从 ml4w 官方 GitHub 下载原版对比
                                    │   │   raw.githubusercontent.com/mylinuxforwork/dotfiles/...
                                    │   ├─ 本地 == 官方原版 → 安全，继续汉化
                                    │   ├─ 已有中文         → 跳过（幂等）
                                    │   └─ 本地 ≠ 官方     → 警告跳过，不盲改
                                    │   ├─ 备份原文件 → .ml4w-zh-backups/
                                    │   └─ 替换 old → new
                                    │
ml4w-zh check ──────────────────────┼─ 对比官方源，报告四种状态：
                                    │   ✅ 已汉化    本地有中文，官方未变
                                    │   🔄 需更新    有中文但官方已更新
                                    │   📝 未汉化    官方一致，未翻译
                                    │   ❌ 缺失      文件不在本地
                                    │
ml4w-zh revert ─────────────────────┼─ 离线恢复，扫 .ml4w-zh-backups/ 目录
                                    │   不依赖网络，不下载任何东西
                                    │
ml4w-zh init ───────────────────────┘─ 仅备份 + 创建 PROTECTED 文件
```

## 安装

```bash
# 下载二进制
curl -sL https://github.com/danxcheng/ml4w-zh/releases/latest/download/ml4w-zh-x86_64-linux -o /usr/local/bin/ml4w-zh
chmod +x /usr/local/bin/ml4w-zh

# 或者从源码编译
cargo install ml4w-zh
```

## 用法

```bash
# 一键汉化（自动下载翻译定义 → 对比官方源 → 备份 → 替换）
ml4w-zh

# 先看看会改什么
ml4w-zh --dry-run

# 检查状态
ml4w-zh check

# 恢复英文
ml4w-zh revert

# 仅备份 + PROTECTED
ml4w-zh init
```

汉化后按 **Super + Ctrl + R** 重载 Hyprland 配置。

## 依赖

- **运行时**: `curl`（几乎所有 Linux 都有）
- **编译时**: Rust 工具链 + `serde` + `serde_json`
- **系统**: 仅测试于 **Arch Linux** + **ml4w 默认配置**

## ⚠️ 注意

- **仅测试于 Arch Linux + ml4w 默认配置**。如果你改了 ml4w 源码，`check` 会检测到"与官方不一致"并跳过
- ml4w 更新后重新跑一次 `ml4w-zh` 即可
- 备份在 `~/.config/hypr/scripts/.ml4w-zh-backups/`
- 技术力有限，欢迎 PR

## 项目结构

```
ml4w-zh/
├── src/main.rs              # 源码
├── patches/patches.json     # 翻译定义（old → new 对照表）
├── ml4w-zh-x86_64-linux    # 预编译二进制（strip 后 542KB）
└── README.md
```
