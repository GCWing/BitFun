
# 22-Session 综合分析报告

**分析日期**: 2026-07-22
**覆盖范围**: 2026-07-19 至 2026-07-21，22 个 session
**分析类型**: 指挥官战略决策模式 + 早期 agentic session 知识提取

---

## 一、概览

| 分类 | Session 数 | 会话编号 |
|---|---|---|
| 指挥官（已有 rollout） | 6 | 46cc66b6, 42fbbe24, 4e65de2c, 3f58fb7c, 34863803, c4c501a0 |
| 指挥官（子代理分析） | 4 | 3b834f3c(44), f6276d09(42), cc881d14(43), e81509ad(45) |
| 批次 C 早期 agentic | 12 | 25-38（跳号32、35） |
| **合计** | **22** | |

---

## 二、指挥官战略决策模式（核心产出）

### 2.1 决策模式总览

通过对 10 个指挥官 session 的深度分析，识别出 **10 个战略决策模式**：

---

#### 模式 1：方案未定不执行

**来源**: 46cc66b6, 3b834f3c, e81509ad

**表现**：
- 用户明确拒绝 agent 在需求未对齐时就提出的 5-Phase 排期
- 口头禅："之前的进度全部清零，我方案都没定呢——你先明确我要什么再说"
- 在多个 session 中反复出现，是最核心的决策模式

**决策逻辑链**：需求对齐→确认目标→技术方案→分解→派发，前一步未闭合则绝不进入下一步

---

#### 模式 2：三版架构分层

**来源**: 3b834f3c (Turn 49)

**表现**：
- 太极 dev 版（热更新开发）→ SRC 稳定版（build 固化）→ 官方原版（参考对照）
- 用户纠正 agent：**"SRC 版也不是原版，是中间产物版"**

**架构意义**：三个独立工作区，各自有明确的用途和代码规范，互不污染

---

#### 模式 3：侦察驱动决策——不凭假设

**来源**: e81509ad, f09ed1e2, cc881d14

**表现**：
- BitFun CronService 文档声称存在但实际代码中不存在 → 立即侦察→评估影响→替代方案（TaskDag）
- stolgo 回测引擎侦察→确认架构对齐→决定 Backtest 引擎设计（BarDataView._limit 直接移植）
- 所有外部依赖假设必须在实现前通过代码侦察验证

**教训**：Phase 4 决策 6 中的 CronService 假设在 Phase 5 才被打破。结论：**侦察先行，不凭文档假设**

---

#### 模式 4：开源/闭源边界——Trait 分层设计

**来源**: e81509ad, 3b834f3c

**核心理念**："Engine defines WHAT (traits), Strategy defines HOW (impls)"

**表现**：
- 开源部分：所有 trait 定义、Pipeline 引擎、数据管道、通用算法
- 闭源部分：具体策略实现（Strategy impl）、量化参数、太极公式
- 移除闭源 crate 时注释保留路径，"加法不做减法"
- 文档全程英文，不含任何太极公式/策略引用

**架构意义**：trait 边界 = 知识产权隔离墙

---

#### 模式 5：精确指令制——不猜测、不自作主张

**来源**: 3b834f3c (多次), cc881d14

**铁则原文**："你他妈有什么资格猜测"（3b834f3c Turn 7）

**表现**：
- 构建 exe 问题时，agent 被要求直接读 `package.json` scripts 而非猜测 `--debug`/`--no-bundle` 组合
- cc881d14 中每个任务都含完整文件路径、代码示例、验证命令
- agent 发现 spec 中 API 与实际代码不一致时，主动适配并记录偏离原因（而非盲从或盲改）

**教训**：先读→后确认→再执行。任何一步未做实即被惩罚。

---

#### 模式 6：先规矩后干活——认知框架前置

**来源**: 3b834f3c (Turn 1/20/77)

**表现**：
- `master-framework` 技能必须**最先加载**，不可跳过
- 同一个 session 中 agent 3 次遗忘加载 → 每次都被严厉纠正
- 用户的核心行为准则："你连规矩都不学，做什么事？"

**违规后果**：多轮无效工作被"全部清零"的风险

---

#### 模式 7：文档先行——类型契约先于代码

**来源**: e81509ad, 34, f09ed1e2

**表现**：
- `type-contract-phase5.md`（1612 行）和 `ARCHITECTURE.md`（828 行）在**代码动工之前**创建
- Phase 5+6 三文档规划（plan + type-contract + dispatch-prompts）定稿后才派发执行
- 类型边界矩阵确保 28 个类型跨 crate 依赖和所有权明确，消除派发冲突

