# Data Flow Maps

> 关键数据在系统内的流动路径
> 更新日期: 2026-07-25

---

## 1. Agent 对话流程

```
User Input
  → CLI/Desktop (apps/cli 或 apps/desktop)
    → assembly/core: Coordinator.receive_session_event()
      → AgenticEvent::DialogTurnStarted
        → execution/agent-runtime: SessionRuntime.execute_dialog_turn()
          → AI Provider (adapters/ai-adapters)
            → assembly/core: ModelRoundStarted/Completed
              → streaming: TextChunk / ThinkingChunk events
              → tools: ToolEvent → ToolRegistry → ToolPipeline → ToolExec
                → services/git/ssh/filesystem
          ← DialogTurnCompleted
        → TurnSettlement: token usage, permissions
      → Goal Continuation check
    → Response to user
```

## 2. 量化交易数据流

```
Real-time Market Tick
  → taiji-realtime (WebSocket bridge)
    → taiji-bar (Tick → KLine aggregation)
      → taiji-engine: Pipeline
        → Pipeline::feed_tick_direct()
          → BarGenerator (generate/update bars)
            → ComputeNode[0]: technical indicators
              → ComputeNode[N]: derived signals
                → Signal fusion (taiji-engine/fusion)
                  → Risk check (taiji-engine/risk)
                    → Compliance check (taiji-engine/compliance)
                      → Signal → taiji-executor
                        → OrderManager → Bridge → Exchange

Backtest flow:
  Historical Data → DataSource (replay module)
    → Pipeline (same pipeline, replay mode)
      → TradeRecord → backtest stats → WalkForward
```

## 3. 事件系统流

```
Event Source (AgentRuntim/Coordinator/apps)
  → AgenticEventEnvelope (id + priority + timestamp)
    → Event Queue (priority-ordered)
      → Event Router
        → Frontend (SSE stream → web-ui)
        → Backend subscribers (persistence/audit/logging)

Event Priority:
  Critical: SystemError, DialogTurnFailed
  High: SessionStateChanged, DeepReviewQueueStateChanged
  Normal: TextChunk, ToolEvent, ModelRoundCompleted
  Low: (catch-all)
```

## 4. ACP 协议流（Agent Communication Protocol）

```
Agent A (in-process)
  → interfaces/acp: ACP Client
    → transport (ws/http)
      → Relay Server (apps/relay-server)
        → transport
          → interfaces/acp: ACP Server
            → Agent B (remote process)

External Subagent flow:
  apps/cli → ACP Client → Relay → ACP Server → ExternalSubagentRegistration
    → ExternalSubagentRuntime → execution → results → back through ACP
```

## 5. MCP 集成流（Model Context Protocol）

```
Agent needs external tool
  → services-integrations: MCP Client
    → rmcp::ClientCapabilities
      → transport_remote (SSE streaming)
        → MCP Server (external)
          → tool list → tool call → tool result
    ← 结果通过 AgenticEvent::ToolEvent 回传
```

## 6. 插件运行时流

```
Plugin (compiled .wasm / .so)
  → execution/plugin-runtime-host
    → PluginRuntimeBinding
      → rquickjs (JavaScript sandbox)
        → oxc (JS/TS parsing for analysis)
    ↔ agent-runtime (shared state + events)
```

## 7. 前端状态流

```
web-ui (React/TypeScript)
  → Session SSE stream
    → Redux/Zustand store
      → Components (TreeView, Chat, SessionList)
        → User interaction
          → API calls back to backend
            → new AgenticEvent cycle

Mobile-web (React)
  → Same core API
    → Mobile-optimized UI (touch, responsive)
      → Pairing flow (QR code → remote session)
