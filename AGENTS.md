# AGENTS.md — ml4w-zh

> 项目上下文文件，Hermes 和 Claude Code 自动读取。

## 项目简介

ML4W Hyprland 点文件的中文汉化工具。扫描 Hyprland 配置文件，将英文字段替换为中文。

## 技术栈

- **语言**: Rust (binary target)
- **构建**: cargo
- **关键 crate**: clap（参数解析）、anyhow（错误处理）、walkdir（文件遍历）、serde_yaml（YAML frontmatter）、regex（模式匹配）、colored（彩色输出）

## 目录结构

```
ml4w-zh/
├── src/
│   └── main.rs          ← 入口
├── patches/              ← 预置补丁文件
├── ml4w-zh-x86_64-linux ← 编译好的二进制
├── Cargo.toml
└── README.md
```

## 关键约定

1. 保持原有配置文件结构不变
2. 只替换显示文本，不动功能逻辑
3. 优先 Rust 实现，不依赖 shell 脚本
4. 错误处理使用 anyhow（bail!/context），不用 unwrap
5. CLI 参数用 clap derive 宏

## 构建 & 运行

```bash
cargo build --release
./target/release/ml4w-zh
```
