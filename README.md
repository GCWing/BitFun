[中文](README.zh-CN.md) | **English**

<div align="center">

![BitFun](./png/BitFun_title.png)

</div>
<div align="center">

[![GitHub release](https://img.shields.io/github/v/release/GCWing/BitFun?style=flat-square&color=blue)](https://github.com/GCWing/BitFun/releases)
[![Website](https://img.shields.io/badge/Website-openbitfun.com-6f42c1?style=flat-square)](https://openbitfun.com/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow?style=flat-square)](https://github.com/GCWing/BitFun/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue?style=flat-square)](https://github.com/GCWing/BitFun)

</div>

---

## Introduction

BitFun is an Agentic OS—and a companion right beside you.

It will interact through phones, watches, desktop robots, and more. It is part of your everyday life, and it evolves with you.

![first_screen_screenshot](./png/first_screen_screenshot.png)

---

## Remote Control

Pair by scanning a QR code, and your phone instantly becomes a remote command center for the desktop Agent. Send one message, and the AI on the desktop starts working right away.

Besides mobile browser pairing, BitFun also supports Telegram / Feishu bots / WeChat bots for remote commands, with real-time visibility into Agent progress.

---

## Dual Modes

BitFun offers two modes for different scenarios:

- **Assistant Mode**: warm, remembers your preferences, with long-term memory. Best for ongoing collaboration—maintaining a project, continuing your aesthetic and working habits.
- **Expert Mode**: saves tokens, execution-first, clean context. Best for immediate tasks—fixing a bug, tweaking a style.

---

## Agent System


| Agent            | Role                    | Core Capabilities                                                                                                                                                    |
| ---------------- | ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Personal Assistant** | Your dedicated AI companion | Long-term memory and personality; orchestrates Code / Cowork / custom Agents on demand, and can iterate and grow                                                    |
| **Code Agent**   | Coding agent            | Four modes: Agentic (autonomous read / edit / run / verify) / Plan (plan first, then execute) / Debug (instrument → gather evidence → root cause) / Review (repo-standard review) |
| **Cowork Agent** | Knowledge-work agent    | Built-in PDF / DOCX / XLSX / PPTX; fetch and extend capability packs from the Skill marketplace as needed                                                           |
| **Custom Agent** | Domain specialist       | Quickly define a domain-specific Agent with Markdown                                                                                                                 |


---

## Ecosystem

> It grows on its own.

Supports Skills, MCP (including MCP App), and custom Agents; also on-demand Mini Apps (from one line of requirements to a runnable UI, with one-click packaging into a desktop app).

---

## Platform Support

Built with Tauri for Windows, macOS, and Linux; mobile control works through the phone browser, Telegram, Feishu, WeChat, and more.

---

## Quick Start

### Download and use

Download the latest desktop installer from [Releases](https://github.com/GCWing/BitFun/releases). After installation, configure your model and start using BitFun.

### Build from source

**Prerequisites:**

- [Node.js](https://nodejs.org/) (LTS recommended)
- [pnpm](https://pnpm.io/)
- [Rust toolchain](https://rustup.rs/)
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) (required for desktop development)

**Commands:**

```bash
# Install dependencies
pnpm install

# Run desktop in development mode
pnpm run desktop:dev

# Build desktop
pnpm run desktop:build
```

For more details, see the [Contributing guide](./CONTRIBUTING.md).

---

## Contributing

We welcome great ideas and code; we are maximally open to AI-generated code. Please submit PRs directly to the `main` branch; we review and merge there.

**Contribution directions we care about most:**

1. Good ideas / creativity (features, interaction, visuals, etc.)—via Issues
2. Improving the Agent system and outcomes
3. Improving stability and foundational capabilities
4. Growing the ecosystem (Skills, MCP, LSP plugins, or better support for certain vertical development scenarios)

---

## Disclaimer

1. This project is spare-time exploration and research into next-generation human–machine collaboration, not a commercial profit-making project.
2. More than 97% was built with Vibe Coding. Code feedback is welcome; refactoring and optimization via AI is encouraged.
3. This project depends on and references many open-source projects. Thanks to all open-source authors. **If your rights are affected, please contact us for remediation.**

---
