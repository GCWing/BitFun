# Phase 5-7 Batch A 方法论分析报告

> 分析对象：2026-07-21 批次 A 的 12 个 session（01-12）
> 分析方法：SessionHistory 全量导出 + 三个并行子任务 + 主干直读关键 turn
> 分析时间：2026-07-22

---

## 一、Session 任务总览

| Session | 名称 | 核心任务链 | 相位跨度 |
|---------|------|-----------|---------|
| 01 (3c0083fd) | 1161行 | Hook回滚 → Phase4 Recon(stolgo) → R4.24 → Phase5 Recon(C4) → R5.0 → 创建Phase6派发文档 | P4→P5→P6 |
| 02 (ff130da0) | 882行 | taiji-engine骨架 → Phase4 Recon(TTS) → R4.22验证 → Phase5 Recon(X1衔接) → R5.1 → 创建Phase6类型契约 | P4→P5→P6 |
| 03 (09d89d12) | 604行 | RiskMonitor trait → Phase4 Recon(多平台发布) → Phase5 Recon(B4国际化) → R5.2 → R6.0(taiji-llm) | P4→P5→P6 |
| 04 (c211ca4e) | 425行 | TickData类型 → Phase4 Recon(K线渲染) → Phase5 Recon(B3 SEO) → R5.3(TaskDag) | P4→P5 |
| 05 (47c9a201) | 732行 | bar/signal/state类型 → Phase4 Recon(pa-agent) → R4.4(K线模板) → R5.4(taiji-alert) → R6.1(ExecutionBridge) | P4→P5→P6 |
| 06 (cf3b3b60) | 672行 | Kahn拓扑排序 → Phase4 Recon(Cron调度) → R4.16(Legion模板) → Phase5 Recon(X2分发) → R5.5(TeachingChart) → R6.12(情绪分析) | P4→P5→P6 |
| 07 (87be6453) | 760行 | Python绑定骨架 → Phase4 Recon(Canvas) → R4.12(视频工作室) → R5.6(教学Canvas) → R6.2(实时数据) | P4→P5→P6 |
| 08 (51f7d64a) | 779行 | ComputeNode trait → Phase4 Recon(biliup) → R4.26+27(Phase5前期调查) → Phase5 Recon(C5编排) → R5.7(知识图谱) → R6.11(异常检测) | P4→P5→P6 |
| 09 (eba3f415) | 540行 | StateStore实现 → Phase4 Recon(vibe安全) → Phase5 Recon(B2报告发布) → R5.8(测验MiniApp) → 紧急修复(Signal.disclaimer) | P4→P5 |
| 10 (93cbc7e8) | 1093行 | DataSource trait → Phase4 Recon(vibe工具注册) → R4.15(视频生成命令) → Phase5 Recon(C3看板) → R5.9(Hugo脚手架) → R6.9(RL环境) | P4→P5→P6 |
| 11 (3ce5c68c) | 572行 | identity cache → Phase4 Recon(MiniApp架构) → Phase5 Recon(B1 SSG对比) → R5.11(博文生成) → R6.10(模式匹配) | P4→P5→P6 |
| 12 (6befdd9d) | 962行 | NodeFactory → Phase4 Recon(视频方案) → R4.23(文档同步) → Phase5 Recon(C6运维) → R5.10(Markdown生成) → R6.3(合规免责) | P4→P5→P6 |

### 1.1 总体统计

- **总行数**：10,501 行转录
- **总 turn 数**：约 110+ turns（含 /compact）
- **Phase 4 Recon**：12 次（每个 session 恰好 1 次）
- **Phase 5 Recon**：12 次（每个 session 恰好 1 次）
- **Phase 4 实现（R4.x）**：7 次
- **Phase 5 实现（R5.x）**：12 次
- **Phase 6 实现（R6.x）**：7 次
- **Meta 文档创建**：2 次（Phase 6 派发提示词 + 类型契约）

---

## 二、Commander→Agent 派发模式的五层结构

从 12 个 session 的 50+ 个用户消息中，提取出 Commander 的 5 种派发格式：

### L1：完整代码嵌入（最高确定性）
Commander 给出完整 Rust/Python/TOML 代码，Agent 只需写入文件 + 验证编译。

**示例**：Session 02 Turn 0（taiji-engine 骨架 — 完整给出 Cargo.toml + 9 个模块的代码）

