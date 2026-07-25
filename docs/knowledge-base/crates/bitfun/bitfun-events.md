# bitfun-events

**路径**: src/crates/contracts/events
**描述**: 独立的事件定义层，提供事件发送接口和各类事件类型，独立于平台。

## 模块

- `agentic` — Agentic 事件（AgenticEvent、Subagent、Tool 事件）
- `backend` — 后端事件（Tool 执行生命周期）
- `emitter` — EventEmitter trait（事件发送接口）
- `frontend_projection` — 前端事件投影
- `speech` — 语音模型事件常量
- `types` — 通用事件类型

## 核心类型

- `EventEmitter` — 事件发送 trait
- `AgenticEvent` — 核心 agentic 事件枚举（TextChunk、ThinkingChunk、ToolEvent、DialogTurnCancelled 等）
- `AgenticEventEnvelope` — 事件信封
- `AgenticEventPriority` — 事件优先级（Low/Normal/High/Critical）
- `AgenticFrontendEvent` — 前端投影事件
- `ToolEventData` — 工具事件数据（Started/Progress/Completed/Error/Cancelled 等）
- `ToolEventIdentity` — 工具事件身份标识
- `SubagentParentInfo` — 子 agent 父信息
- `DeepReviewQueueReason/State/Status` — 深度审核队列状态
- `ModelRoundAttemptDiagnostic` — 模型轮次诊断
- `BackgroundCommandLifecycleInfo` — 后台命令生命周期
- `ToolExecutionStartedInfo/ProgressInfo/CompletedInfo/ErrorInfo/TerminalReadyInfo` — 工具执行细节
- `SPEECH_MODEL_PROGRESS_EVENT`, `SPEECH_MODEL_STATUS_CHANGED_EVENT` — 语音事件常量

## 功能

事件层核心 crate。定义 agentic 运行时的完整事件体系，从前端文本流、thinking 流、工具调用事件到后台命令生命周期事件。平台无关，被 agent-stream、runtime-services、agent-runtime 等 crate 引用。
