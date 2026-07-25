# Taiji 全方位透视代码图谱

> **范围**: taiji-quant 分支（基于 BitFun v0.2.14）
> **包含**: 36 个 BitFun 上游 crate + 19 个 taiji-quant 量化引擎 crate + 前端（web-ui + mobile-web）
> **代码量**: 1564 .rs / 1190 .ts / 537 .tsx / 99 .scss
> **生成日期**: 2026-07-25

---

## 图谱结构

```
docs/knowledge-base/
├── index.md                  ← 本文件，总索引
├── dependency-graph.md       ← 全 workspace 依赖关系图
│
├── crates/
│   ├── bitfun/               ← 36 个上游 BitFun crate 分析
│   └── taiji/                ← 19 个量化引擎 crate 分析
│
├── features/
│   ├── agentic-system.md     ← AI Agent 协调系统
│   ├── ultra-mode.md         ← Ultra/deep 多 Agent 模式
│   ├── legion-mode.md        ← 军团编排系统
│   ├── frontend-architecture.md ← 前端架构
│   └── ...
│
└── references/               ← 参考文档、外部链接
```

## BitFun 核心架构分层

```
apps/                         ← 可执行入口
├── cli                       ← CLI 命令行
├── desktop                   ← Tauri 桌面应用
├── server                    ← 服务端
├── relay-server              ← 中继服务
└── sdk-host                  ← SDK 宿主

crates/
├── contracts/                ← 契约层（最底层，被所有依赖）
│   ├── core-types            ← 核心 DTO 类型
│   ├── events                ← 事件系统
│   ├── runtime-ports         ← 运行时端口 trait
│   └── product-domains       ← 产品领域模型
│
├── execution/                ← 执行引擎
│   ├── agent-runtime         ← Agent 运行时核心
│   ├── agent-stream          ← Agent 流处理
│   ├── tool-*                ← 工具执行系统
│   ├── harness               ← 执行 harness
│   ├── plugin-runtime-host   ← 插件运行时宿主
│   └── runtime-services      ← 运行时服务
│
├── assembly/                 ← 产品组装层
│   ├── core                  ← 核心组装（最复杂，~400 文件）
│   ├── external-sources      ← 外部源集成
│   └── product-capabilities  ← 产品能力
│
├── services/                 ← 服务层
│   ├── services-core         ← 核心服务
│   ├── services-integrations ← 第三方集成（MCP/git/ssh）
│   ├── relay-service         ← 中继服务
│   ├── page-function-runtime ← 页面函数运行时
│   └── terminal              ← 终端服务
│
├── adapters/                 ← 适配器层
│   ├── ai-adapters           ← AI 提供商适配
│   ├── claude-code-adapter   ← Claude Code 适配
│   ├── codex-adapter         ← Codex 适配
│   ├── opencode-adapter      ← OpenCode 适配
│   ├── static-hook-support   ← 静态钩子支持
│   ├── webdriver             ← WebDriver
│   └── transport             ← 传输层
│
├── interfaces/               ← ACP 协议层
│   ├── acp                   ← Agent Communication Protocol
│   └── sdk-host              ← SDK 宿主接口
│
└── taiji/                    ← 量化引擎（本仓库独有）
    ├── taiji-engine          ← 核心引擎（DAG 管线）
    ├── taiji-llm             ← LLM 客户端
    ├── taiji-backtest        ← 回测
    ├── taiji-executor        ← 执行
    ├── taiji-realtime        ← 实时数据
    ├── taiji-content         ← 内容生成
    ├── taiji-pattern         ← 模式匹配
    ├── taiji-abnormal        ← 异常检测
    ├── taiji-sentiment       ← 情绪分析
    ├── taiji-orderflow       ← 订单流
    ├── taiji-strategen       ← 策略生成
    ├── taiji-publisher       ← 多平台发布
    ├── taiji-growth          ← 运营增长
    ├── taiji-alert           ← 告警
    ├── taiji-knowledge-graph ← 知识图谱
    ├── taiji-bar             ← K线聚合
    ├── taiji-blog-gen        ← 博客生成
    ├── taiji-cli             ← 量化 CLI
    └── taiji-strategy-template ← 策略模板
```

## 核心功能域索引

| 功能域 | 涉及 crate | 说明 |
|--------|-----------|------|
| 🧠 AI Agent 协调 | assembly/core, execution/agent-runtime | Agent 生命周期、对话管理、工具调用 |
| 🌀 多 Agent 协作 | execution/agent-runtime, adapters/* | Ultra mode、Deep review、多 Agent 链 |
| ⚙️ ACP 协议 | interfaces/acp, adapters/* | Agent 间通信协议 |
| 🔗 MCP 集成 | services/services-integrations | MCP client/server 协议实现 |
| 🖥️ 前端 UI | web-ui, mobile-web | React/TypeScript 桌面+移动端 |
| 📈 量化引擎 | taiji/* | DAG 管线、回测、执行、策略 |
| 🔌 插件系统 | execution/plugin-runtime-host | 插件运行时 |
| 🚀 CLI | apps/cli | 命令行界面 |
| 🪟 桌面 | apps/desktop | Tauri 桌面应用 |

## 依赖流向

```
contracts/ (最底层)
  → execution/
    → assembly/
      → services/
        → adapters/
          → interfaces/
            → apps/
              → (用户)

taiji/ (与 assembly/core 平级)
  → 可单独编译，不依赖 apps/
```

> 详情见 [dependency-graph.md](dependency-graph.md)
