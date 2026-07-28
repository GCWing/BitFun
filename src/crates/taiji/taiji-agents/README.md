# Taiji Agents — Phase 3 多智能体提示词模板

> 版本：Phase 3 (2026-07-21)
> 对应 Type Contract：`.bitfun/team/type-contract-phase3.md` §5

## 概述

Taiji Agent 系统由 7 个独立 AI Agent 组成，按"量价时空"四维分析框架编排为有向无环图（DAG）。每个 Agent 以 Markdown 提示词模板定义角色、分析逻辑和输出格式，由 BitFun Commander 在每根 K 线闭合后调度执行。

## Agent 总览

| # | Agent | 角色 | 输入源 | 输出 | DAG 层级 |
|---|-------|------|--------|------|:---:|
| 1 | [structure-agent] | 市场结构分析：趋势方向、趋势强度、拐点结构、支撑/阻力 | `{{PIPELINE_EXPORT_PATH}}` | `structure_analysis.json` | L0 |
| 2 | [delta-agent] | 资金流向分析：六核心指标、建仓/平仓/中性判定 | `{{PIPELINE_EXPORT_PATH}}` | `delta_analysis.json` | L0 |
| 3 | [magnet-agent] | 磁体定位分析：磁体位置、虚实判定、MM1/MM2 目标测量、多级共振 | `{{PIPELINE_EXPORT_PATH}}` | `magnet_analysis.json` | L0 |
| 4 | [thrust-agent] | 三推形态分析：力竭判定、BOS/CHoCH 结构改变检测 | `{{PIPELINE_EXPORT_PATH}}` | `thrust_analysis.json` | L0 |
| 5 | [risk-agent] | 风控约束：ATR 波动率、凯利仓位、方向约束、止损距离 | `{{PIPELINE_EXPORT_PATH}}` | `risk_analysis.json` | L0 |
| 6 | [resonance-agent] | 多维度共振分析：四维方向一致性、冲突检测、多周期共振 | L0 四个 Agent 的输出 + `{{PIPELINE_EXPORT_PATH}}` | `resonance_analysis.json` | L1 |
| 7 | [decision-agent] | 交易决策：多门控参数计算、最终入场/止损/止盈/仓位 | resonance + risk + L0 四个 Agent 的 confidence | `decision.json` | L2 |

**DAG 层级说明**：
- **L0**（5 个 Agent）：并行执行，仅依赖 Pipeline 原始数据，互不依赖
- **L1**（resonance）：依赖 L0 中 structure/delta/magnet/thrust 四个 Agent 的输出
- **L2**（decision）：依赖 resonance（L1）+ risk（L0）+ 所有 L0 Agent 的 confidence

## 数据流

```
                        ┌─────────────────────┐
                        │  Pipeline Export     │
                        │  (pipeline.json)     │
                        └──────┬──────────────┘
                               │
          ┌────────────────────┼────────────────────────┐
          │                    │                        │
     ┌────▼────┐    ┌────▼────┐    ┌────▼────┐    ┌────▼────┐    ┌────▼────┐
     │structure │    │ delta   │    │ magnet  │    │ thrust  │    │  risk   │
     │ analysis │    │analysis │    │analysis │    │analysis │    │analysis │
     └────┬─────┘    └────┬────┘    └────┬────┘    └────┬────┘    └───┬─────┘
          │               │              │              │             │
          └───────────────┴──────┬───────┴──────────────┘             │
                                │                                     │
                          ┌─────▼─────┐                               │
                          │ resonance │                               │
                          │ analysis  │                               │
                          └─────┬─────┘                               │
                                │                                     │
                          ┌─────┴─────────────────────────────────────┘
                          │
                    ┌─────▼─────┐
                    │ decision  │
                    │  .json    │
                    └───────────┘
```

**并行执行**：L0 的 5 个 Agent 互不依赖，可由 BitFun Commander 并行调度。L1（resonance）等待 structure/delta/magnet/thrust 全部完成。L2（decision）等待 resonance + risk 完成。

**数据传递**：Agent 之间通过 JSON 文件传递数据，路径由 `{{UPSTREAM_OUTPUTS}}` 模板变量注入。每个 Agent 严格输出 `analysis` 子对象 + `confidence` 字段，下游按字段路径读取。

## 提示词模板结构

每个 Agent 的 `.md` 文件遵循统一模板结构：