**特征**：
- 任务命中率 100%（零协商往返）
- 代码由 Commander 预先编写
- Agent 角色 = 写入器 + 编译验证器

**适用场景**：骨架创建、类型定义、简单 trait

### L2：关键签名 + 验收（高确定性）
Commander 只给出 trait 签名 / struct 字段 / 核心 API 定义，Agent 自行实现细节。

**示例**：Session 03 Turn 0（RiskMonitor trait — 给出 6 个配套类型的签名，Agent 实现检查逻辑）

**特征**：
- Agent 有实现自由度但语义被签名锁定
- 验收标准明确（编译通过 + 测试通过 + 行为正确）
- "先读取类型契约确认签名"纪律

**适用场景**：模块实现、trait 实现

### L3：结构化派发（中确定性）
Commander 给出文件路径、操作步骤、验收标准、参考文件列表。代码由 Agent 自行编写。

**示例**：Session 03 Turn 8（R6.0 taiji-llm — 标记优先级 P0、依赖无、参考侦察 A1 + Plan 决策 1，给出核心签名）

**Phase 6 的增强**：Phase 6 的 L3 派发词加入了标准化的元数据块：
```
**优先级**：P0/P1/P2
**依赖**：无 / R6.5（回测引擎）
**参考**：侦察 B3 + Plan 决策 11
```

**适用场景**：Phase 6 大规模执行、跨语言任务

### L4：侦察派发（低确定性）
Commander 给出搜索关键词 / 分析目标 / 问题列表 / 产出格式，Agent 调研后回答。

**示例**：Session 01 Turn 17（stolgo ATR 止损侦察 — 给文件路径 + 3 个具体问题）

**两种子格式**：

| 子类型 | 关键词 | 产出 |
|--------|--------|------|
| Phase 4 Recon | "只读探查。到\<外部路径\>" | 代码适应性分析 |
| Phase 5 Recon | "搜索关键词：\<terms\>" | 技术选型推荐（关键发现+推荐+备选+风险，≤ 1 页） |

**适用场景**：技术选型、外部代码分析、可行性评估

### L5：Meta 文档生成
Commander 给出文档结构（严格参照已有格式）、参考文件链（Phase 6 Plan → Phase 5 Dispatch Prompts → 20 Recon Reports）、验收标准。

**示例**：Session 01 Turn 25（Phase 6 派发提示词文档 — 2518 行）

**特征**：
- "严格按 X 格式"是核心约束
- 参考文件链可追溯 3-4 层
- 验收标准可自动校验（R-ID 覆盖率、路径正确性）

**适用场景**：跨 Phase 的派发文档、类型契约文档

### 派发模式演变总图

```
确定性
  ↑
  │ L1 ──────────────── L1 ──────────────── L1 （骨架创建）
  │        L2 ──────────── L2 ──────────── L2  （trait 实现）
  │               L3 ──────────── L3 ───── L3  （模块实现）
  │                      L4 ──────── L4 ── L4  （侦察调研）
  │                            L5 ────── L5 ── （Meta 文档）
  └──────────────────────────────────────────────→ Phase
       P4           P5           P6
```

**关键洞察**：Commander 的任务确定性感知极强。确定性高的任务（创建骨架、硬编码配置）用 L1 嵌入完整代码；不确定性高的任务（技术选型、外部调研）用 L4 只给问题不给答案。Phase 6 的任务更复杂（跨语言、多 crate），但元数据标准化（优先级/依赖/参考）让 Agent 能自主执行。

---

## 三、跨 Session 的协调与通信机制

### 3.1 四种协调机制

| 机制 | 载体 | 作用 | 例子 |
|------|------|------|------|
| **文件系统** | `src/crates/taiji/taiji-*/` 下的 Rust 代码 | 共享代码状态 | Session 09 的 StateStore 被 Session 12 的 R6.9 RL 环境读取 |
| **类型契约** | `.bitfun/team/type-contract*.md` | 接口签名权威来源 | 12 个 session 的 turn 0 全部引用 type-contract.md |
| **Phase Plan** | `docs/plans/phase*-plan.md` | 排期共识 + 决策文档 | R-ID 引用格式如 "Phase 4 Plan v1.1.0 §四 R4.24" |
| **R-ID 序列** | 全局编号（R4.22, R5.0...） | 任务依赖的隐式拓扑 | Commander 按 R-ID 顺序跨 session 分派，Agent 只需看到自己的 R-ID |