**核心理念**：类型契约是所有 agent 派发的基础合同

---

#### 模式 8：99% 准备 + 1% 执行

**来源**: 跨 session 的一致模式

**量化证据**：
- Phase 4 Recon 覆盖：stolgo 回测引擎、WonderTrader 全栈（DataMgr/CTP/CTAEngine/WtExecMon）、openctp API、Rust CTP 生态、Hugo/Zola SSG 选型
- Phase 6 Recon 覆盖：LLM 框架、多 Agent 辩论、历史回测、信号融合、CTP 执行接口、风险约束 Agent、WebSocket、延迟优化、实时推流、分布式回测、GPU 加速、策略热更新、管线衔接、交易合规——共 14 份侦察报告
- 侦察总量：25+ 份独立侦察报告

**与普通 agentic 的对比**：agentic session 中单个任务从 spec→实现 1-3 轮，而指挥官 session 中每个方向需 5-10 轮反复对齐后才进入执行

---

#### 模式 9：复用不拼装——算法引用 ≠ 框架依赖

**来源**: 46cc66b6, f09ed1e2, 3b834f3c

**表现**：
- ctp2rs 只是 FFI 绑定算法的**参考**，不是引入框架依赖
- WonderTrader 的 CTP 方向/开平映射表"直接可复用算法"
- 外部代码的价值是"抄算法，不是抄组装"

**边界**：参考代码用于理解"怎么做"（算法），但产品架构必须由太极自身的设计驱动

---

#### 模式 10：零容忍三角——猜测/警告/和稀泥

**来源**: 3b834f3c, fe996913, 46cc66b6

| 零容忍项 | 原文 | Session |
|---|---|---|
| 不猜测 | "你他妈有什么资格猜测" | 3b834f3c Turn 7 |
| 零警告 | "我的眼里容不得任何警告" | 3b834f3c Turn 55 |
| 不做和稀泥 | "必须给出明确裁决" | fe996913 Turn 6 |
| 不编造 | 恒等校验 + 数据不编造 | 34 |
| 不准杀自己进程 | "不准杀自己进程，第二次了" | 3b834f3c Turn 64 |

---

### 2.2 指挥官工作流状态机

```
                 +--违规: 回到对齐--+
                 ↓                   |
  [加载规矩] → [需求对齐] → [方案定稿] → [侦察验证] → [文档锁定] → [派发执行] → [审查审计]
       ↑            ↑           ↑            ↑            ↑            ↑           ↑
   铁则1-3     方案未定不执行   99%准备    侦察驱动    类型契约   精确指令    闭环审查
```

**关键转换条件**：
- 加载规矩→需求对齐：master-framework 加载完成且 10 棵决策树已遍历
- 需求对齐→方案定稿：用户确认"方案定了"，而非 agent 自行判断
- 方案定稿→侦察验证：所有外部依赖假设已验证（无文档幻觉）
- 侦察验证→文档锁定：type-contract 覆盖所有跨 crate 类型
- 文档锁定→派发执行：dispatch-prompts 为每个 R-ID 含完整 spec + 验证命令
- 派发执行→审查审计：所有测试通过 + cargo check 零错误 + cargo test 零失败

---

## 三、批次 C 早期 Session 知识汇总

### 3.1 技术领域覆盖

| 领域 | 涉及 Session | 关键产出 |
|---|---|---|
| DVMI 算法 | 25, 26, 27, 36, 38 | 趋势线修复、双线通道、退出信号、跨验证、property-based 测试 |
| 数据管道 | 27, 31, 37, 38 | CsvReplaySource、SchemaAdapter 字段映射、TickValidator |
| 回测引擎参考 | 25, 34, 37 | WonderTrader 全栈侦察、dvmi_backtest_v4 分析、CTP 数据流 |
| Agent 提示词 | 29, 33, 34 | 5 个 Agent 模板（risk/decision/delta/magnet/resonance）、四门决策树 |
| Tauri 命令 | 28, 31, 43 | feed_tick、feed_csv、dashboard、taiji_status、taiji_export |
| MiniApp | 33, 34 | 配置面板、预览进度面板、分发配置 |
| 视频管线 | 26, 36, 38 | VideoAsset、ComposeConfig、EncodingProfile、音画同步测试 |
| 教学管道 | 27, 34 | chapter_splitter、lecture_generator、content_indexer、Hugo SSG |
| 实时推流 | 33, 38 | KLineRenderer、LiveStreamEngine、WebSocket、RTMP |
| 订单流分析 | 34, 38 | VPIN、OFI、Welford 在线统计、市场微观结构 |
| Feature flags | 31 | Unleash、杀开关、AB 实验 |
| GPU 加速 | 30, 38 | candle 本地推理、RAG 语义嵌入 |
| 隐私合规 | 33 | 6 部法规 × 产品触达点映射 |
| 交易合规 | 38 | 6 条红线（无牌照不荐股、风险揭示强制等） |
| 全链路测试 | 27, 38 | E2E 集成测试（CSV→Tick→Bar→Signal 15875 ticks→13 signals） |
| 性能基准 | 37 | BarGenerator 吞吐量、DAG 拓扑、StateStore 读写 |
| 开源文档 | 29 | taiji-engine README + ARCHITECTURE.md（零太极公式） |

