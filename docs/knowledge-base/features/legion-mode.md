# 军团编排系统（Legion Mode）

> **涉及文件**: BeeColonyMonitor.tsx, CreateLegionPage.tsx, LegionCard.tsx, orchestration-patterns.ts
> **核心模块**: assembly/core → agentic → agents → definitions → subagents
> **更新日期**: 2026-07-25

---

## 概述

Legion（军团）模式是 BitFun 的高级 Agent 编排系统——将多个 Agent 按拓扑结构组织为"军团"，实现复杂的多 Agent 协作工作流。

## 架构

```
LegionControl Tool
├─ 读取 JSON 模板（军团模板定义）
├─ Kahn 拓扑排序（确定执行顺序）
├─ 分层创建 Session
├─ 注入上下文
└─ 并行/串行派发

军团模板:
├─ bee-colony-standard    (8 节点全链)
├─ bee-colony-quick       (4 节点精简)  
├─ bee-colony-parallel    (12 节点 3 并行)
└─ bee-colony-single      (单执行者)
```

## 审查机制（cca-haha 模式）

```
每轮 LLM 审查 → 结果注入下一轮
├─ 书记官（Secretary）: 上下文压缩/恢复
├─ 纪律委员（Disciplinarian）: ABORT/WARN 决策
└─ 提示蜂（Prompter）: SKILL 推荐
```

## 基础 Agent 角色

| 角色 | 英文名 | 职责 |
|------|--------|------|
| 指挥官 | Commander | 派发、协调、审查 |
| 书记官 | Secretary | 上下文管理、日志 |
| 产品经理 | Product Manager | 需求分析 |
| 规划师 | Planner | 任务分解 |
| 执行者 | Executor | 代码/文件操作 |
| 审查员 | Reviewer | 交叉审查 |
| 验收官 | Acceptor | 终验 |
| 优化师 | Optimizer | 性能优化 |

## 军阶体系（Ranks）

军团成员分为 9 级军阶，从 `R-09 候蜂` 到 `R-01 蜂王`，每级有不同的权限、技能要求和处罚力度。

> 详见 `docs/plans/legion-ranks-design.md`