### 3.2 跨 Session 依赖的三种模式

**模式 A：类型前向依赖（常规）**
```
Session N 创建类型/trait → Session N+1 引用该类型
```
示例：Session 09 StateStore → Session 12 RL 环境

**模式 B：反向开孔（回退修改）**
```
Session N+1 需要访问 Session N 的私有字段 → 回退修改 Session N 的 crate
```
示例：Session 12 R6.9 需要 Pipeline 暴露 `state_store_arc()`，回退修改 `pipeline/mod.rs`

> **教训**：共享核心类型（StateStore、Pipeline、Signal）应在设计时就预暴露公共只读访问 API。每次后来者需要"开孔"时改一次，累计浪费的编译迭代远超提前暴露的成本。

**模式 C：字段变更级联（跨 Session 修复）**
```
Session N 给共享 struct 新增字段 → Session N 修复可见 crate
                                     → Session N+M 修复被遗漏的闭源 crate
```
示例：Session 15 R6.3 给 `Signal` 加 `disclaimer` 字段 → 修复 3 个 crate → Session 09 紧急修复剩余 5 个 crate

> **教训**：共享 struct 增字段需要 workspace 级 `grep 'Signal {'` 全量验证。单 session 内不可能覆盖所有构造点（闭源 crate 不在当前编译单元的首轮检查范围内），需要建立"字段变更 checklist"流程。

### 3.3 R-ID 序列作为隐式拓扑

Commander 用 R-ID 序列管理依赖，Agent 不需要知道其他 session：

```
Phase 4 并行线：
  Session 03: RiskMonitor trait
  Session 09: StateStore
  Session 12: NodeFactory
  Session 06: DAG

Phase 5 并行线：
  Session 02: R5.1 (WebsitePublisher)
  Session 03: R5.2 (taiji-teach)
  Session 04: R5.3 (TaskDag)
  Session 08: R5.7 (知识图谱)

Phase 6 提前启动 + 并行：
  Session 03: R6.0 (taiji-llm, P0)
  Session 12: R6.9 (RL environment, P1)
  Session 06: R6.12 (情绪分析, P2)
```

**关键洞察**：Phase 4 的类型层是纯并行（无互相依赖）；Phase 5 开始出现跨 crate 引用；Phase 6 需要在 Phase 5 类型就绪后开始，但 P0 任务可以提前在 Phase 5 执行期启动。

---

## 四、每 Session 的三段式结构

所有 12 个 session 都遵循严格的三段式节奏：

```
┌──────────────────────────────────────────────────────┐
│ Turn 0:    基础类型 / trait / 骨架（锚点层）         │
│            1-3 个文件，编译验证通过                   │
│            确定性最高，多为 L1/L2 格式                │
├──────────────────────────────────────────────────────┤
│ Turn 2:    Phase 4 Recon（侦察层）                   │
│            外部代码分析 / 技术可行性评估               │
│            L4 格式，产出调查报告                      │
├──────────────────────────────────────────────────────┤
│ Turn 4+:   R4.x / R5.x / R6.x 实现（执行层）         │
│            跨 Phase 的执行任务                        │
│            确定性随 Phase 递增降低                    │
└──────────────────────────────────────────────────────┘
```

**Turn 0 的锚点作用**：
- Session 02 Turn 0：taiji-engine crate 骨架（13 个子模块）→ 所有后续 session 的代码都写入这个 crate
- Session 03 Turn 0：RiskMonitor trait → R5.4（taiji-alert）使用它做风控检查
- Session 12 Turn 0：NodeFactory → R6.11 的 5 个异常检测节点通过 factory 注册

锚点层确保每个 session 的第一步产出是"可被后续引用"的类型/trait，避免空转。

---

## 五、参考文件链的标准化演进

从 12 个 session 追踪 Commander 的"先读取"声明，发现参考链从扁平到多级：

### Phase 4 早期：单级引用
```
type-contract.md → 代码
```

### Phase 4 中后期：二级链
```
type-contract.md → Phase 4 Plan (§ 章节号 + R-ID) → 代码
```