### 3.2 高价值可复用知识

#### DVMI 算法（2 Critical bugs fixed）
- **str_vals W-step diff bug**: Python `diff()` 无参=1-step，Rust 实现成 W-step → 算法错误
- **compute_six_core bar 覆盖 bug**: `execute_dag` 用 `set()` 覆盖而非追加，永远只有 1 根 bar
- **calc_dual_line 通道宽度**: Python 参考 `±VOLALITY`，Rust 2x 宽度导致偏差

#### CTP 集成
- Rust CTP 生态已有 10+ 项目，ctp2rs（openctp 官方推荐）可直接替代 Python 桥接
- CTP 实际 44 字段（非 42），AveragePrice + ActionDay 为 6.3.15+ 新增
- 认证两步流程：ReqAuthenticate → ReqUserLogin → ReqSettlementInfoConfirm
- GIL 约束：回调串行，需入队异步化
- 多账户需独立 API 实例 + 独立 FlowPath

#### Pipeline / DAG
- `Pipeline::from_config` 实现模式：遍历 config 字段 → 字符串→枚举映射 → 条件构造
- NodeFactory 注册后需手动调用 `on_init`（Pipeline 不会自动调用）
- DAG 环路检查使用 `if let Ok(...)` 导致被静默吞没，需改为 Result 传播

#### Agent 提示词模板模式
- **结构**：角色定义→输入数据→分析框架（按步骤）→输出格式（JSON+示例）→铁则
- **decision_agent 四 Gate 门控**：resonance→risk.constraints→entry/sl/tp/size→final confidence
- **resonance_agent 严格四维全向**：4/4 同向才成立共振（旧版是打分制 ≥2）
- **铁则**：数据不编造 / 恒等校验 / 合法 JSON / 不预估未来

#### Tauri / CLI
- `custom-protocol` feature 不在 Tauri v2 默认 features 中，必须显式添加
- 每个 Tauri 命令需注册 `remote_workspace_policy`
- `desktop:dev` 走 `devUrl`（热更新），`desktop:build:fast` 是正确构建命令
- PowerShell + JSON + 中文文本 → Hook 触发（编码问题），改用 Node.js 脚本

#### 其他
- **Welford 在线统计算法**: O(1) 空间，CDF 用 Abramowitz & Stegun §7.1.26 有理逼近
- **CSV 解析**: golden_tick 的 quotes 列含逗号+JSON，需状态机解析器
- **ryu 格式化**: `4520.0` 序列化为 `"4520"`（无 `.0`）
- **循环依赖规避**: `taiji-dvmi → taiji-engine` 单向，测试放 dvmi 侧
- **Unleash 最小部署**: 512MB VM + PostgreSQL 16-alpine
- **交易合规 6 红线**: 无牌照不荐股 / 风险揭示强制 / 免责声明 / 数据来源标注 / 模拟交易声明 / 不承诺收益

---

## 四、失败模式与教训

### 4.1 高频失败模式（出现 3+ 次）

| 失败模式 | 次数 | 典型案例 | 改进方案 |
|---|---|---|---|
| **cargo check ≠ cargo test** | 5+ | 25, 26, 28, 37: check 通过但 test 编译失败 | 固定验证流程：先 `cargo check` → 再 `cargo test` |
| **预存编译错误阻塞** | 4+ | 33, 34: 无关 crate 的预存错误干扰验证 | 按受影响 crate 验证，不全量 `--workspace` |
| **spec 与实际 API 不一致** | 3+ | 43, 38: spec 中 API 名称/参数与实际代码不符 | 实现前先读目标文件确认 API 签名 |
| **测试断言值需要对齐** | 3+ | 33, 36: ryu 格式化、CSV 列索引与实际格式不符 | 测试值以实际运行结果为准，不盲从模板 |
| **上下文压缩导致状态丢失** | 3+ | 42, 43, 45: compact 后 agent 遗忘前置约束 | 每次恢复后重新加载核心规则 |

