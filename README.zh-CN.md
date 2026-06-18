**中文**  [English](README.md)

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

## 以 Code Agent 为核心的本地 AI 工作台

BitFun 基于一个面向长程任务、强调工程执行与 Token 经济性的 Code Agent 打造本地 AI 工作台。

它能理解复杂上下文、调用工具、等待结果、修正偏差，把长程任务持续推进到可交付状态；编码、调研、办公、文档、桌面操作和可扩展工作流，都在同一个本地桌面环境里展开。

核心目标很直接：让 AI 从一次任务执行，进化成可以长期工作的生产力系统。

![readme_hero_CN](./png/readme_hero_CN.png)

---

## Agent 核心指标

下面的数据用于观察 BitFun Agent 的核心能力。统一使用 **Deepseek-V4-Pro**，从任务完成率、KV Cache 复用和大仓库检索效率三个指标评估。

BitFun 在 **SWE-Bench-Pro** 和 **SWE-Bench-Verified** 上均领先 Open Code 与 Claude Code。SWE-Bench-Pro 关注复杂软件工程，SWE-Bench-Verified 关注人工验证的 GitHub issue 修复。

| 评测集 | BitFun | Open Code | Claude Code |
| --- | ---: | ---: | ---: |
| **SWE-Bench-Pro** | **52%** | 51% | 50% |
| **SWE-Bench-Verified** | **74%** | 72% | 69% |

当前数据为每个 case 跑 1 次得到的 BitFun 初始评测结果，后续会持续优化并放出完整评测详情。评测集说明：[SWE-Bench-Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public) / [SWE-Bench-Verified](https://www.swebench.com/verified.html)

Agent 执行是否经济，关键在于重复上下文能否被稳定复用。同一轮 SWE-Bench-Pro 评测中，BitFun 的 KV Cache 平均命中率为 **98.67%**；728 条有效 cache 记录里，**83.1%** 的 trials 命中率不低于 98%，**51.8%** 不低于 99%。Token 侧，Cached Input 占 **98.71%**，Uncached Input (scaled) 占 **1.29%**。

| KV Cache 命中率 | 记录数 |
| --- | ---: |
| 80-90% | 7 |
| 90-95% | 13 |
| 95-98% | 103 |
| 98-99% | 228 |
| 99-99.5% | 239 |
| >=99.5% | 138 |

Agent 还需要反复寻找上下文。大仓库检索方面，BitFun 通过 **flashgrep** 在 Chromium 等超大仓库中最高降低约 **94.6%** 搜索耗时，平均加速约 **36.1x**。

| 检索路径 | 搜索耗时 |
| --- | ---: |
| 传统检索 | 145.9s |
| BitFun + flashgrep | 7.82s |

---

## 一个桌面，五类 Agent 工作流

| 工作流 | 解决什么 |
| --- | --- |
| **Code** | 面向真实仓库的 Code Agent: Agentic、Plan、Debug、测试、审查、Deep Review 和持续迭代。 |
| **Research** | 收集上下文、比较资料、总结发现，并输出结构化结论、报告或后续行动。 |
| **Cowork** | 处理 PDF / DOCX / XLSX / PPTX、写作、改写、总结、排版和办公协作。 |
| **Operate** | 通过 Computer Use 操作浏览器和桌面应用，完成点击、输入、跳转、等待和确认等流程。 |
| **Extend** | 接入 MCP、安装 Skills、定义 Markdown Agent、生成 Mini App，并继续改造 BitFun 自己。 |

![first_screen_screenshot_CN](./png/first_screen_screenshot_CN.png)

---

## 开箱即用

### 直接下载

前往 [Releases](https://github.com/GCWing/BitFun/releases) 下载最新桌面端安装包，安装后配置模型即可开始使用。

### 从源码运行

**前置依赖：**

- [Node.js](https://nodejs.org/)（推荐 LTS）
- [pnpm](https://pnpm.io/)
- [Rust 工具链](https://rustup.rs/)
- [Tauri 前置依赖](https://v2.tauri.app/start/prerequisites/)

```bash
pnpm install
pnpm run desktop:dev
```

更多开发说明见 [CONTRIBUTING_CN.md](./CONTRIBUTING_CN.md)。

---

## 定制你的 BitFun

BitFun 的扩展路径从轻到重连续展开：

| 层级 | 方式 | 适合场景 |
| --- | --- | --- |
| **L1** | Markdown Agent | 定义角色、流程、约束和工具组合。 |
| **L2** | MCP / Skills | 接入外部工具、专业能力和工作流。 |
| **L3** | Mini App | 为任务生成专属界面、表单、面板或可视化。 |
| **L4** | 源码级改造 | 修改工具、适配器、UI、Runtime 或产品形态。 |

你可以用 BitFun 的 Code Agent 来扩展 BitFun 本身。

---

## 贡献

欢迎 Star、Issue 和 PR。我们尤其关注：

1. Code Agent、Deep Review、调试和长任务执行能力
2. Cowork、调研、文档和桌面工作流
3. MCP、Skills、Mini App、LSP 插件和新领域 Agent
4. Runtime 稳定性、性能、上下文效率和可验证性

请将 PR 直接提交至 `main` 分支。更多说明见 [CONTRIBUTING_CN.md](./CONTRIBUTING_CN.md)。

---

## 声明

1. 本项目为业余时间探索、研究构建下一代人机协同交互，非商用盈利项目。
2. 本项目 97%+ 由 Vibe Coding 完成，代码问题欢迎指正，也欢迎通过 AI 进行重构优化。
3. 本项目依赖和参考了众多开源软件。感谢所有开源作者。如侵犯您的相关权益，请联系我们整改。