```markdown
# Agent Name — 简短描述

## 角色
角色定义 + 领域知识背景

## 输入数据
模板变量（{{PIPELINE_EXPORT_PATH}} / {{UPSTREAM_OUTPUTS}}）+ 关注字段表

## 分析框架
分步骤的分析逻辑（伪代码风格，LLM 可直接执行）

## 置信度规则
confidence 分档表 + 条件说明

## 输出格式
严格 JSON schema + 示例（含正常/异常/边界情况）

## 铁则
硬性约束（不可违反的规则）
```

## 使用方法

### 在 BitFun Commander 中触发

```yaml
# 单 Agent 执行
commander:
  agent: structure_agent
  template: src/crates/taiji/taiji-agents/structure-agent.md
  inputs:
    PIPELINE_EXPORT_PATH: /data/pipeline/latest.json

# 全 DAG 执行（Phase 3 默认模式）
commander:
  dag: taiji-phase3
  inputs:
    PIPELINE_EXPORT_PATH: /data/pipeline/latest.json
  upstream_outputs:
    structure: /data/agents/structure_analysis.json
    delta: /data/agents/delta_analysis.json
    magnet: /data/agents/magnet_analysis.json
    thrust: /data/agents/thrust_analysis.json
  output: /data/agents/decision.json
```

### 手动测试

```bash
# 单 Agent 测试（将模板变量替换为实际路径后发送给 LLM）
cat structure-agent.md | sed 's|{{PIPELINE_EXPORT_PATH}}|/tmp/test_pipeline.json|g' | llm

# Schema 校验
python scripts/validate-agent-outputs.py
```

### 输出 Schema

每个 Agent 的输出格式由 `scripts/agent-output-schemas/<agent>.schema.json` 定义。运行 `scripts/validate-agent-outputs.py` 可校验所有示例输出是否符合 schema。

## 文件布局

```
src/crates/taiji/taiji-agents/
├── README.md                  ← 本文件
├── structure-agent.md         ← 市场结构分析（趋势方向/强度/支撑阻力）
├── delta-agent.md             ← 资金流向分析（六核心/建仓平仓）
├── magnet-agent.md            ← 磁体定位分析（虚实/MM 目标/共振）
├── thrust-agent.md            ← 三推形态分析（力竭/BOS/CHoCH）
├── risk-agent.md              ← 风控约束（ATR/凯利/方向开关）
├── resonance-agent.md         ← 共振分析（四维一致性/冲突检测）
└── decision-agent.md          ← 交易决策（入场/止损/止盈/仓位）
```

关联文件：
```
scripts/agent-output-schemas/   ← 7 个 Agent 的 JSON Schema 定义
scripts/validate-agent-outputs.py  ← Schema 校验脚本
docs/review/cross-review-report.md ← 交叉审查报告（18 问题，6 已修复）
```

## 理论对齐

7 个 Agent 的分工对应"量价时空"四维分析框架：

| 维度 | Agent | 分析对象 | 核心指标 |
|------|-------|----------|----------|
| 价（结构） | structure-agent | 趋势方向、拐点、趋势线 | `trend_direction`, `trend_strength` |
| 量（资金） | delta-agent | 六核心、持仓变化 | `net_position` |
| 空（位置） | magnet-agent | 磁体区、MM 目标 | `magnet_position`, `mm1_target`, `mm2_target` |
| 时（节奏） | thrust-agent | 三推、BOS/CHoCH | `triple_push_found`, `exhaustion` |
| 闭环验证 | resonance-agent | 四维同向共振 | `resonance`, `resonance_type` |
| 风险边界 | risk-agent | ATR、凯利 | `allow_long`, `allow_short`, `max_size` |
| 执行仲裁 | decision-agent | 综合决策 | `action`, `entry`, `stop_loss`, `take_profit` |

## 设计原则

1. **独立维度**：L0 的 5 个 Agent 分析独立、互不调用——确保信号来自不同视角，避免循环论证。
2. **门控决策**：resonance 和 decision 采用"通过才继续"的二元决策树——任一 Gate 失败即终止，不留模棱两可。
3. **不编造**：所有 Agent 的铁则第一条就是"数据不足时降低置信度，不编造"。OI 缺失、swing 缺失、bars 不足等情况均有明确的降档规则。
4. **可审计**：每个 Agent 输出 `confidence` + `gate_trace`/`decision_trace`（resonance/decision），下游可追溯上游推理链。
