# Last Token

<p align="center">
  <img src="public/last-token.png" alt="Last Token Logo" width="128" height="128">
</p>

<p align="center">
  <strong>Cross-platform LLM / AI API quota monitoring and exhaustion alerts</strong>
</p>

<p align="center">
  <a href="README.md">English</a> ·
  <a href="README_ZH.md">中文</a>
</p>

<p align="center">
  <a href="https://website-psi-liard-42.vercel.app"><img src="https://img.shields.io/badge/Website-Live-brightgreen" alt="Website"></a>
  <a href="https://github.com/wangm23456/last-token/actions"><img src="https://img.shields.io/github/actions/workflow/status/wangm23456/last-token/build-windows.yml?branch=main&label=Build" alt="Build Status"></a>
  <a href="https://github.com/wangm23456/last-token/releases"><img src="https://img.shields.io/github/v/release/wangm23456/last-token?include_prereleases&label=Release" alt="Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"></a>
</p>

---

## About

**Last Token** is a lightweight desktop app built with **Tauri 2 + React + Rust**. It is designed for developers who rely on LLM APIs every day: it monitors quota usage across major AI providers, estimates burn rate, and predicts when credits will run out — so critical work is not interrupted by sudden exhaustion.

API credentials are loaded locally from CLI config files or environment variables. Nothing is uploaded to third-party servers.

🌐 Landing page: [website-psi-liard-42.vercel.app](https://website-psi-liard-42.vercel.app)

---

## Features

- **Multi-provider support**: Claude (CLI/Code), GitHub Copilot, Kimi For Coding, MiniMax coding plans, Gemini (CLI), Zhipu GLM (personal/team), Volcano Ark, and ZenMux proxy.
- **Auto scan & one-click import**:
  - Reads official CLI auth configs (e.g. `~/.claude/.credentials.json`, `~/.codex/auth.json`, `~/.gemini/oauth_creds.json`).
  - Detects shell environment variables (e.g. `MOONSHOT_API_KEY`, `MINIMAX_API_KEY`, `ANTHROPIC_API_KEY`).
- **Burn-rate & exhaustion forecasts**: Time-series snapshots estimate usage rate (e.g. `%/hour`) and warn before reset windows.
- **Dual UI**: Full desktop dashboard plus a macOS menu-bar / system tray panel.
- **Privacy first**: Secrets stay in local memory and SQLite only.

---

## Downloads

Grab installers from the [Releases](https://github.com/wangm23456/last-token/releases) page:

- **macOS (Apple Silicon & Intel)**: `.dmg` / `.app`
- **Windows**: `.exe` (NSIS) / `.msi`
- **Linux**: `.deb` / `.AppImage`

---

## Development

### Requirements

- [Node.js](https://nodejs.org/) (>= 18) or [Bun](https://bun.sh/)
- [Rust](https://www.rust-lang.org/) (>= 1.75)
- [Tauri CLI](https://tauri.app/v2/start/) toolchain

### Quick start

```bash
git clone https://github.com/wangm23456/last-token.git
cd last-token

bun install
bun run tauri dev
```

### Checks

```bash
bun run typecheck
bun run test

cd src-tauri && cargo test
```

### Production build

```bash
bun run tauri build
```

### Landing page (`website/`)

```bash
cd website
bun install
bun run dev
```

Production deploys automatically via Vercel when `website/` changes on `main`.

---

## Release pipeline

Tag and push to trigger GitHub Actions + CrabNebula Cloud builds:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The workflow builds macOS, Windows, and Linux artifacts and uploads them to GitHub Releases / CrabNebula CDN.

---

## License

MIT — see [LICENSE](LICENSE).
