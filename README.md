# Last Token

<p align="center">
  <img src="public/last-token.svg" alt="Last Token Logo" width="128" height="128">
</p>

<p align="center">
  <strong>跨平台 LLM / AI API 额度实时监测与耗尽预警工具</strong>
</p>

<p align="center">
  <a href="https://github.com/wangm23456/last-token/actions"><img src="https://img.shields.io/github/actions/workflow/status/wangm23456/last-token/build-windows.yml?branch=main&label=Build" alt="Build Status"></a>
  <a href="https://github.com/wangm23456/last-token/releases"><img src="https://img.shields.io/github/v/release/wangm23456/last-token?include_prereleases&label=Release" alt="Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"></a>
</p>

---

## 📖 简介 (About)

**Last Token** 是一款基于 **Tauri 2 + React + Rust** 构建的高性能轻量级桌面应用。它专为频繁使用各类 LLM API 的开发者打造，能够自动捕获并监控各大 AI 服务提供商的配额使用情况、计算实时消耗速率，并准确预测额度耗尽时间，避免在关键开发阶段因 API 额度枯竭而中断工作。

所有的 API 凭证均优先从本地 CLI 配置文件或环境变量中只读加载，绝不上传至任何第三方服务器。

---

## ✨ 核心特性 (Features)

- 🤖 **多提供商支持**：覆盖 **Claude (CLI/Code)**、**GitHub Copilot**、**Kimi For Coding**、**MiniMax 编程套餐**、**Gemini (CLI)**、**智谱 GLM (个人/团队版)**、**火山方舟 Ark** 以及 **ZenMux 代理**。
- ⚡ **自动扫描与一键导入**：
  - 自动读取本地官方 CLI 授权配置（如 `~/.claude/.credentials.json`、`~/.codex/auth.json`、`~/.gemini/oauth_creds.json`）。
  - 智能识别 Shell 环境变量（如 `MOONSHOT_API_KEY`、`MINIMAX_API_KEY`、`ANTHROPIC_API_KEY` 等）。
- 📈 **智能消耗速率与耗尽预测**：通过时间序列快照算法计算当前时段消耗速率（如 `%/小时`），结合重置周期准确预警风险。
- 🖥️ **双界面形态**：支持完整桌面主界面面板与无缝沉浸的 **macOS 顶部状态栏 / 托盘小组件 (Tray Panel)**。
- 🔒 **隐私与安全第一**：敏感 API 密钥仅在本地内存及 SQLite 存储，绝不经过外部中转服务。

---

## 📦 安装与下载 (Downloads)

你可以直接前往 [Releases 页面](https://github.com/wangm23456/last-token/releases) 下载适合你操作系统的安装包：

- **macOS (Apple Silicon & Intel)**: `.dmg` / `.app`
- **Windows**: `.exe` (NSIS) / `.msi`
- **Linux**: `.deb` / `.AppImage`

---

## 🛠️ 本地开发与构建 (Development)

### 环境要求 (Requirements)

- [Node.js](https://nodejs.org/) (>= 18) 或 [Bun](https://bun.sh/)
- [Rust](https://www.rust-lang.org/) (>= 1.75)
- [Tauri CLI](https://tauri.app/v2/start/) 依赖环境

### 快速开始 (Quick Start)

```bash
# 1. 克隆项目仓库
git clone https://github.com/wangm23456/last-token.git
cd last-token

# 2. 安装前端依赖
bun install

# 3. 启动本地开发模式 (Vite + Tauri Dev)
bun run tauri dev
```

### 自动化测试与检查 (Testing & Diagnostics)

```bash
# 前端类型检查
bun run typecheck

# 运行前端单元测试 (Vitest)
bun run test

# 运行后端 Rust 单元测试
cd src-tauri && cargo test
```

### 应用打包 (Production Build)

```bash
# 构建本地生产包
bun run tauri build
```

---

## 🚢 CI/CD 与自动化发布 (Release Pipeline)

本项目配置了基于 **GitHub Actions** 及 **CrabNebula Cloud** 的自动化构建发布流水线 (`.github/workflows/build-windows.yml`)：

当你需要发布新版本时，只需在本地打标签并推送：

```bash
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions 将自动触发多平台矩阵构建（macOS, Windows, Linux），并上传产物至 GitHub Releases 及 CrabNebula CDN。

---

## 📄 开源协议 (License)

本项目基于 [MIT License](LICENSE) 协议开源。
