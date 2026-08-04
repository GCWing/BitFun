**中文**  [English](README.md)

<div align="center">

![BitFun](./png/BitFun_title.png)

### 开源桌面 AI Agent —— 在真实仓库里交付代码，在真实桌面上动手干活。

Code Agent · Cowork · Computer Use —— 本地优先，基于 Rust Runtime。

[**⬇ 下载 macOS · Windows · Linux 版**](https://github.com/GCWing/BitFun/releases/latest)

[官网](https://openbitfun.com/) · [文档](./docs) · [讨论区](https://github.com/GCWing/BitFun/discussions) · [参与贡献](./CONTRIBUTING_CN.md)

[![GitHub release](https://img.shields.io/github/v/release/GCWing/BitFun?style=flat-square&color=blue)](https://github.com/GCWing/BitFun/releases)
[![Downloads](https://img.shields.io/github/downloads/GCWing/BitFun/total?style=flat-square&color=brightgreen)](https://github.com/GCWing/BitFun/releases)
[![Stars](https://img.shields.io/github/stars/GCWing/BitFun?style=flat-square&color=yellow)](https://github.com/GCWing/BitFun/stargazers)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow?style=flat-square)](https://github.com/GCWing/BitFun/blob/main/LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue?style=flat-square)](https://github.com/GCWing/BitFun/releases)

[![Trendshift](https://trendshift.io/api/badge/repositories/44672)](https://trendshift.io/repositories/44672)

</div>

<!-- TODO: 把下面这张截图换成 20-30 秒的真实任务演示 GIF，
     录制脚本见 scripts/record-demo.sh —— 这是整个 README 里最值得做的一张图。 -->

![BitFun 桌面端](./png/first_screen_screenshot_CN.png)

---

## 安装

**直接下载** —— 前往 [Releases](https://github.com/GCWing/BitFun/releases/latest) 下载最新桌面端安装包，安装后配置模型即可开始使用。

**或从源码运行：**

```bash
pnpm install
pnpm run desktop:dev
```

前置依赖：[Node.js](https://nodejs.org/) 22.12+（推荐 LTS）、[pnpm](https://pnpm.io/) 10.15.0（建议通过 Corepack 使用）、[Rust 工具链](https://rustup.rs/)、[Tauri 前置依赖](https://v2.tauri.app/start/prerequisites/)。更多说明见 [CONTRIBUTING_CN.md](./CONTRIBUTING_CN.md)。

---

## 你可以把什么交给 BitFun

两类复杂工作：在真实仓库里完成编码交付，在资料和文件中完成办公交付。遇到需要浏览器、桌面软件、终端或远程环境的任务时，它可以进入真实工作现场。

| 场景 | 目标交付 | 典型能力 |
| --- | --- | --- |
| **编码** | 从真实仓库推进到可合并结果。 | Agentic、Plan、Debug、测试、Git、Deep Review、长程任务、Benchmark。 |
| **办公** | 从资料推进到可交付文档。 | Research、PPT、DOCX、XLSX、PDF、总结、写作、会议纪要、报告。 |

**通用能力**

- **桌面执行底座**：Computer Use、浏览器操作、桌面应用、文件系统、终端、远程工作区和 Mini App，让 Agent 能进入真实工作环境。
- **可定制化扩展**：MCP、Skills、Agent 自定义、Mini App 和源码级扩展，让 BitFun 可以按你的工具链、角色和界面继续生长。

---

## Agent 核心指标

下面的数据用于观察 BitFun Agent 的核心能力，统一使用 **Deepseek-V4-Pro** 测得。

> [!NOTE]
> 当前数据为每个 case 跑 1 次得到的 BitFun 初始评测结果。评测会受到任务抽样、模型版本、运行环境和单次执行偶然性的影响，存在一定波动；这组数据仅用于说明当前 Agent 已具备可用的基础竞争力，并不代表固定排名或最终上限。后续会持续优化并放出完整评测详情。

**1. 完成效果** —— BitFun 在 **SWE-Bench-Pro**（复杂软件工程）和 **SWE-Bench-Verified**（人工验证的 GitHub issue 修复）上均领先 Open Code 与 Claude Code。

![Agent benchmark scores](./png/agent_benchmark_scores.svg)

评测集说明：[SWE-Bench-Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public) / [SWE-Bench-Verified](https://www.swebench.com/verified.html)

**2. Token 经济** —— Agent 执行是否经济，需要综合评估端到端 Token 消耗、执行耗时和 KV Cache 复用。同一轮 SWE-Bench-Pro 中，BitFun 的平均 KV Cache 命中率为 **98.67%**。后续完整评测会继续补充成本与耗时指标。

![KV Cache hit rate distribution](./png/kv_cache_hit_rate.png)

**3. 超大工程下的上下文检索** —— 成本之外，Agent 体验还取决于它能否在超大工程里快速找回上下文。面对 Chromium 这类千万行级代码仓库，BitFun 通过 **flashgrep** 最高降低约 **94.6%** 搜索耗时，平均加速约 **36.1x**。

![flashgrep search speed](./png/flashgrep_search_speed.png)

---

## 定制你的 BitFun

BitFun 的扩展路径从轻到重连续展开：

| 层级 | 方式 | 适合场景 |
| --- | --- | --- |
| **L1** | Agent 自定义 | 定义角色、流程、约束和工具组合。 |
| **L2** | MCP / Skills / [Hooks](docs/features/agent-hooks.zh-CN.md) | 接入外部工具和专业能力，并在 Agent 生命周期节点运行你自己的命令 —— 完全兼容 Codex Hooks，已有脚本无需适配。 |
| **L3** | Mini App | 为任务生成专属界面、表单、面板或可视化。 |
| **L4** | 源码级改造 | 修改工具、适配器、UI、Runtime 或产品形态。 |

你可以用 BitFun 的 Code Agent 来扩展 BitFun 本身。

---

## 我们要做成什么样

- **黑灯工厂**（正在构建中）：白天设计，夜间任务流转到服务器持续执行，早上直接验收成果。
- **无限半径**（正在构建中）：从桌面、浏览器持续延伸到移动端、可穿戴等更多设备，让工作随时接入、连续协作。
- **应用进化**：支持自定义 Agent、MCP、Skills、Mini App 乃至源码级改造，组合专属工作流；**社区伙伴已拓展出短剧、媒体等丰富版本**。
- **多快好省**：追求更高效率、更优效果与更低成本。
- **极致桌面**：持续打磨更易用、更好用、更漂亮的桌面体验。

![readme_hero_CN](./png/readme_hero_CN.png)

---

## 社区与贡献

有问题、想法或 Bug，欢迎到 [讨论区](https://github.com/GCWing/BitFun/discussions) 和 [Issues](https://github.com/GCWing/BitFun/issues) 交流。

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
