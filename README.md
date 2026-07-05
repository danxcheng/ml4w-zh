# ml4w-zh

ML4W Hyprland dotfiles 中文汉化工具

一键汉化 ML4W 桌面环境的菜单、提示、通知、快捷键说明等所有界面文字。

## 用法

```bash
# 查看状态 — 检查哪些文件已汉化、哪些需要更新
ml4w-zh check

# 预览改动（不实际修改）
ml4w-zh apply --dry-run

# 应用所有汉化
ml4w-zh apply

# 从备份恢复英文原版
ml4w-zh revert

# 初始化：备份原文件 + 创建 PROTECTED 保护文件
ml4w-zh init
```

## 项目结构

```
ml4w-zh/
├── src/main.rs              # Rust CLI 工具
├── patches/
│   └── translations.yaml    # 翻译定义（文件路径 + 查找替换对）
├── docs/
│   └── 注意事项.md           # 详细技术说明
├── .github/workflows/       # CI：自动检查 ml4w 更新后补丁是否冲突
└── README.md
```

## 翻译定义格式

`patches/translations.yaml` 定义了每个文件要改什么：

```yaml
translations:
  - path: ".config/hypr/scripts/gamemode.sh"
    backup: true                    # 改前自动备份
    create_protected: true          # 创建 PROTECTED 文件防止 ml4w 更新覆盖
    patches:
      - old: '--s "Gamemode activated"'
        new: '--s "游戏模式已开启"'
      - old: '--m "Animations and blur are now disabled."'
        new: '--m "动画和模糊效果已禁用。"'
```

## 实现思路

- **备份优先**：改任何文件前先备份到 `.ml4w-zh-backups/`
- **PROTECTED 保护**：对 `hypr/scripts/` 下的文件自动创建 PROTECTED 文件，ml4w 安装器会跳过
- **幂等操作**：重复执行 `apply` 不会重复修改（除非用 `--force`）
- **Check 模式**：检查当前文件是否已被汉化，方便 ml4w 更新后确认哪些需要重新应用
