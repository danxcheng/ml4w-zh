# ml4w-zh

ML4W dotfiles 汉化工具 — 从 ml4w 官方源校验后替换，备份优先。

## 用法

```
ml4w-zh apply              从 ml4w 官方源校验后汉化
ml4w-zh apply --dry-run    预览，不修改
ml4w-zh check              检查汉化状态
ml4w-zh revert [文件名]     恢复全部或指定文件
ml4w-zh init               仅备份 + PROTECTED
ml4w-zh update-patches     下载最新翻译定义
ml4w-zh generate           从 TOML 定义生成 patches.json（开发者用）
```

## 工作流程

1. `ml4w-zh apply --dry-run` — 预览将要翻译的内容
2. `ml4w-zh apply` — 确认后执行汉化
3. `Super + Ctrl + R` — 重载 Hyprland 配置

每个文件汉化前自动备份到 `.ml4w-zh-backups/`。恢复用 `ml4w-zh revert`。

## 已知问题

### 1. 欢迎窗口大小
首次汉化后 Welcome 窗口的默认大小可能不合适。窗口可以手动拉伸，也可以用 `Win + T` 切换平铺/浮动后恢复。

### 2. "开机显示"按钮显示不全
Welcome 窗口底部的"开机显示"复选框在中文下可能显示不全。这是 QML 布局对中文字符宽度适配的问题，不影响功能。

### 3. 窗口标题不翻译
窗口标题（如 `ML4W Welcome`）保留英文，因为 Hyprland 的窗口规则和 killactive 脚本依赖标题匹配来识别窗口。

## 许可

MIT
