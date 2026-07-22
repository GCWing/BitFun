# Vibe Coding 自动化长任务工作流 —— 从 Phase 1 到 Phase 10 的完整实战

> 三省三域（公司·军事·航天）方法论 + BitFun SessionMessage 编排引擎
>
> 全部代码零手工编写，两天完成 272 文件、43,355 行、400+ 测试。

---

## 一、核心方法论：99% 准备 + 1% 执行

```
┌──────────────────────────────────────────────────────────────┐
│                   99%：准备阶段                               │
│                                                              │
│  Step 1: 前期调查（替补池全量并行侦察）                       │
│  Step 2: 三文档规划（Plan + Type-Contract + Dispatch-Prompts）│
│  Step 3: 深度审计（执行路径追踪 + 跨文档交叉审计 + 缺口登记） │
│  Step 4: 下一阶段前期调查                                    │
├──────────────────────────────────────────────────────────────┤
│                   1%：执行阶段                                │
│                                                              │
│  Step 5: 拓扑派发（SessionMessage 异步，Gate Loop 逐波闭合）  │
│  Step 6: 审查阶段（交叉审查 + Bug 修复 + 全量测试）           │
│  Step 7: 终审阶段（代码审查 + 文档同步 + R-ID 闭合 + 质量门） │
├──────────────────────────────────────────────────────────────┤
│                   闭环                                        │
│                                                              │
│  Step 8: 学习沉淀（经验提取 → 铁则映射 → 技能更新）           │
└──────────────────────────────────────────────────────────────┘
```

---

## 二、12 条铁则（每步操作的前置检查）

| # | 铁则 | 域 |
|---|------|-----|
| 1 | 不编造，不猜测，不确定就问。不附和奉承。 | 航天 |
| 2 | 测试先写。以参考源输入→输出为基准。测试不过不提交。 | 航天 |
| 3 | 一命通关。99% 工作在准备阶段。执行出问题 = 回退根因重来。 | 航天 |
| 4 | R-ID 贯穿全链。需求→原子分解→派发→执行→审查→验收，每步带 R-ID。 | 公司 |
| 5 | 战至一兵一卒。数据源断切备源，节点崩降级重组，进程死快照恢复。 | 军事 |
| 6 | 防呆设计：把用户当傻子。空列表不 crash，单元素不除零。 | 军事 |
| 7 | 参考库文件不得删除或覆盖。 | 军事 |
| 8 | 同一方法失败两次→停→换思路。 | 军事 |
| 9 | 零硬编码。全部配置驱动（YAML/JSON）。 | 公司 |
| 10 | 先参后改。每个模块先找参考源码→逐行理解→标注出处→再适配。 | 航天 |
| 11 | 改动前先 Read。不读不改。 | 航天 |
| 12 | 分工纪律。指挥官审查方案+派发任务。写代码交给编码 Agent。 | 公司 |

---

## 三、Phase 1-10 实战路线图

### Phase 1-3：太极交易引擎核心（34 R-ID）

**场景**：从 Python 参考实现（czsc）移植量价时空交易算法到 Rust。

**方法**：
- Python→Rust 移植五步法：读源文件精确行范围 → 逐行对比标注差异 → 解决依赖 → 写实现 + 至少 3 个测试 → 编译验证
- 五层派发模型：按任务确定性选择派发格式（L1 完整代码嵌入 → L5 Meta 文档生成）

**实战案例**：`find_pivots_raw` 移植

```
1. 侦察 Agent 读取 Python 源文件精确行范围，标注每个变量和分支
2. Commander 写入 type-contract（Pivot struct + ComputeNode trait 签名）
3. 派发 Execute Agent：输入签名 + 参考行号 → 产出 Rust 实现
4. Review Agent 逐行对比 Python vs Rust，发现 key 格式不一致（bars:1m vs bars:F1）
5. 修复 → cargo test 通过 → R-ID 闭合
```

**成果**：taiji-engine、taiji-bar、taiji-dvmi 核心逻辑，59 测试通过。

---

### Phase 4：内容工坊（36 替补 Session 并行侦察）

**场景**：7 大技术领域（CTP 执行、回测、风控、视频渲染、TTS、多平台发布、定时调度）需要全面调研。

**方法**：替补池全量并行侦察法
1. 列出所有待调研领域 + 参考项目 + 外部 API
2. 从 session 池选取空闲 session，每 session 派发一个只读探查任务
3. 全部并行运行，Commander 不干预
4. 侦察报告标注编号（#1-#N），后续规划 R-ID 溯源

