# ml4w-zh

ML4W Hyprland dotfiles 汉化工具。零依赖。

## 用法

```bash
# 一键汉化（自动下载翻译定义 → 备份原文件 → 替换）
ml4w-zh

# 预览（不实际修改）
ml4w-zh --dry-run

# 检查汉化状态
ml4w-zh check

# 恢复英文原版
ml4w-zh revert

# 仅备份 + 创建 PROTECTED 文件
ml4w-zh init
```

## 原理

1. 从 GitHub 拉取 `patches.json`（翻译定义）
2. 备份原文件到 `.ml4w-zh-backups/`
3. 对 `scripts/` 目录自动创建 `PROTECTED` 文件（ml4w 安装器会跳过）
4. 逐条替换

翻译定义在 `patches/patches.json`，欢迎 PR。

## 构建

```bash
cargo build --release
# 产物在 target/release/ml4w-zh
```

零外部依赖，只调系统自带的 `curl`。

## 注意

- 不搞自动构建，不搞 CI 检测
- ml4w 更新后重新跑一次 `ml4w-zh` 即可
- 备份在 `~/.config/hypr/scripts/.ml4w-zh-backups/`
- 技术力有限，不保证覆盖所有界面
