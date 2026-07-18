
## 免责声明

- **Vibe Coding**: 本 PR 全部代码由 AI 生成，提交者无编程背景，仅提供设计思路、架构方向与代码示例参考。
- **开发环境**: 全程使用 BitFun + DeepSeek V4 Pro 对话开发，通过 BitFun DeepReview 进行代码审核。
- **实验性质**: 本 PR 是探索性框架示例，旨在验证 BitFun 原生基础设施是否支持复杂长任务的 Multi-Agent Workflow 编排能力。并非生产级实现，希望可为官方功能更新提供参考。
- **测试程度**: AI 辅助完成，已通过 `cargo check --workspace` + `pnpm run type-check:web` + Team 模式端到端功能验证。未经过完整单元测试覆盖。

---

## 核心设计思路

**不积跬步，无以至千里；千里之堤，溃于蚁穴。** 长任务实现的本质不是"一个 Agent 扛到底"，而是把每一步都做对——每一步过审查、每一步纠偏、每一步确定性输出。

**原子任务分解**: 任何复杂任务都可分解为不可再分的原子步骤。分解粒度决定并行度——可并行的并行（提效），有依赖的串行（保流程）。

**三蜂 Loop 循环**: 每个原子任务由提示蜂+执行蜂+审查蜂三个 Agent 通过 SessionMessage 自循环。Loop 确保执行上下文最大化节省 token，不通过 Gate 绝不输出到下一步。审查蜂不是"最后看一眼"，是每一步都盯——偷懒、幻觉、死循环、跳过验证——实时拦截+纠正注入。

**事实、效率、结果**: 三个不可妥协的维度。事实 = 每一步基于证据，不做假设；效率 = 最大化并行，最小化空转；结果 = Gate 通过才是输出，不通过就是回退。

**只调度不执行**: 军团长分解任务、拓扑排序、并行派发、Gate 裁决。不亲手改一行代码。

**Mesh 网状通信**: 军团成员之间 SessionMessage 直连（不经军团长），形成真正的对等网络。

**现成积木最大化复用**: DeepReview 审查团队已是完整三蜂结构。Claw 已有 session 工具。不做新引擎——泛化现有结构即可支持任意任务编排。不引入外部依赖。

**每一步都是正确的**: 行为守卫生效（Hook Abort）+ 审查蜂实时行为审计 + Gate Loop 层层把关 → 长任务才敢放心交出去。

---

## 蜂群架构：多Agent军团编排 + 审查Hook + DAG画布

### 一、核心架构思想

**公司(分解派发) + 军事(容错重组) + 航天(追溯零错)** 三域合一，通过八个固定Agent角色实现任意复杂度的任务编排。

#### 八个固定角色（积木）
| 角色 | 二元决策树 | 职责 |
|------|-----------|------|
| 指挥官 Commander | 树1-3 P→C→D | 感知全局→认知根因→决策方向，只指挥不执行 |
| 秘书 B01 Secretary | L3库检索 | 按任务领域检索提示词/技能/框架/规则，输出注入包 |
| 产品经理 ProductManager | R-ID需求定义 | 方向→结构化需求文档，分配追溯ID+验收标准 |
| 规划师 Planner | 树4-6 P→D→C | 需求→原子步分解，判依赖→串行/并行派发 |
| 执行者 Executor | 树7-10 O→O→D→A | 观察→思考→决策→执行，收到退回自动切Debug模式 |
| 审查者 Reviewer | 三节点审查 | ①功能审查(QA) ②安全审查(OWASP/CVE) ③Debug(诊断→退回) |
| 验收者 Acceptor | R-ID逐项闭合 | 对照需求文档按R-ID验证，全部闭合才交付 |
| 优化者 Optimizer | 复盘+归档+知识库 | 复盘进化→更新模板，新知识→写入L3库，产物→命名归档 |

#### 六条原则
> 串行保流程，并行提效率，循环促执行，节点控准确，原子降难度，嵌套解万物

#### 10棵二元决策树
每棵只有两条路：通过→下一棵，不通过→修正→重检→直到通过。任何节点可展开为完整子链（自由嵌套）。

#### 与ruflo的本质差异
角色固定（八个通用骨架），提示词灵活。同一executor注入量化提示词→量化执行者，注入前端提示词→前端执行者。B01管理领域提示词库。

---

### 二、LegionControl 军团编排

**动态DAG拓扑排序执行引擎。** 读JSON模板→拓扑排序→分层创建session→注入上下文→串行/并行派发。