### Phase 5：三级链形成
```
侦察报告 (docs/plans/phase5-recon/XXX.md)
  → Phase Plan 决策 N
    → 现有平台代码（先读取确认 API）
      → 新代码实现
```

### Phase 6：压缩链
```
侦察编号 (B3) + Plan 决策编号 (Plan 决策 11)
  → 类型签名确认（先读确认）
    → 新代码实现
```

**规律**：参考链的长度与任务不确定性正相关。Phase 4 早期任务（创建类型）确定性高，只需确认签名 → 直接写入。Phase 6 任务（智能交易引擎）不确定性高，需要追溯到 3 层前的侦察报告确认决策依据。

---

## 六、/compact 的六种用法

12 个 session 中 `/compact` 出现约 50 次。分析每次出现前后的 turn 类型：

| 用法 | 频率 | 模式 |
|------|------|------|
| **上下文清理** | 最常见 | 大任务完成后，释放 token 空间给新任务 |
| **相位切换** | 常见 | Phase 4 Recon → Phase 5 Recon 之间必然会 /compact |
| **类型切换** | 常见 | Recon（调研）→ Execute（编码）之间 |
| **人类审批点** | 常见 | Agent 完成交付后，Commander 确认无误再 /compact 继续 |
| **错误恢复** | 少见 | 调试/编译问题解决后清理错误上下文 |
| **元任务边界** | 少数 | 从实现任务切换到 Meta 文档创建任务 |

**核心规律**：`/compact` 的实质是 **token 预算管理**，但其副作用是充当了**人类-in-the-loop 审批检查点**。每个 `/compact` 都是 Commander 在说"这个阶段的产出我认可了，上下文可以清，开始下一阶段"。

---

## 七、侦察任务的标准化路径

### Phase 4 Recon（代码分析型）
```
格式：[Phase 4 Recon: 主题 — 子标题]
方法：只读探查外部代码库
问题：3-5 个具体技术问题
产出：直接回复调查报告（自由格式）
```

### Phase 5 Recon（技术选型型）
```
格式：任务：Phase 5 侦察 编号——标题
方法：搜索关键词（Web Search + 代码库搜索）
产出：docs/plans/phase5-recon/XXX.md
      格式：关键发现 + 推荐方案 + 备选方案 + 风险
      限长：≤ 1 页
      纪律：只侦察不执行
```

### 侦察管理制度（R4.26 + R4.27 建立）
- `phase5-recon/README.md` — 侦察任务清单
- `phase5-recon-index.md` — 已产出报告的状态跟踪表
- 每个侦察报告分配唯一编号（如 C5、X2），供后续实现任务引用

### Recon → Execute 递进保证
**Recon 永不空转**。每个侦察报告都在后续的至少一个 Execute 任务中被引用。Commander 只在有明确执行计划时才发起 Recon。

---

## 八、任务粒度与测试密度的演变

| 指标 | Phase 4 | Phase 5 | Phase 6 |
|------|---------|---------|---------|
| 最小任务 | 1 个文件 | 2-3 个文件 | 4-5 个文件 |
| 最大任务 | 3 个文件 + Cargo.toml | **8 新建 + 6 修改**（R5.7 知识图谱） | 8 源文件 + 38 测试（R6.11） |
| 平均测试数/任务 | 8-12 | 6-20 | 12-38 |
| 跨 crate 频率 | 低（大部分单 crate） | 中（2-3 crate） | 高（3-6 crate） |
| 跨语言频率 | 零 | 低 | 中（PyO3、JS 前端） |

**趋势**：
1. 任务粒度从"1 个文件"线性增长到"8 个文件 + 38 个测试"
2. 测试密度从"任务粒度 / 2"增长到"任务粒度 × 4"
3. 跨 crate 修改比例从 ~10% 增长到 ~60%
4. 跨语言任务仅在 Phase 6 出现（PyO3 Rust↔Python 桥接）

---

## 九、紧急修复模式

从 Session 09 Turn 8 的"Signal.disclaimer 紧急修复"提取完整工作流：

```
触发 → 诊断（grep 全量构造点）→ 分类（区分缺失/已有）
  → 并行执行（4 编辑）→ 验证失败（闭源 crate 被遗漏）
    → 补漏（重新 grep）→ 验证通过
```