**成果**：36 个 session 并行返回高质量侦察报告，三文档规划体系（Plan + Type-Contract + Dispatch-Prompts）固化。

---

### Phase 5：规划工程化（大文档并行拆分）

**场景**：Phase 5 三文档规划总计 ~3000 行。单 session 派发时上下文溢出（两个 session 在读取阶段死亡）。

**方法**：三文档并行拆分 + 嵌入模板免读大文件
- Plan / Type-Contract / Dispatch-Prompts 各一个 session
- 模板结构直接嵌入派发消息，session 无需 Read 外部文件
- 跨文档审计：8 维度对照找缺口（发现 20 个缺口，4 个严重）

**实战案例**：type-contract session 在 3 个 session 死亡后，第四轮采用"嵌入模板"策略，成功产出 1612 行类型契约。

---

### Phase 6-7：大规模执行（24 Session 分批并行）

**场景**：20 个 crate 的实现阶段。同层无依赖→全并行；跨层有依赖→提前启动 P0 缩短关键路径。

**方法**：R-ID 隐式拓扑编排
- Commander 用 R-ID 序列管理跨 session 依赖拓扑
- Agent 不需知道其他 session 的存在
- 三段式 Session 结构：Turn 0 锚点（可被引用的类型/trait）→ Turn 2 侦察 → Turn 4+ 执行

**实战案例**：Phase 6 P0 任务（taiji-engine 核心 trait）在 Phase 5 执行期提前启动，缩短关键路径 30%。

---

### Phase 8：Rebase 适配（main 基线迁移）

**场景**：main 从 bf0b05765 前进到 ee9996436。需要将所有 taiji 改动迁移到新基线。

**方法**：Rebase 工作流 + SessionMessage 异步派发
- `git checkout -b taiji-vN origin/main`（干净新分支）
- 纯新增目录（src/crates/taiji/）批量 checkout → 零冲突
- 集成文件（53 个）逐文件迁入 → 遇冲突手动合并
- 全量回归：cargo check + test + clippy + fmt

**成果**：零冲突迁移，28 R-ID 闭合。固化为一键脚本 `scripts/migrate-taiji.ps1`。

---

### Phase 9：安全审查（8 维度全并行）

**场景**：20 个 crate 需要 CSO 级安全审查。

**方法**：
- Wave 1（并行审查）：8 维度全并行，全部只读，无文件冲突
- Wave 2（串行修复）：P0 优先 → P1 其次，按风险等级 Gate 逐波闭合

**实战案例**：100+ 条告警 → 根因收敛 → 4 条根因修复。验证形式化陷阱：测试通过 ≠ 行为正确 → 额外路径追踪审计。

**成果**：10 项 P0 全部修复，SECURITY.md 新增 Taiji 安全策略。

---

### Phase 10：质量门 + GitHub 上架

**场景**：代码写完后的最终交付验证。

**质量门清单**（缺一不可）：
| 门 | 命令 | 标准 |
|----|------|------|
| 编译 | `cargo check --workspace` | 0e 0w |
| 测试 | `cargo test` (19 crates) | 全部通过 |
| Clippy | `cargo clippy -- -D warnings` | 0 warning |
| 格式 | `cargo fmt --check --all` | passed |
| 安全 | Phase 9 报告 | P0 全部闭合 |

**成果**：154 测试通过，0 clippy warning，4 项 CI job 上线。

---

## 四、Ultra Mode：已实现

BitFun 后续规划的 "ultra 模式"——自主多 Agent 编排 + 质量门闭环——本 PR 已实现并实战验证：

| Ultra 模式能力 | 本 PR 实现 | 文件 |
|---------------|-----------|------|
| 军团编排引擎 | `LegionControl` 工具：JSON 模板 → Kahn 拓扑排序 → 分层 Session → 并行派发 | `legion_control_tool.rs` |
| 异步任务派发 | `SessionMessage`：真正的异步推送，零轮询 | 全链路 |
| 质量门闭环 | Gate Loop Protocol：Dispatch → Collect → Inspect → PASS/FAIL → Correct | scheduler + team_mode |
| 静默检测 | Goal Continuation：双条件（队列空 + 无活跃 Session） | `scheduler.rs` |
| 容错重组 | 连败 2 次→停→换人；panic 隔离；数据源断切备源 | 铁则 5 + 铁则 8 |
| 拓扑排序 | Kahn 算法自动分层，同层全并行 | `dag.rs` |

所有 commit 即成果。272 文件、43K 行、400+ 测试、0 warning——全部通过 Vibe Coding 在两天内完成。