#### 核心实现
- `LegionControl` 工具：load模板 / list / status，支持自定义agent类型
- 拓扑排序算法：条件边跳过（空condition字段不参与环检测），分层并发创建
- `SessionControl`：Collapsed→Expanded，支持动态创建/列出/关闭会话
- `SessionMessage`：跨会话消息派发，传workspace路径和agent类型
- `SessionHistory`：导出会话转录，支持turn选择器（单轮/范围/最后N轮）

#### 预设模板
- `bee-colony-standard`：8节点全链（cmd→sec→pm→plan→exec→review→accept→opt）
- `bee-colony-quick`：4节点精简链（cmd→exec→review→accept）
- `bee-colony-parallel`：12节点3并行执行者+各自审查
- `bee-colony-single`：单执行者模式
- `bee-colony-multi-legion`：14节点多军团接力

---

### 三、cc-haha 模式审查Hook

**每轮对话结束触发LLM审查（非自审，非Agent会话），审查结果注入下一轮上下文。**

```
每轮（有工具调用）
  │
  ├─ stop hook: 检查 REVIEW_BUFFER → 注入上轮审查结果
  │     ABORT:* → HookResult::Abort（硬拦截）
  │     CTX:*   → [书记官] 上下文恢复
  │     SKILL:* → [提示蜂] 技能推荐
  │     WARN:*  → [审查员] 警告
  │
  └─ std::thread → primary model → LLM审查本轮
       → 结果 push REVIEW_BUFFER → 下轮stop hook取出
```

#### 审查员三合一职责
1. **书记官(上下文守护)**：检测上下文压缩→提取压缩前关键决策/进度/用户纠正→CTX注入
2. **纪律委员(行为审查)**：Read-before-Edit、LionHeart保护、策略死循环、PS+中文+JSON、全部工具失败、改后未验证→ABORT/WARN
3. **提示蜂(技能推荐)**：匹配Agent行为到可用BitFun技能→SKILL推荐

#### 设计要点
- cc-haha模式：stop→外部进程→结果注入，不创建Agent会话，不fork上下文
- 使用primary模型（与主Agent同款），非独立fast模型
- 同步检查全部移除，AI全权判断
- 平时沉默（PASS过滤），只在违规/上下文退化时出声

---

### 四、STALE_TRACKER 修复

新增`last_target`字段区分"同工具不同文件"和"同工具同文件"。
- Write commander.md → Write planner.md → Write executor.md（不同target）→计数器重置
- 修复前：Write×4连续触发误判Abort
- 修复后：target变化时计数器重置

---

### 五、蜂群DAG画布MiniApp

#### 内置MiniApp `bee-colony-dag`
- 8节点纵向DAG（cmd→sec→pm→plan→exec→review→accept→opt）
- 4色状态：idle=灰、running=蓝(脉冲动画)、done=绿、failed=红
- Gate节点(review/accept)红色虚线边框
- 500ms轮询`app.storage('bee-colony-state')`实时更新

#### BeeColonyMonitor浮动面板
- GitBranch按钮固定在Agent场景右下角
- 点击展开MiniAppRunner加载蜂群DAG
- 支持最大化/还原

#### 状态推送工具
`bee_colony_state_push.py`：set/reset/批量JSON三种模式，直写storage.json

---

### 六、Agent注册

| Agent | 类型 | 文件 |
|-------|------|------|
| commander | Mode | `%APPDATA%/bitfun/agents/commander.md` |
| secretary (B01) | Subagent | `%APPDATA%/bitfun/agents/secretary.md` |
| product-manager | Subagent | `%APPDATA%/bitfun/agents/product-manager.md` |
| planner | Subagent | `%APPDATA%/bitfun/agents/planner.md` |
| executor | Subagent | `%APPDATA%/bitfun/agents/executor.md` |
| reviewer | Subagent | `%APPDATA%/bitfun/agents/reviewer.md` |
| acceptor | Subagent | `%APPDATA%/bitfun/agents/acceptor.md` |
| optimizer | Subagent | `%APPDATA%/bitfun/agents/optimizer.md` |
| bee-reviewer | Subagent | `%APPDATA%/bitfun/agents/bee-reviewer.md` |
| b01-context-agent | Subagent | `%APPDATA%/bitfun/agents/b01-context-agent.md` |
| c01-audit-agent | Subagent | `%APPDATA%/bitfun/agents/c01-audit-agent.md` |

---

### 七、关键文件变更

