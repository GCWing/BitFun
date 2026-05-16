**English**  [中文](README.zh-CN.md)

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

## What BitFun Is

**BitFun is a desktop-grade Agent runtime (Local Agent Runtime) and a ready-to-use suite of desktop Agent applications.**

- It is the **foundation**—a Rust core plus a Tauri shell, with sessions, tools, memory, MCP, LSP, and remote-control protocols built in, designed for long-running use;

- It is the **product**—install once and you get four official Agents out of the box: Code, Cowork, Computer Use, and Personal Assistant, covering almost every mainstream Agent capability shape in the industry today.

> **One install: use it as an Agent, or use it as a Runtime.**

BitFun aims to pack **the coding power of Code Agents, the office productivity of Cowork, the assistant experience of OpenClaw, the control surface of Computer Use, and more**—the most popular Agent capabilities in the industry—into one desktop app, with the full protocol stack (Agentic runtime, tools, memory, MCP, Skills, context compression, remote control) ready by default. You can use it immediately, or define **your own domain Agents** on top of it.


![readme_hero](./png/readme_hero.png)


---

## Why BitFun

- **One app, almost every mainstream Agent capability in the industry**: Code / Cowork / Computer Use / document collaboration / generative UI / Mini App / MCP / remote control … No juggling multiple tools or paying for separate subscriptions for each.
- **Download and run—no DIY assembly**: MCP / LSP / filesystem / terminal / Git / remote SSH are all built in; configure your model and go, without spending time wiring the protocol stack from scratch.
- **Your data stays on your machine**: Sessions, memory, and working directories live under `.bitfun/sessions/`, portable, exportable, and auditable; nothing is forced to the cloud—suitable for privacy and compliance scenarios.
- **Deeply customizable, with no gap from a single Markdown file to a full-repo fork**: ~90% of domain needs are covered with one `.md`; missing a tool? a UI? want to change the product? Have the Code Agent do it inside BitFun—**the way you customize it is by using it**.
- **Control the desktop from your phone**: Pair by QR code, or use Telegram, Feishu Bot, or WeChat Bot as remote entry points. The Agent works on the desktop; you check progress on the go.
- **A desktop app you can actually live with**: Rust core + Tauri shell—fast cold start, low idle footprint, fine to leave running in the background for a long time.
- **Self-improving**: 97%+ of the code was produced by BitFun’s built-in Code Agent via Vibe Coding, so it naturally fits AI-assisted development.

---

## What's New

BitFun combines **flashgrep** with **ripgrep** into an enhanced code-search pipeline. On very large repositories such as Chromium, search time drops by up to about **94.6%**, with an average speedup of about **36.1×**, significantly reducing the time you spend exploring a project.

![flashgrep feature](./png/feat_flashgrep.png)

---

## Cutting Edge · Ready Out of the Box

New paradigms appear almost weekly in the Agent space. BitFun’s pace is: **when we see something great, we ship it on the desktop and make it work seamlessly with what you already have.**


![first_screen_screenshot](./png/first_screen_screenshot.png)

Below is BitFun’s **official Agent and capability inventory**, plus how we track the industry’s latest Agent patterns. Zero extra setup—download and use:

| Capability | Description |
| --- | --- |
| **Code Agent** | Four modes: Agentic (autonomous read / edit / run / verify) / Plan (plan first, then execute) / Debug (instrument → gather evidence → root cause) / Review (repo-standard review) |
| **Deep Review** | A parallel Code Review Team for higher-risk code changes, with reviewer roles, a quality gate, and user-approved remediation |
| **Session usage report** | Type `/usage` in chat to view recorded runtime, token usage, and model/tool/file summaries for the current session. |
| **Cowork Agent** | Native PDF / DOCX / XLSX / PPTX workflows; extend on demand from the Skill marketplace |
| **Document collaboration** | Write and ask in the document; the AI rewrites, continues, summarizes, and lays out text directly in paragraphs |
| **Computer Use** | Sees the screen and drives mouse and keyboard to operate browsers and any desktop app—hand repetitive clicking to the Agent |
| **Personal Assistant** | Long-term memory and personality; schedules Code / Cowork / Computer Use / custom Agents as needed |
| **Remote control / IM** | Phone QR pairing, Telegram, Feishu Bot, WeChat Bot for remote commands with live progress |
| **MCP / MCP App** | One-click hookup for external tools; MCP can also be packaged as installable Apps |
| **Generative UI** | On-demand interactive UI components during chat, embedded in the message stream for immediate use |
| **Mini App** | One sentence to a standalone runnable app—generate, run, one-click package for desktop |
| **Markdown-defined Agents** | Write a `.md` file and run it in the Runtime right away for most domain customization |
| **Long-term memory + project context** | Accumulates across sessions; readable by any Agent |
| **Self-iteration** | Code Agent can change BitFun’s own repository |
| **⋯⋯** | Next trends in progress—open an Issue with requests |

