# bitfun-agent-runtime

**路径**: src/crates/execution/agent-runtime
**描述**: Agent runtime owner contracts。拥有可以在不依赖 bitfun-core 具体会话/调度器生命周期下构建和测试的运行时决策。

## 模块

- `agents` — Agent 定义
- `checkpoint` — 检查点
- `context_profile` — 上下文分析
- `custom_agent` — 自定义 agent
- `custom_subagent` — 自定义子 agent
- `deep_research` — 深度调研
- `deep_review` — 深度审核
- `dialog_turn` — 对话轮次
- `event_bus` — 事件总线
- `event_queue` — 事件队列
- `event_router` — 事件路由
- `event_source` — 事件源
- `events` — 事件
- `evidence_ledger` — 证据账本
- `file_read_state` — 文件读取状态
- `output_surface` — 输出表面
- `permission` — 权限
- `post_call_hooks` — 调用后 hooks
- `prompt` — 提示
- `prompt_cache` — 提示缓存
- `prompt_markup` — 提示标记
- `remote_file_delivery` — 远程文件投递
- `runtime` — 运行时
- `scheduled_job` — 定时任务
- `scheduler` — 调度器
- `sdk` — SDK
- `session` — 会话
- `session_control` — 会话控制
- `session_state` — 会话状态
- `session_state_manager` — 会话状态管理器
- `side_question` — 旁路问题
- `skill_agent_snapshot` — Skill agent 快照
- `skills` — Skills
- `thread_goal` — 线程目标
- `thread_goal_tools` — 线程目标工具
- `turn_cancellation` — 轮次取消
- `user_questions` — 用户提问

## 功能

Agent 运行时决策层。定义 agent 系统的核心编排逻辑，包括 agent 定义、对话轮次管理、会话状态管理、调度器、checkpoint、evidence ledger、prompt 缓存、文件读取状态追踪等。不依赖 bitfun-core 的具体实现，是产品运行时之间的抽象边界。包含 42 个模块，是 execution 层最核心的 crate。
