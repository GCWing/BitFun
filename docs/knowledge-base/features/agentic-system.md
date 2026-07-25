# AI Agent 协调系统

> **涉及 crate**: assembly/core, execution/agent-runtime, contracts/events
> **核心文件**: coordinator.rs, runtime.rs, agentic.rs
> **更新日期**: 2026-07-25

---

## 概述

BitFun 的 Agent 协调系统是产品核心——管理 Agent 的创建、对话、工具调用、事件分发和权限控制。

## 架构层级

```
┌─ Runtime Ports (contracts/runtime-ports) ─────────────────┐
│  trait AgentDialogTurnPort         ↕ Agent 对话流转        │
│  trait AgentSessionManagementPort  ↕ Session CRUD          │
│  trait AgentSubmissionPort         ↕ 任务提交               │
│  trait AgentTurnSettlementPort     ↕ Turn 结算              │
│  trait AgentTurnCancellationPort   ↕ Turn 取消              │
│  trait AgentSessionForkPort        ↕ Session 分支           │
│  trait AgentSessionClosePort       ↕ Session 关闭           │
│  trait AgentSessionModePort        ↕ 模式切换               │
│  trait AgentSessionModelPort       ↕ 模型切换               │
│  ...                                                       │
└───────────────────────────────────────────────────────────┘
        ↕ 实现
┌─ Agent Runtime (execution/agent-runtime) ─────────────────┐
│  AgentRuntime  struct                                     │
│  ├─ session_tree: SessionTreeManager     ← Session 树     │
│  ├─ agent_registry: RuntimeAgentRegistry ← Agent 注册表    │
│  ├─ plugin_runtime: PluginRuntimeBinding ← 插件绑定         │
│  ├─ permit_manager: PermissionManager    ← 权限管理         │
│  ├─ event_source: AgentEventSource       ← 事件源           │
│  └─ ...                                                   │
│  SessionRuntime struct                                     │
│  ├─ session_id                                            │
│  ├─ dialog_turn: AgentDialogTurnPort                      │
│  ├─ turn_settlement: AgentTurnSettlementPort              │
│  └─ ...                                                   │
└───────────────────────────────────────────────────────────┘
        ↕ 产品组装
┌─ Product Assembly (assembly/core) ────────────────────────┐
│  service_agent_runtime.rs                                 │
│  ├─ core_agent_runtime_builder()       ← 11 参数构建器    │
│  └─ create_session_runtime()           ← Session 运行时    │
│                                                           │
│  agentic/coordination/coordinator.rs                      │
│  ├─ Coordinator struct                  ← 总协调器         │
│  ├─ AgenticEvent 事件系统                                 │
│  ├─ Goal continuation 机制                                │
│  └─ 上下文压缩/恢复                                       │
│                                                           │
│  agentic/agents/registry/                                 │
│  ├─ AgentRegistry                        ← Agent 注册表    │
│  ├─ BuiltinAgentConfig                   ← 内置 Agent      │
│  ├─ SubagentConfig                       ← 子 Agent        │
│  └─ ExternalSubagentRegistration         ← 外部 Agent      │
└───────────────────────────────────────────────────────────┘
```

## 关键事件流（AgenticEvent）

| 事件 | 触发时机 | 消费者 |
|------|---------|--------|
| SessionCreated | Session 创建 | UI/日志 |
| SessionStateChanged | Session 状态变更 | UI/持久化 |
| DialogTurnStarted | 对话 turn 开始 | 前端 |
| DialogTurnCompleted | 对话 turn 结束 | 结算/审计 |
| SubagentTurnCompleted | 子 Agent turn 结束 | 协调器 |
| TextChunk / ThinkingChunk | 流式输出 | 前端 SSE |
| ToolEvent | 工具调用事件 | 工具链 |
| TokenUsageUpdated | Token 消耗 | 计费/统计 |
| ModelRoundCompleted | 模型轮次完成 | 协调器 |
| ContextCompressionStarted/Completed | 上下文压缩 | 上下文管理 |
| ImageAnalysisStarted/Completed | 图片分析 | 前端 |
| ReviewPropagationNeeded | 深度审查传播 | Ultra mode |

## Agent 注册系统

```
AgentRegistry (tokio::sync::RwLock)
├─ agents: HashMap<String, AgentEntry>        ← 全局 Agent
├─ project_subagents: HashMap<Path, ...>       ← 项目级子 Agent
├─ user_custom_agents_loaded: RwLock<bool>     ← 用户自定义加载状态
└─ external_subagents: ExternalSubagentRegistryState
```

**Agent 类型**:
- `builtin`: 内置 Agent（模板化的预设）
- `custom`: 用户自定义（提示词驱动）
- `external`: 外部子 Agent（通过 ACP 协议通信）
- `project_subagent`: 项目级子 Agent

## Goal Continuation 机制

```
对话结束 → Goal 条件检查
  ├─ 队列有未处理项 → 触发 continuation
  ├─ 有活跃 Session → 等待
  └─ 无待处理 → 正常结束

防止同时触发：双条件Guard
  1. queues.has_items() = false
  2. active_turns.is_empty() = true
```