### 4.2 关键教训

1. **BarGenerator 从未初始化（C1）**: 注释写了懒初始化意图但代码从未实现 → 引擎完全不可运行。教训：注释的"TODO"不会被自动执行，必须显式初始化。

2. **SchemaAdapter 不完整（C2）**: `..Default::default()` 将 47 字段结构体除 5 个外全部归零，Delta 恒为零。教训：`Default` derive 在复杂结构体上会掩盖字段缺失。

3. **DAG 环路静默吞没（C3）**: `if let Ok(...)` 吞掉错误，无日志无传播。教训：所有 `Result` 必须显式处理或传播，`let _ =` 和 `if let Ok` 是潜在 bug 源。

4. **CronService 假设破裂（Phase 4→5）**: 基于文档假设存在的能力，实际代码中不存在。教训：所有外部依赖假设必须在实现前通过代码侦察验证。

5. **annotation keys 断裂（跨模块）**: 用无前缀 key 查 Pipeline JSON → 全部标注失效。教训：跨模块的 key 约定需在 type-contract 中显式定义。

---

## 五、用户偏好信号汇总

### 5.1 新增偏好（本次分析首次发现）

| 偏好 | 来源 Session | 强度 |
|---|---|---|
| "不猜测，先查清再说" | 3b834f3c | 铁则级 |
| "不准杀自己进程" | 3b834f3c (2 次) | 铁则级 |
| 三版架构：dev → stable → reference | 3b834f3c | 架构级 |
| ACP Agent 能力分层：CodeBuddy=编码，Qoder=架构 | 3b834f3c | 工具级 |
| 零警告策略（包括 harmless warning） | 3b834f3c | 铁则级 |
| "不做和稀泥"——SSG 选型必须裁决 | fe996913 | 行为级 |
| 交易合规 6 条红线 | f09ed1e2 | 铁则级 |
| 精确替换而非迭代修补 | c4c501a0 | 行为级 |
| 全局修复测试断言（不只在当前文件） | c4c501a0 | 行为级 |
| 注释保留而非删除闭源引用 | e81509ad | 行为级 |

### 5.2 累计铁则清单（本次跨 session 确认）

1. **方案未定不执行** — 需求对齐→目标确认→技术方案，前一步未闭合不推进
2. **不猜测** — 必须查清事实再行动，任何猜测都会被惩罚
3. **从零重建** — 每个文件新建过审，不"改造"旧代码
4. **复用不拼装** — 参考算法不引入框架，trait 边界 = 知识产权隔离墙
5. **零容忍** — 零警告、零猜测、零和稀泥、零编造
6. **先规矩后干活** — master-framework 最先加载，10 棵决策树遍历后方可工作
7. **99% 准备 + 1% 执行** — 侦察→文档→契约→派发，准备不充分不执行
8. **自主执行不交互** — 有明确 spec 时直接执行，不问不确认
9. **全局修复** — 一个 bug 修复影响所有测试时，一次性全部修复
10. **build + test 双验证** — `cargo check` 零错误 + `cargo test` 零失败

---

## 六、跨 Session 模式演进

### 6.1 Agent 成熟度变化

| 维度 | 早期批次 C (25-38) | 指挥官 (42-51) |
|---|---|---|
| 任务粒度 | 单文件/单函数级 | 多 crate/全链路级 |
| 侦察先行 | 偶有侦察 | 每个方向必有侦察报告 |
| 文档先行 | 无 | type-contract + dispatch-prompts |
| 审查闭环 | 一次性 | 审查→修复→再审 |
| R-ID 闭合 | 无系统追踪 | 逐项闭合矩阵 |
| 错误处理 | 简单修复 | 根因分析 + 系统性改进 |

### 6.2 架构演进时间线

```
7/19 46cc66b6 → 需求对齐 + 四硬约束定稿 + 审查蜂降级
7/20 3b834f3c → 三版架构 + Phase 1 闭合 + 代码图谱建立
7/21 f6276d09 → DVMI 审计 + Phase 3 交叉审查 + Phase 4 规划
7/21 cc881d14 → taiji-engine 审计 + Tauri 命令 + taiji-cli
7/21 e81509ad → dvmi 追加 + Phase 4/5/6 Recon + 架构文档
7/21 42fbbe24 → Bar 生成 + SchemaAdapter + CronService 验证
7/21 c4c501a0 → dvmi 计算节点 + 序列化 + ACP 探测
7/21 3f58fb7c → Phase 4-7 多阶段规划 + 派发提示词
7/21 4e65de2c → Pipeline 集成测试 + taiji_status/export + RiskMonitor
7/21 34863803 → Phase 6 骨架 + ACP permission_mode 加固
```

