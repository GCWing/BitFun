# 致谢

太极（Taiji）量化交易系统的诞生，离不开以下优秀开源项目的启发与支持。我们在此向所有项目维护者和贡献者致以诚挚的感谢。

---

## 代码级参考（算法移植与适配）

以下项目的代码被阅读、理解后以 Rust 重新实现。我们保留了原项目的设计精髓，同时适配了太极的 DAG 管线架构。

### czsc — 缠中说禅量化框架

- 项目地址：[https://github.com/zengbin93/czsc](https://github.com/zengbin93/czsc)
- 许可证：Apache License 2.0
- 作者：[zengbin93](https://github.com/zengbin93)
- 参考内容：BarGenerator 聚合逻辑、Freq 枚举设计、RawBar 数据结构
- 涉及文件：`taiji-engine/src/pipeline/bar_gen.rs`、`taiji-engine/src/types/bar.rs`、`taiji-bar/src/lib.rs`
- 修改说明：从 Python 移植为 Rust；将 BarGenerator 重构为 ComputeNode 管线架构；扩展 RawBar 增加 delta 分类和成交量分布字段

### WonderTrader — 开源量化交易框架

- 项目地址：[https://github.com/wondertrader/wondertrader](https://github.com/wondertrader/wondertrader)
- 许可证：MIT
- 参考内容：CtaStrategy 上下文隔离设计、ICtaStraCtx 接口模式
- 涉及文件：`taiji-engine/src/node.rs`（ComputeNode trait 设计）

### openctp — CTP 开放平台

- 项目地址：[https://github.com/openctp/openctp](https://github.com/openctp/openctp)
- 许可证：BSD
- 参考内容：TickData 字段布局（匹配 CTP FTD-XML 协议，47 字段）
- 涉及文件：`taiji-engine/src/types/tick.rs`

---

## 算法参考（方法论启发，独立实现）

以下项目的方法论被参考，但代码为独立 Rust 实现，无直接移植关系。

### pa-agent — 交易 Agent 框架

- 项目地址：[https://github.com/naskio/pa-agent](https://github.com/naskio/pa-agent)
- 许可证：MIT
- 参考内容：二元决策树引擎、增量指标状态机（EmaState/AtrState）、两阶段门控流水线
- 涉及文件：`taiji-engine/src/debate/`、`taiji-strategen/src/`

### vibe-trading — Vibe Coding 交易框架

- 项目地址：[https://github.com/vibe-trading/vibe-trading](https://github.com/vibe-trading/vibe-trading)
- 许可证：MIT
- 参考内容：ReAct 循环上下文管理、Swarm YAML 预设、AlphaMeta AST 解析、Agent 安全三层模型
- 涉及文件：`taiji-strategen/src/`、Agent swarm 编排设计

### chanlun.rs — Rust 缠论实现

- 项目地址：[https://github.com/luishsr/chanlun.rs](https://github.com/luishsr/chanlun.rs)
- 许可证：MIT
- 参考内容：流式增量计算模式、BarGenerator 逐 bar 推进设计
- 涉及文件：`taiji-engine/src/pipeline/mod.rs`

### stolgo — 无未来函数保障

- 项目地址：[https://github.com/stolgo/stolgo](https://github.com/stolgo/stolgo)
- 许可证：MIT
- 参考内容：BarDataView 截断 + LookaheadError 无未来函数保障模式
- 涉及文件：`taiji-engine/src/source/`

### SnowNLP — 中文情感分析

- 项目地址：[https://github.com/isnowfy/snownlp](https://github.com/isnowfy/snownlp)
- 许可证：MIT
- 参考内容：中文情感分析流程设计
- 涉及文件：`taiji-sentiment/src/tokenizer.rs`

### cnsenti — 中文情感词典

- 项目地址：[https://github.com/duanyifei1937/cnsenti](https://github.com/duanyifei1937/cnsenti)
- 许可证：MIT
- 参考内容：金融情感词典结构
- 涉及文件：`taiji-sentiment/src/tokenizer.rs`

---

## 工具与设计模式参考

### ffmpeg-sidecar — FFmpeg Rust 封装

- 项目地址：[https://github.com/nicholaschiasson/ffmpeg-sidecar](https://github.com/nicholaschiasson/ffmpeg-sidecar)
- 许可证：MIT
- 参考内容：FfmpegCommand Builder 模式
- 涉及文件：`taiji-content/src/composer.rs`

### biliup — B 站命令行投稿工具

- 项目地址：[https://github.com/biliup/biliup](https://github.com/biliup/biliup)
- 许可证：MIT
- 参考内容：多 CDN 线路探测 + 分块并发上传设计
- 涉及文件：`taiji-publisher/src/`

### youtube-uploader-mcp — YouTube MCP 上传工具

- 项目地址：[https://github.com/youtube-uploader-mcp](https://github.com/youtube-uploader-mcp)
- 许可证：MIT
- 参考内容：MCP Tool 接口模式（Tool{Name,Define,Handle}）
- 涉及文件：Agent tool 设计

---

## 方法论参考（无代码）

### CNN Fear & Greed Index

- 参考内容：五因子情绪温度计计算框架（市场波动率、动量、成交量、避险需求、期权偏斜）
- 涉及文件：`taiji-sentiment/src/fgi.rs`

---

## 声明

1. 以上所有第三方项目的引用均为"算法逻辑参考"或"设计模式启发"，**未复制原项目源代码**。
2. 所有太极代码为独立 Rust 实现，与原项目的 Python/Go/C++ 代码无直接对应关系。
3. 如任何第三方权利人认为引用方式超出合理使用范围，请联系我们。
4. 本致谢文件同时满足 Apache 2.0 §4 和 BSD 许可证的再分发声明要求。

---

*最后更新：2026-07-22*
*致谢人：量价仓交易狮（B站）*
