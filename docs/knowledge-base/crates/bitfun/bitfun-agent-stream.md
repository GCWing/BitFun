# bitfun-agent-stream

**路径**: src/crates/execution/agent-stream
**描述**: Lightweight agent stream processing for BitFun。处理 AI streaming 响应，支持工具预检测和参数流。

## 模块

- `hidden_text` — 隐藏文本块解析（HiddenTextBlock/StreamParser/Tag）
- `tool_call_accumulator` — 工具调用累加器
- `unified` — 统一响应格式

## 核心类型

- `StreamProcessor` — 核心流处理器
  - `process_stream` — 处理 AI streaming 响应
  - `process_stream_with_options` — 带选项的流处理
  - `derive_watchdog_timeout` — 看门狗超时推导
  - 内部：`StreamContext` 管理累积状态
- `StreamResult` — 流处理结果（full_thinking, full_text, tool_calls, usage, provider_metadata, partial_recovery_reason 等）
- `StreamProcessError` — 流处理错误（AiClient/Cancelled）
- `ToolCall` — 精简工具调用值（tool_id, tool_name, arguments, raw_arguments, is_error, parse_error, repair_kind）
- `StreamEventSink` trait — 事件队列抽象
- `StreamProcessOptions` — 处理选项（partial recovery, JSON repair, hidden text tags）
- `SseLogCollector` — SSE 日志收集器（出错时输出原始 SSE 数据）
- `SseLogConfig` — SSE 日志配置
- `HiddenTextBlock`, `HiddenTextStreamParser`, `HiddenTextTag` — 隐藏文本标记系统
- `ToolArgumentRepairKind`, `ToolCallCompletion` — 工具参数修复/完成类型
- `UnifiedResponse`, `UnifiedTokenUsage`, `UnifiedToolCall` — 统一 AI 响应格式

## 功能

AI 流式响应处理引擎。接收 AI provider 的流式响应（SSE），累积文本/thinking/tool call 块，支持工具调用预检测和参数流式累积，错误恢复（partial recovery），JSON 参数修复，隐藏文本标签剥离，SSE 日志收集诊断。被 bitfun-core 的 StreamProcessor 重新导出。