| 文件 | 变更 |
|------|------|
| `legion_control_tool.rs` | 新增，拓扑排序+分层创建+条件边 |
| `post_call_hooks.rs` (core) | cc-haha审查：REVIEW_BUFFER+stop hook注入+三职合一批量处理 |
| `post_call_hooks.rs` (agent-runtime) | ToolCallSummary扩展input_preview字段 |
| `execution_engine.rs` | 每轮spawn审查LLM调用+primary模型 |
| `session_control_tool.rs` | Collapsed→Expanded曝光 |
| `session_message_tool.rs` | Collapsed→Expanded曝光 |
| `session_history_tool.rs` | Collapsed→Expanded曝光 |
| `builtin.rs` | bee-colony-dag内置注册 |
| `BeeColonyMonitor.tsx/.scss` | 新增浮动审查面板 |
| `commander.md` | 嵌入任务启动协议(B01优先) |
| `SKILL.md` v10.4 | 上下文恢复协议+三域架构+八角色完整定义 |
| `AgentsScene.tsx` | 解决main合并冲突，两边import都保留 |

---

## Summary

Agentic Dynamic Workflows: 泛化 BitFun 原生会话工具，实现智能体军团编排——Agent 通过 SessionControl 创建子 Agent 会话节点，SessionMessage 发送任务并自动接收回复，SessionHistory 审查战报，Goal 追踪进度。不引入新引擎，全用原生基础设施。

Fixes #N/A（新功能）

## Type and Areas

Type: **feat**

Areas: **Rust core, desktop/Tauri, web UI, ACP interface**

## Motivation / Impact

**问题**: Team 模式的 SessionControl/SessionMessage/SessionHistory 三个工具处于 Collapsed 状态，Agent 看不到。session 工具的 agent_type 硬编码，ACP 外部 Agent 无法被编排。DeepReview 审查团队被限制为代码审查专用。

**改动**:
- 3 个 session 工具 Collapsed→Expanded，Team 模式直接可用
- agent_type 从硬编码枚举改为 AgentRegistry 动态获取（含 ACP）
- ACP Agent 注册为 Mode，出现在智能体选择器中
- LegionControl 工具：读军团模板 JSON，拓扑排序，一键创建多个 Agent 会话
- Hook 框架加 Abort 能力 + BehaviorGuard 行为检测
- team_mode.md 重写：从 gstack skills（316行）→ 军团指挥系统（108行）+ Gate Loop Protocol

**影响**: Team/Claw/agentic/Plan/Debug/Multitask 模式现在全部能创建和互发消息。ACP 外部 Agent 可被编排。审查蜂可见性保持不变。

## Verification

```bash
# Rust
cargo check --workspace  # 零 error，零 warning

# TypeScript  
pnpm run type-check:web  # 零 error

# E2E 功能验证（Team 模式）
SessionControl(action:"list")              ✅ 列出 11 个会话
SessionControl(action:"create")            ✅ 创建 agentic 节点
SessionMessage(session_id, message)        ✅ Plan/agentic/ACP 通信闭环
SessionHistory(session_id)                 ✅ 战报导出
LegionControl(action:"list"/"load")        ✅ 列表+拓扑创建
Goal get/create/update/complete            ✅ 追踪
ACP agent_type 动态注册                    ✅ 13 个 Agent 入注册表
LegionControl 条件边跳过                    ✅ fail 边不参与循环检测
Hook Abort 框架编译                         ✅ 无 error
```

## Reviewer Notes

**架构边界**: 所有改动严格遵循 BitFun 六层架构：
- contracts: 未改
- execution: HookResult::Abort + BehaviorGuard（post_call_hooks.rs）
- services: 未改
- adapters/ACP: AcpAgent 注册（manager.rs）
- assembly/core: 工具曝光 + 动态 agent_type + LegionControl + prompt 重写
- interfaces: ACP agent 入前端核心区

**不引入新引擎**: 全用 SessionControl/SessionMessage/SessionHistory/Goal/Task 原生工具。

**不破坏现有功能**: DeepReview 审查流程不变，审查蜂可见性保持 Hidden/restricted 原始状态。

**AI 辅助**: 本 PR 由 AI 辅助完成，已通过 cargo check --workspace + type-check:web + 完整 E2E 功能验证。

## Checklist

- [x] This PR is focused and does not include secrets, temporary prompts, generated scratch files, or unrelated artifacts.
- [x] Relevant verification is recorded above.
- [x] User-facing strings, docs, and locales are updated where applicable.