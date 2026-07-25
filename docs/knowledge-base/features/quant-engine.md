# 量化引擎系统（taiji-quant）

> **涉及 crate**: taiji-engine, taiji-bar, taiji-backtest, taiji-executor, taiji-realtime, 等 19 crates
> **核心文件**: src/crates/taiji/
> **更新日期**: 2026-07-25

---

## 架构总览

```
实时行情
  │
  ▼
taiji-realtime ──┬── WebSocket 桥接
                 └── 行情通道
  │
  ▼
taiji-bar ── Tick → K线聚合
  │
  ▼
taiji-engine ── DAG 管线
  ├── 数据源层 (Data Source)
  │   ├── adapter    ← 统一数据适配
  │   ├── datasource ← 数据源管理
  │   ├── mgr        ← 多数据源调度
  │   ├── replay     ← 历史回放
  │   └── validator  ← 数据校验
  │
  ├── 计算层 (Compute)
  │   ├── pipeline/bar_gen  ← K线生成
  │   ├── pipeline/reorg    ← K线重组
  │   ├── pipeline/status   ← 管线状态
  │   ├── node              ← 计算节点
  │   ├── signal            ← 信号生成
  │   ├── fusion            ← 信号融合
  │   ├── risk              ← 风控
  │   └── compliance        ← 合规检查
  │
  ├── 状态层 (State)
  │   ├── state/mod.rs      ← 全局状态
  │   ├── state/snapshot.rs ← 快照
  │   └── store             ← 状态存储
  │
  ├── DAG 编排
  │   ├── dag.rs            ← 有向无环图
  │   ├── factory.rs        ← 节点工厂
  │   ├── config.rs         ← 配置解析
  │   └── error.rs          ← 错误类型
  │
  └── 辩论系统 (Debate)
      ├── orchestrator  ← 辩论编排
      ├── agents        ← 辩论 Agent
      ├── decision      ← 决策机制
      └── record        ← 辩论记录
  │
  ▼
taiji-executor ── 订单执行
  ├── order_mgr  ← 订单管理
  ├── bridge     ← 交易所桥接
  ├── position   ← 持仓管理
  └── types      ← 订单类型

taiji-abnormal ── 异常检测
  ├── vol_anomaly    ← 成交量异常
  ├── vol_regime     ← 成交量状态切换
  ├── gap_alert      ← 跳空预警
  ├── trend_accel    ← 趋势加速
  ├── corr_fracture  ← 相关性断裂
  └── scorecard      ← 综合评分卡

taiji-pattern ── 模式匹配
  ├── dtw     ← 动态时间规整
  ├── node    ← 模式节点
  └── index   ← 模式索引

taiji-orderflow ── 订单流分析
  ├── ofi     ← 订单流不平衡
  ├── vpin    ← 成交量推进
  └── welford ← 在线统计

taiji-sentiment ── 情绪分析
  ├── fgi        ← 恐惧贪婪指数
  ├── tokenizer  ← 中文分词
  └── node       ← 情绪节点

taiji-strategen ── 策略生成
  ├── analyzer    ← 策略分析器
  ├── compiler    ← 策略编译器
  ├── hypothesis  ← 假设生成
  ├── pipeline    ← 生成管线
  └── refiner     ← 策略优化

taiji-backtest ── 回测引擎
  ├── runner      ← 回测运行
  ├── walk_forward← Walk-forward 验证
  ├── stats       ← 回测统计
  └── trade_record← 交易记录

taiji-publisher ── 多平台发布
  ├── biliup           ← B站直播
  ├── publisher_twitter ← Twitter
  ├── publisher_wechat_mp ← 微信公众号
  ├── social_auto      ← 社交自动发布
  └── publish_scheduler ← 发布调度

taiji-growth ── 运营增长
  ├── task_dag_exec    ← 增长任务 DAG
  ├── report_md_gen    ← 报告生成
  ├── email_dispatcher ← 邮件分发
  └── publisher_website← 网站发布

taiji-alert ── 告警系统
  ├── alerters   ← 多渠道告警
  └── heartbeat  ← 心跳检测

taiji-content ── 内容工坊
  ├── composer       ← 视频合成
  ├── kline_renderer ← K线渲染
  ├── live_stream    ← 直播推流
  ├── annotation     ← 图表标注
  └── cron_job       ← 定时任务

taiji-llm ── LLM 客户端
  ├── provider/bitfun ← BitFun 驱动
  ├── provider/local  ← 本地模型
  ├── embedding       ← 向量嵌入
  └── client          ← LLM 调用

taiji-knowledge-graph ── 知识图谱
  ├── embedding    ← 实体嵌入
  └── types        ← 图类型

taiji-engine-py ── Python 绑定
  ├── rl_env        ← 强化学习环境
  ├── reward_calculator ← 奖励计算
  └── python/engine_py  ← PyO3 绑定
```

## 数据流示例

```
Tick 数据 → taiji-realtime → taiji-bar(K线聚合) → taiji-engine
  → [signal 信号生成]
  → [fusion 信号融合]
  → [risk 风控]
  → [compliance 合规]
  → taiji-executor(订单执行)
```

## 辩论系统

```
市场状态 → 多方 Agent
         → 空方 Agent
         → 仲裁 Agent → 决策(做多/做空/观望)
         → 记录到辩论历史
```

## RL 强化学习环境

```
taiji-engine-py (Python → PyO3)
├─ RL Environment (gymnasium 兼容)
├─ 状态: K线 + 技术指标 + 持仓
├─ 动作: 开仓/平仓/持仓
└─ 奖励: PnL + 风险惩罚
```