**关键教训**：
1. Windows PowerShell 下 `cargo check` 的退出码可能误报（实际成功但 `$LASTEXITCODE = 1`）
2. 单次 grep 不可信——需在 cargo check 失败后做二次确认
3. 治本方案是"字段变更 checklist"——修改共享 struct 时建立 workspace 级自动检查

---

## 十、Phase 6 任务复杂度的三层叠加

Phase 6 任务（R6.x）的复杂度来自三种叠加：

### 第一层：跨 crate 依赖
R6.3 合规 → 修改 6 个文件跨越 4 个 crate
R6.9 RL 环境 → 回退修改已有 crate 暴露 accessor

### 第二层：跨语言边界（PyO3）
R6.9 RL 环境 → Rust↔Python 桥接需要处理：
- `#[pyclass]` + `PyRef` + `Bound<'_, PyModule>` 版本差异
- pyo3 cdylib 测试在 Windows 上需要 Python DLL 在 PATH
- 3 个新 pyclass 需在 `lib.rs` 显式注册

### 第三层：算法实现复杂度
R6.10 模式匹配 → 手写多维 DTW + LB_Keogh + Sakoe-Chiba 带约束
R6.11 异常检测 → 5 个独立指标 ComputeNode + 融合评分卡

**建议**：Phase 6 的 PyO3 任务应预留更多编译迭代预算（10+ vs 正常的 3-5 次）

---

## 十一、可复用的方法论模式总结

| # | 模式 | 来源 | 可复用性 |
|---|------|------|---------|
| 1 | **三段式 Session 结构**：锚点层→侦察层→执行层 | 全部 12 session | 所有 session 设计必须遵循 |
| 2 | **五层派发格式**：L1(代码嵌入)→L2(签名+验收)→L3(结构化)→L4(侦察)→L5(Meta) | 全部 commands | 按任务确定性选择层级 |
| 3 | **先读后做**：读取类型契约→读取现有代码→读取侦察报告→实现 | 100% 任务遵守 | 非可选纪律 |
| 4 | **Recon 不空转**：每个侦察报告在后续 Execute 中必须被引用 | Phase 4-5 | 避免调研浪费 |
| 5 | **参考链标准化**：侦察编号→Plan决策编号→类型签名确认→实现 | Phase 5-6 | 可追溯决策依据 |
| 6 | **R-ID 全局序列**：用编号管理跨 session 任务拓扑 | 全部 Phase | 避免任务遗漏 |
| 7 | **Meta 文档先行**：Phase 6 派发文档在 Phase 5 中期开始准备 | Session 01, 02 | 文档准备好后再大规模执行 |
| 8 | **紧急修复 checklist**：共享 struct 增字段 → workspace 级 grep → 全部修复 | Session 09, 15 | 避免二阶段修复 |
| 9 | **共享核心类型预暴露 API**：StateStore/Pipeline/Signal 设计时就加公开访问器 | Session 12 反向开孔教训 | 减少回退修改 |
| 10 | **侦察报告四段式**：关键发现+推荐方案+备选方案+风险，≤1页 | Phase 5 Recon | 保持统一模板 |
| 11 | **P0 提前启动**：Phase 6 P0 任务在 Phase 5 执行期就启动 | Session 03 R6.0 | 缩短关键路径 |
| 12 | **人类-in-the-loop 审批**：/compact 作为 Commander 的阶段性确认 | 全部 session | 每个任务后保留确认 |

---

## 十二、Commander 能力模型

从 50+ 个派发消息中反推 Commander 的核心能力：

1. **任务确定性感知**：能准确判断一个任务是"我可以直接写代码"还是"需要 Agent 调研"
2. **相位推进**：知道什么时候 Phase 4 收尾、什么时候 Phase 5 可以并行启动、什么时候开始准备 Phase 6 文档
3. **R-ID 拓扑编排**：知道哪些 R-ID 可以并行（无依赖）、哪些必须串行（有依赖）
4. **文档先行意识**：在 Phase 5 执行期就开始准备 Phase 6 的派发文档和类型契约
5. **侦察→执行闭环**：发起的每个侦察都有后续的执行任务消费，不空转
6. **质量控制**：通过 /compact 在每个任务边界做阶段性审批
7. **紧急响应**：能快速诊断编译失败、定位遗漏文件、发起紧急修复
