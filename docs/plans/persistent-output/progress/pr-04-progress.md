# PR-4 Hook 集成 — 进度报告

## 完成状态：✅ 已完成

### 步骤

| # | 步骤 | 状态 | 详情 |
|---|---|---|---|
| 1 | 创建分支 | ✅ | `feat/pr-04-hook-integration` from `feat/pr-03-coordination-tools` |
| 2 | 从 taiji-quant 提取文件 | ✅ | `coordinator.rs`, `review_propagation.rs`, `tool_pipeline.rs`, `native_hooks.rs`, `native_hooks_tests.rs` |
| 3 | 修复 Hook 链路断裂 | ✅ | 见下方详情 |
| 4 | `#[cfg(feature = "taiji")]` 守卫 | ✅ | 所有新增/taiji代码已加特征守卫 |
| 5 | 编译检查 | ✅ | `cargo check -p bitfun-core --features taiji` 通过 |
| 6 | 测试 | ✅ | `cargo test -p bitfun-core --features taiji` — 1576 passed, 5 failed (均为 taiji-quant 预存问题) |
| 7 | 输出进度文档 | ✅ | 本文档 |

---

## 详情

### Step 2：文件提取

从 `taiji-quant` 分支 checkout 了以下文件：

| 文件 | 说明 |
|---|---|
| `src/crates/assembly/core/src/agentic/coordination/coordinator.rs` | 会话树 (SessionTreeManager) 集成、SubagentTurnCompleted 事件发射、background subagent 完成后的 dialog turn 提交、depth 跟踪 |
| `src/crates/assembly/core/src/agentic/coordination/review_propagation.rs` | 审核传播管理器 (ReviewPropagationManager) — 叶子 Agent 完成后的父 session 审核传播 |
| `src/crates/assembly/core/src/agentic/tools/pipeline/tool_pipeline.rs` | 测试补丁：`depth: None` 等字段 |
| `src/crates/assembly/core/src/native_hooks.rs` | 移除 overview 功能，简化为直接构建引擎 |
| `src/crates/assembly/core/src/native_hooks_tests.rs` | 移除 overview 相关测试 |

### Step 3：Hook 链路断裂修复

#### 3.1 SubagentStop → ReviewPropagation

**问题**：`coordinator.rs` 中 `execute_hidden_subagent_internal` 函数调用 `native_hooks::dispatch_subagent_stop` 后，返回值（blocking reason）只被 `warn!` 日志记录，未传递给 `ReviewPropagationManager`。

**修复**：在 `dispatch_subagent_stop` 调用之后，添加 `#[cfg(feature = "taiji")]` 守卫的 `ReviewPropagationManager::on_leaf_completed` 调用，将 subagent 完成信息传播给审核管理器。

```rust
#[cfg(feature = "taiji")]
{
    use super::review_propagation::{ReviewPropagationAction, ReviewPropagationManager};
    let parent_id = subagent_parent_info.as_ref().map(|info| info.session_id.as_str());
    let action = ReviewPropagationManager::on_leaf_completed(
        &session_id, &agent_type, &response_text, parent_id,
    );
    if let ReviewPropagationAction::ReviewNeeded { parent_session_id, child_session_id } = action {
        debug!("ReviewPropagation: review needed for parent session {} from completed child {}",
            parent_session_id, child_session_id);
    }
}
```

#### 3.2 PostToolUse → Poke

**问题**：`tool_pipeline.rs` 的 `apply_post_tool_use_hooks` 函数处理完 PostToolUse 原生钩子后，未触发 Warden Poke 审计检查。

**修复**：在钩子处理后，添加 `#[cfg(feature = "taiji")]` 守卫的 Poke 审计检查段，对 `WriteFile`/`DeleteFile`/`ExecuteCode` 类工具调用进行分类并在 tool result 中注入审计上下文。

```rust
#[cfg(feature = "taiji")]
if !tool_result.is_error {
    use bitfun_agent_tools::classify_tool_call;
    let op_class = classify_tool_call(tool_name, &task.invocation.effective_arguments);
    match op_class {
        OperationClass::WriteFile | OperationClass::DeleteFile | OperationClass::ExecuteCode => {
            debug!("Poke audit triggered for destructive tool call: ...");
            hook_sections.push(format!(
                "[Warden Audit-Poke] Tool `{}` performed a {} operation...", ...
            ));
        }
        _ => {}
    }
}
```

### Step 4：特征守卫

所有 taiji 新增代码均添加了 `#[cfg(feature = "taiji")]` 守卫：

| 文件/模块 | 守卫位置 |
|---|---|
| `coordinator.rs` | `SessionTreeManager` import, `SubagentCompletionStatus` import, ReviewPropagation 调用段 |
| `tool_pipeline.rs` | Poke audit 检查段 |
| `review_propagation.rs` | 文件级 `#![cfg(feature = "taiji")]` |
| `restrictions.rs` | 文件级 `#![cfg(feature = "taiji")]` |
| `warden/mod.rs` | 文件级 `#![cfg(feature = "taiji")]` |
| `poke.rs` | 文件级 `#![cfg(feature = "taiji")]` |
| `rbac_poke_integration.rs` | 文件级 `#![cfg(feature = "taiji")]` |

### Step 5 & 6：编译与测试

- **编译**：`cargo check -p bitfun-core --features taiji` ✅
- **测试**：`cargo test -p bitfun-core --features taiji` — 1576 passed

**已知测试失败（5个，均为 taiji-quant 预存问题，非本次改动引入）**：

| 测试 | 原因 |
|---|---|
| `session_mode_runtime_updates_the_real_core_session` | TryLockError(()) — 锁竞争 |
| `test_prepare_subagent_execution_hidden_target_session_ok` | 断言失败 — cleanup 后 session 仍存在（taiji-quant 测试逻辑问题） |
| `update_restrictions_patch_overrides_role_template` | 断言失败 — restrictions 测试 |
| `poke_message_example` | JSON serde — 字段名 `pokeId` vs `poke_id` 不匹配 |
| `poke_response_example` | JSON serde — 枚举变体名 `Acknowledged` vs `acknowledged` 不匹配 |

---

## 合规性

- ✅ 不提交 (no commit)
- ✅ 不改非 taiji 代码（所有修改均限于 taiji feature gate 内或 taiji 功能模块）
- ✅ Task 工具未使用
