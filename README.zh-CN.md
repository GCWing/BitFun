**中文** | [English](README.md)

<div align="center">

![BitFun](./png/BitFun_title.png)

**有个性，有记忆的 AI 助理**

工作交给它，生活留给你

面向编程与知识工作的 AI Agent 工作台

</div>

<div align="center">

[![GitHub release](https://img.shields.io/github/v/release/GCWing/BitFun?style=flat-square&color=blue)](https://github.com/GCWing/BitFun/releases)
[![Website](https://img.shields.io/badge/Website-openbitfun.com-6f42c1?style=flat-square)](https://openbitfun.com/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow?style=flat-square)](https://github.com/GCWing/BitFun/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-blue?style=flat-square)](https://github.com/GCWing/BitFun)

</div>

---

## 简介

AI 时代，真正的人机协同不是简单的 ChatBox，而是一个懂你、陪你、自我成长并且随时随地替你做事的伙伴。BitFun 的探索，从这里开始。

BitFun 是一款内置 **Code Agent** 与 **Cowork Agent** 的新一代 AI 助理，有记忆、有个性，能自我迭代。可通过手机扫码或 Telegram / 飞书 Bot 随时遥控桌面端——下达指令、实时查看每一步执行过程，让 Agent 在后台替你做事。


![BitFun 产品界面](./png/first_screen_screenshot-zh-CN.png)

---

## 双模式协同

BitFun 提供两种模式，适配不同场景需求：

- **助理模式（Assistant Mode）**：有温度，记住偏好，具备长期记忆。适合持续协作类任务，如维护项目、延续你的审美与工作习惯。
- **专业模式（Professional Mode）**：省 token，直达执行，干净上下文。适合即时执行类任务，如修一个 bug、改一处样式。

---

## 远程遥控


扫码配对，手机即刻变成桌面 Agent 的远程指挥中心。一条消息，桌面上的 AI 立刻开始工作。

桌面端生成二维码，手机浏览器扫码打开即可使用，无需安装 App。

除手机扫码外，也支持接入 Telegram / 飞书 Bot 远程下达指令，并实时查看 Agent 的执行进度。

| 特性 | 说明 |
|---|---|
| **扫码配对** | 扫描桌面端二维码，密钥交换完成，一次绑定长期连接 |
| **完整遥控** | 查看会话列表、切换模式、下达指令，桌面端一切尽在掌控 |
| **实时推流** | Agent 执行的每一步、每个工具调用，手机端实时可见 |



## Agent 体系

| Agent | 定位 | 核心能力 |
|---|---|---|
| **个人助理**（Beta） | 你专属的 AI 伙伴 | 长期记忆、个性设定；按需调度 Code / Cowork / 自定义 Agent，并可自我迭代成长 |
| **Code Agent** | 代码代理 | 四种模式：Agentic（自主读改跑验证）/ Plan（先规划后执行）/ Debug（插桩取证→根因定位）/ Review（基于仓库规范审查） |
| **Cowork Agent** | 知识工作代理 | 内置 PDF / DOCX / XLSX / PPTX 处理，可从 Skill 市场按需获取和扩展能力包 |
| **自定义 Agent** | 垂域专家 | 通过 Markdown 快速定义专属领域 Agent |


## 生态扩展

> 它会自己成长。

Mini Apps 从对话中涌现，Skills 在社区里更新，Agent 在协作中进化。

| 扩展层 | 说明 |
|---|---|
| **Mini Apps** | 从一句需求生成可运行界面，并可一键打包成桌面应用 |
| **Skills 市场** | 安装社区能力包，让 Agent 快速获得新技能 |
| **MCP 协议** | 接入外部工具和资源，把 Agent 的能力延伸到系统之外 |
| **自定义 Agent** | 用 Markdown 定义角色、记忆和能力范围 |
| **ACP 协议（WIP）** | 结构化多 Agent 通信标准，让 BitFun 与主流 AI 工具互联协作 |

---

## 平台支持

项目采用 Rust + TypeScript 技术栈，支持跨平台和多形态复用，确保你的 Agent 助理随时在线、随处可达。

| 形态 | 支持平台 | 状态 |
|---|---|---|
| **Desktop**| Windows、macOS | ✅ 已支持 （Tauri） |
| **远程控制** | 手机浏览器、Telegram、飞书 | ✅ 已支持 |

---

## 快速开始

### 直接下载使用

在 [Releases](https://github.com/GCWing/BitFun/releases) 页面下载最新桌面端安装包，安装后配置模型即可开始使用。

> CLI、Server 和原生移动 App 仍在规划或开发中；当前已支持桌面端与远程控制能力。

### 从源码构建

**前置依赖：**

- [Node.js](https://nodejs.org/)（推荐 LTS 版本）
- [pnpm](https://pnpm.io/)
- [Rust 工具链](https://rustup.rs/)
- [Tauri 前置依赖](https://v2.tauri.app/start/prerequisites/)（桌面端开发需要）

```bash
# 安装依赖
pnpm install

# 以开发模式运行桌面端
pnpm run desktop:dev

# 构建桌面端
pnpm run desktop:build
```

更多详情请参阅[贡献指南](./CONTRIBUTING_CN.md)。

---

## 贡献

欢迎大家贡献好的创意和代码，我们对 AI 生成代码抱有最大的接纳程度。请 PR 优先提交至 `dev` 分支，我们会定期审视后同步到主干。

**我们重点关注的贡献方向：**

1. 贡献好的想法 / 创意（功能、交互、视觉等），提交 Issue
2. 优化 Agent 系统和效果
3. 提升系统稳定性和完善基础能力
4. 扩展生态（Skill、MCP、LSP 插件，或对某些垂域开发场景的更好支持）

---

## 声明

1. 本项目为业余时间探索、研究构建下一代人机协同交互，非商用盈利项目。
2. 本项目 97%+ 由 Vibe Coding 完成，代码问题也欢迎指正，可通过 AI 进行重构优化。
3. 本项目依赖和参考了众多开源软件，感谢所有开源作者。**如侵犯您的相关权益请联系我们整改。**

---

<div align="center">

世界正在被改写，这一次，你我皆是执笔人

</div>
