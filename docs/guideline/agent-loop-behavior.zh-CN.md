# Agent loop 行为

> 根 `AGENTS.md` 配套（STD-06）。修改 agent 主循环、重复工具调用处理，或防死循环策略时阅读。
>
> 另见 `src/crates/execution/` 下就近模块文档（尤其 `agent-runtime`）。
>
> [English](agent-loop-behavior.md)

## 硬约束

- 不要把「硬编码限制 / 模式匹配」当成处理 agent loop 死循环的第一手段，例如仅凭字符串相同或调用次数就拦截重复工具调用。
- 硬编码堆多了，会把 agent loop 拧成脆弱的流程引擎。应先查根因：工具实现、模型交互、会话上下文如何打包、prompt / tool schema 设计，或状态是否不同步。
