# bitfun-tool-call-jsonrepair

**路径**: src/crates/execution/tool-call-jsonrepair
**描述**: Tool-call JSON repair profile forked from jsonrepair-rs。流式工具参数的 JSON 修复库。

## 模块

- `chars` — 字符处理
- `error` — 错误类型
- `parser` — JSON 修复解析器

## 核心类型

- `RepairOptions` — 修复选项（strict/forgiving）
- `JsonRepairError`, `JsonRepairErrorKind` — 修复错误
- `JsonRepairWriteError`, `JsonRepairStreamError` — I/O 错误包装

## 核心函数

- `jsonrepair(input)` — 修复破损 JSON 字符串（默认模式）
- `jsonrepair_with_options(input, options)` — 带选项修复
- `repair_tool_call_json(input)` — 工具调用 JSON 修复 profile（不把 `#` 当注释）
- `jsonrepair_to_writer`, `jsonrepair_reader_to_writer` — 写入器/读取器 API
- `jsonrepair_value`, `jsonrepair_parse` — 修复并解析（feature: serde）

## 支持修复的类型

- 单引号/花引号 → 双引号
- 尾随/缺失逗号
- 注释（`//`, `/* */`, `#`）
- Python 关键字（True/False/None）
- JavaScript 关键字（undefined/NaN/Infinity）
- Markdown 代码围栏
- JSONP 包装器
- 未引用的键和字符串
- 截断 JSON（自动闭括号）
- 字符串拼接
- 无效转义序列
- MongoDB 构造器
- NDJSON
- 省略号操作符

## 功能

从 jsonrepair-rs MIT 库 fork 的工具调用 JSON 修复 crate。专门为 AI 模型输出的不完整/不规范的 JSON 参数设计。`repair_tool_call_json` profile 不把 `#` 当注释处理（保留 Markdown 内容）。是 agent-stream 中工具参数修复的后端。
