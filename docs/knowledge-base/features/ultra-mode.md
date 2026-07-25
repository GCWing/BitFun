# Ultra / Deep 多 Agent 模式

> **涉及 crate**: assembly/core, execution/agent-runtime, web-ui
> **核心变更**: ultra-mode 14 修复（commit 315482715）
> **更新日期**: 2026-07-25

---

## 概述

Ultra 模式是 BitFun 的高级 Agent 协作模式——允许在单个对话中派生子 Agent 并行工作，并在父 Agent 与子 Agent 之间建立深度审查链。

## 核心概念

```
用户对话 (Session)
  └─ Ultra 子对话 (Sub-session)
       ├─ Turn 1: 子 Agent 执行
       ├─ Turn 2: 子 Agent 继续
       └─ ...
       └─ Deep Review 模式
            ├─ 子 Agent → 父 Agent 传播
            └─ 审查队列状态管理
```

## 架构组件

### 1. Session 树管理

```rust
// services-core/src/session/tree.rs
pub struct SessionTreeManager {
    // 管理父 Session → 子 Session 的关系树
    // 支持超时清理（5 秒无活动自动回收）
    // 支持深度限制（Ultra 链层次）
}
```

### 2. Ephemeral Subagent

```
轻量子 Agent 模式，不持久化到数据库。
- 由 DelegationPolicy 控制是否持久化
- 适用于一次性子任务
- 生命周期与父对话绑定
```

### 3. 深度审查（Deep Review）

```
子 Agent 完成 → 审查队列
  ├─ DeepReviewQueueStateChanged 事件
  ├─ 父 Agent 轮询审查结果
  └─ ReviewPropagationNeeded 事件传播
       └─ 传播到父 Session 的 UI 层
```

### 4. 安全加固（14 项修复）

| 修复 | 文件 | 说明 |
|------|------|------|
| depth 值类型修复 | runtime.rs | AsyncSubAgentParams depth: u32 → correct type |
| session 关系修复 | tree.rs | parent_id/session_id 正确关联 |
| 持久化控制 | DelegationPolicy | Ephemeral vs Persistent 区分 |
| 安全审计 | events/agentic.rs | ReviewPropagationNeeded 枚举 |
| 前端适配 | web-ui/* | Ultra 模式 UI 显示 |