---

## 七、知识缺口与待跟进

1. **Session 32 和 35** 在批次 C 中缺失（跳号原因不明）
2. **子代理 a3 对 session 25-27 的分析不够详细**（仅摘要，缺失逐 session 的任务/关键词/偏好信号）
3. **DVMI backtest_v4 脚本的完整 Python→Rust 迁移** 尚未执行（难度最高的是 `calc_envelope_trendline()`）
4. **StateStore COW 性能瓶颈**（全量复制 O(n)）已识别但 DashMap 改造未执行
5. **Phase 6 的 GPU 加速、策略热更新** 仍处于侦察阶段，未进入实现
6. **交易合规 6 条红线** 在 Phase 6 才被识别，需前向回溯至已有产出（Signal 输出、视频前 3 秒、CLI 启动页）

---

## 八、附录：Session 快速索引

### 指挥官 Session

| Short ID | # | 日期 | 核心任务 | 战略意义 |
|---|---|---|---|---|
| 46cc66b6 | - | 7/19 | 需求对齐、仓库策略、架构定稿、审查蜂Hook降级 | 总指挥启动 |
| 3b834f3c | 44 | 7/20 | 安全审查、三版架构、代码图谱、Phase 1闭合 | 架构定稿 |
| f6276d09 | 42 | 7/21 | DVMI审计、Phase 3交叉审查、Phase 4规划 | 质量门禁 |
| cc881d14 | 43 | 7/21 | taiji-engine审计、Tauri命令、taiji-cli | 基座加固 |
| e81509ad | 45 | 7/21 | dvmi追加、Phase 4/5/6侦察、架构文档 | 侦察规划 |
| 42fbbe24 | 48 | 7/21 | Bar生成、SchemaAdapter、Cron验证 | 管道精化 |
| c4c501a0 | 47 | 7/21 | dvmi计算节点、ACP探测 | 计算引擎 |
| 3f58fb7c | 49 | 7/21 | Phase 4-7规划、派发提示词 | 多阶段规划 |
| 4e65de2c | 50 | 7/21 | Pipeline测试、Tauri状态/导出、RiskMonitor | 集成验证 |
| 34863803 | 51 | 7/21 | Phase 6骨架、ACP权限加固 | 安全审计 |

### 批次 C 早期 Session

| Short ID | # | 核心任务 |
|---|---|---|
| e040310e | 25 | StateManager快照恢复、dvmi趋势线修复、WonderTrader风控/回测侦察 |
| 4adf3584 | 26 | 视频管线侦察、WonderTrader WtUftEngine、RAG管道建设 |
| cab13a0d | 27 | CronJob调度、社交发帖调研、E2E全链路测试、i18n英文化 |
| b3487df8 | 28 | MagnetNode、WonderTrader数据管理、R-ID闭合、数据看板、AgentWeights |
| 78517a49 | 29 | RiskNode+DefaultRiskMonitor、WtCtaEngine侦察、risk/decision模板、PublishScheduler |
| 8b293226 | 30 | BarGenerator管道集成、CTP侦察、知识图谱可视化、EmailDispatcher |
| 778d68f1 | 31 | 军事重组算法、WtExecMon侦察、social-auto-upload、Feature flags、Phase 6终审 |
| 41a622a1 | 33 | PyO3、pa-agent侦察、delta_agent模板、配置面板、隐私合规、实时推流引擎 |
| 7e9e3796 | 34 | PipelineStatus可观测性、magnet/resonance模板、Phase 4派发提示词、StateValue通用化、订单流分析 |
| fe996913 | 36 | 双端对照测试、CTP FFI调查、SSG选型、Phase 5a交叉审查 |
| 7d3da11f | 37 | 性能基准测试、openctp行情订阅、ComposeConfig/EncodingProfile、CsvReplaySource |
| f09ed1e2 | 38 | property-based测试、CTP认证管理、音画同步测试、Phase 5+6规划、全链路测试、Phase 6 C+X侦察 |

---

*报告由 BitFun agentic session 分析管线自动生成。数据来源：6 份 rollout summary + 6 份子代理分析报告。*
