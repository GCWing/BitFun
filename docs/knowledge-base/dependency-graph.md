# Taiji Workspace 依赖关系图

> 基于 Cargo.toml 自动提取，覆盖全部 55 个 workspace member
> 生成日期: 2026-07-25

---

## 量化引擎依赖（taiji/*）

```
taiji-engine ──────────────────────────────┐
  ├─ taiji-llm (LLM client)                │
  ├─ taiji-bar (K线数据)                    │
  ├─ taiji-example (示例数据)                │
  ├─ taiji-backtest (回测)                  │
  └─ taiji-executor (执行引擎)              │
    │                                       │
    ▼                                       ▼
taiji-abnormal ◄── taiji-engine         taiji-pattern ◄── taiji-engine
taiji-sentiment ◄── taiji-engine        taiji-orderflow ◄── taiji-engine
taiji-realtime ◄── taiji-engine         taiji-strategen ◄── taiji-engine
                                           ├─ taiji-backtest
                                           └─ taiji-llm
taiji-alert ──┬─ (外部: lettre/reqwest)
              └─ (内部: 独立)

taiji-content ──┬─ image: {workspace}
                └─ bitfun-core (路径依赖)

taiji-growth ──┬─ taiji-content
               ├─ taiji-engine
               └─ (外部: tera/lettre/reqwest)

taiji-publisher ──┬─ taiji-content
                  └─ (外部: reqwest/futures)

taiji-knowledge-graph ──┬─ taiji-engine
                        └─ (外部: petgraph)

taiji-blog-gen ─── taiji-growth (二进制)

taiji-cli ───┬─ taiji-engine
             ├─ taiji-bar
             ├─ taiji-example
             └─ taiji-backtest
```

## BitFun 核心依赖流向

```
# 最底层：被所有 crate 依赖
src/crates/contracts/core-types     ← 无内部依赖
src/crates/contracts/events         ← core-types
src/crates/contracts/runtime-ports  ← core-types

# 执行层：依赖契约层
src/crates/execution/tool-contracts     ← core-types
src/crates/execution/tool-call-jsonrepair ← (独立)
src/crates/execution/harness            ← tool-contracts
src/crates/execution/runtime-services   ← (独立)
src/crates/execution/agent-stream       ← runtime-ports + events
src/crates/execution/agent-runtime      ← runtime-ports + services-core + events
src/crates/execution/plugin-runtime-host ← (独立)
src/crates/execution/tool-provider-groups ← (独立)
src/crates/execution/tool-execution     ← (独立)

# 服务层：依赖执行层+契约层
src/crates/services/services-core           ← core-types + events + runtime-ports
  └─ 被 agent-runtime 依赖
src/crates/services/services-integrations    ← (MCP/git/SSH 集成)
src/crates/services/relay-service           ← (中继)
src/crates/services/terminal                ← (终端)

# 组装层：依赖所有下层
src/crates/assembly/core                    ← 依赖 services/* + execution/* + interfaces/*
  └─ 最复杂的 crate（~400 文件）
src/crates/assembly/external-sources        ← (外部数据源)
src/crates/assembly/product-capabilities    ← (产品能力)

# 适配器层
src/crates/adapters/ai-adapters              ← (独立 AI 适配)
src/crates/adapters/claude-code-adapter     ← ai-adapters
src/crates/adapters/codex-adapter           ← ai-adapters
src/crates/adapters/opencode-adapter        ← ai-adapters
src/crates/adapters/static-hook-support     ← (独立)
src/crates/adapters/webdriver               ← (独立)
src/crates/adapters/transport               ← (独立)

# 应用层
src/apps/cli             ← assembly/core + interfaces/acp
src/apps/desktop         ← assembly/core + interfaces/acp + Tauri
src/apps/server          ← (独立)
src/apps/relay-server    ← (中继服务)
src/apps/sdk-host        ← (SDK 宿主)
```

## 关键依赖项（外部 crate 用途）

| 外部 crate | 用途 | 被哪些使用 |
|-----------|------|-----------|
| tokio | 异步运行时 | 几乎全部 |
| serde/serde_json/serde_yaml | 序列化 | 几乎全部 |
| reqwest | HTTP 客户端 | services-core, relay, adapters |
| axum | HTTP 服务器 | relay-server, adapters |
| tauri | 桌面框架 | apps/desktop |
| rmcp | MCP 协议 | services-integrations |
| clap | CLI 参数解析 | apps/cli |
| ratatui/crossterm | TUI 终端 | apps/cli |
| image | 图像处理 | assembly/core, taiji-content |
| sherpa-onnx | 语音识别 | assembly/core |
| tracing/log | 日志 | 几乎全部 |
| rusqlite | SQLite 数据库 | services-core |
| git2 | Git 操作 | services-integrations |
| rquickjs | JavaScript 引擎 | services |
| oxc | JS/TS 解析器 | services |

## 量化引擎特有依赖

| 外部 crate | 用途 | 被哪些使用 |
|-----------|------|-----------|
| petgraph | DAG 图计算 | taiji-engine, taiji-knowledge-graph |
| candle-core | ML 推理 | taiji-llm (optional) |
| ndarray | 数值计算 | taiji-pattern |
| statrs | 统计 | taiji-abnormal, taiji-backtest |
| lettre | 邮件发送 | taiji-alert, taiji-growth |
| tera | 模板引擎 | taiji-growth, taiji-blog-gen |
| jieba-rs | 中文分词 | taiji-sentiment |
| csv | CSV 读写 | taiji-engine |
| pyo3 | Python 绑定 | taiji-engine-py |