---

## How to Customize Your BitFun

Different depths of customization map to different-effort paths. Pick from light to heavy as needed:

| Tier | Approach | Best for | Effort |
| --- | --- | --- | --- |
| **L1** | **Markdown custom Agents** | Swap prompts + pick tool bundles to define a **new Agent capability**—covers most domain needs | Write one `.md` file |
| **L2** | **Mini App** | Capabilities that need UI (panels, forms, visualization, business flows) | One sentence to generate; run immediately |
| **L3** | **Source-level tools** | New tools, model adapters, protocols—give your custom Agent a `tool` BitFun doesn’t ship yet | Use BitFun’s Code Agent to edit BitFun’s own source |
| **L4** | **Free-form source changes** | Rebrand, rebuild UI, change session model, ship a totally different product | Fork the whole repo—naturally fits Vibe Coding |

### Example: Code Agent vs Cowork Agent is a small difference

In BitFun, an Agent = **a prompt (system role + behavior constraints) + the set of tools it may call**. The official Code Agent and Cowork Agent differ only in those two dimensions:

| | Code Agent | Cowork Agent |
| --- | --- | --- |
| **Prompt** | Role and norms for repo work; four operating modes | Role and document workflows for knowledge work |
| **Tooling** | Files / terminal / Git / LSP / build & test | PDF / DOCX / XLSX / PPTX / Skill marketplace |
| **Shared foundation** | Same sessions, memory, MCP, remote control, UI, model adapters | Same sessions, memory, MCP, remote control, UI, model adapters |

**So if you want a “legal review Agent,” a “research literature Agent,” or an “ops incident Agent”—L1 is enough**:

1. Write a Markdown file defining role / guardrails / workflow
2. From the tool registry, enable what it should use (files, browser, specific MCP …)
3. If a specific tool is missing—use **L3**: open BitFun and have the Code Agent add it in source
4. If the Agent needs a dedicated UI—use **L2**: one sentence to spin up a Mini App
5. If you want a completely different product—use **L4**: fork the repo and have the Code Agent help you reshape it

**Key point**: For L3 and L4 you never leave BitFun—**open BitFun, tell the Code Agent what to change, and it shows you the diff**. **The way you customize it is by using it.**

> From one Markdown file to a full fork, there is no discontinuity. That is what “a self-improving foundation” means.

---

## Platform Support

Desktop is built on Tauri for Windows / macOS / Linux; remote control works from mobile browsers, Telegram, Feishu, and WeChat.

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

## Project structure at a glance

```
src/crates/core         # Compatibility facade and product runtime assembly
src/crates/{core-types,agent-stream,runtime-ports} # Extracted core support boundaries
src/crates/{terminal,tool-runtime} # Workspace-level terminal/tool helper crates
src/crates/transport    # Tauri / WebSocket / CLI transport adapters
src/crates/api-layer    # Shared handlers and DTOs
src/apps/desktop        # Tauri desktop host
src/apps/server         # Web server runtime
src/apps/cli            # CLI runtime
src/web-ui              # Shared desktop / Web frontend
```

Design principle: **keep product logic platform-agnostic and expose it through adapters**. See [AGENTS.md](./AGENTS.md).

---

## Contributing

We welcome great ideas and code; we are maximally open to AI-generated code. Please submit PRs directly to the `main` branch; we review and merge there.

**Contribution directions we care about most:**

1. **Runtime core**: session model, tool registry, memory system, protocol adapters
2. **Reference Agents**: capabilities and experience for Code / Cowork / Personal Assistant
3. **Ecosystem**: Skills, MCP, LSP plugins, Mini App templates, and new vertical Agents
4. Ideas / creativity (features, interaction, visuals)—Issues welcome

---

## Disclaimer

1. This project is spare-time exploration and research into next-generation human–machine collaboration, not a commercial profit-making project.
2. More than 97% was built with Vibe Coding. Code feedback is welcome; refactoring and optimization via AI is encouraged.
3. This project depends on and references many open-source projects. Thanks to all open-source authors. **If your rights are affected, please contact us for remediation.**

---
