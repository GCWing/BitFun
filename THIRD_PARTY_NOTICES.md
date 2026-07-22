# 第三方代码与知识产权声明

> 太极（Taiji）量化交易系统中引用的所有外部项目、许可证及使用方式。
> 本文件作为 MIT 许可证的补充，确保所有第三方知识产权得到妥善标注。

---

## 一、代码级参考（直接改编或移植）

以下项目的代码被直接阅读、理解后以 Rust 重新实现。未复制原项目代码，但算法逻辑参考了原项目。

| 项目 | 许可证 | 参考内容 | 涉及文件 |
|------|--------|---------|---------|
| **czsc** (Apache 2.0) | [Apache 2.0](https://github.com/zengbin93/czsc/blob/main/LICENSE) | BarGenerator 聚合逻辑、Freq 枚举设计 | `taiji-bar/src/lib.rs`, `taiji-engine/src/types/bar.rs`, `taiji-engine/src/pipeline/bar_gen.rs` |
| **WonderTrader** (MIT) | [MIT](https://github.com/wondertrader/wondertrader/blob/master/LICENSE) | CtaStrategy 上下文隔离设计、ICtaStraCtx 接口模式 | `taiji-engine/src/node.rs` (ComputeNode trait 设计) |
| **chanlun.rs** (MIT) | [MIT](https://github.com/luishsr/chanlun.rs) | 流式增量计算模式 | `taiji-engine/src/pipeline/mod.rs` (BarGenerator 增量更新) |
| **pa-agent** (MIT) | [MIT](https://github.com/naskio/pa-agent) | 二元决策树引擎、增量指标状态机、两阶段门控流水线 | `taiji-engine/src/debate/`, `taiji-strategen/src/` |
| **vibe-trading** (MIT) | [MIT](https://github.com/vibe-trading/vibe-trading) | ReAct 循环上下文管理、Swarm YAML 预设、AlphaMeta AST 解析、三层安全模式 | `taiji-strategen/src/`, Agent swarm 编排设计 |

## 二、算法参考（方法论启发，非代码改编）

以下项目的方法论被参考，但代码为独立实现，无直接改编关系。

| 项目 | 许可证 | 参考内容 | 涉及文件 |
|------|--------|---------|---------|
| **CNN Fear & Greed Index** | 方法论（无代码） | 五因子情绪温度计计算框架 | `taiji-sentiment/src/fgi.rs` |
| **SnowNLP** (MIT) | [MIT](https://github.com/isnowfy/snownlp) | 中文情感分析流程设计 | `taiji-sentiment/src/tokenizer.rs` |
| **cnsenti** (MIT) | [MIT](https://github.com/duanyifei1937/cnsenti) | 金融情感词典结构 | `taiji-sentiment/src/tokenizer.rs` |
| **stolgo** (MIT) | [MIT](https://github.com/stolgo/stolgo) | 无未来函数保障模式（BarDataView._limit + LookaheadError） | `taiji-engine/src/source/` |

## 三、工具库参考（设计模式启发）

| 项目 | 许可证 | 参考内容 | 涉及文件 |
|------|--------|---------|---------|
| **ffmpeg-sidecar** (MIT) | [MIT](https://github.com/nicholaschiasson/ffmpeg-sidecar) | FfmpegCommand Builder 模式 | `taiji-content/src/composer.rs` |
| **biliup** (MIT) | [MIT](https://github.com/biliup/biliup) | 多 CDN 线路探测 + 分块并发上传 | `taiji-publisher/src/biliup.rs` |
| **youtube-uploader-mcp** (MIT) | [MIT](https://github.com/youtube-uploader-mcp) | MCP Tool 接口模式（Tool{Name,Define,Handle}） | Agent tool 设计 |

## 四、直接依赖（Cargo.toml 声明的 Rust crate）

所有 Rust 依赖通过 crates.io 引入，许可证均为 MIT / Apache 2.0 / BSD 兼容。完整列表见各 crate 的 `Cargo.toml`。

## 五、免责声明

1. 本系统对上述第三方项目的任何引用均为"算法逻辑参考"或"设计模式启发"，**未复制原项目源代码**。
2. 所有代码为独立 Rust 实现，与原项目的 Python/Go 代码无直接对应关系。
3. 如任何第三方权利人认为本系统的参考方式超出"合理使用"范围，请联系我们，我们将立即调整。
4. 闭源 crate（taiji-dvmi / taiji-magnet / taiji-thrust / taiji-risk）的完整实现不在本仓库中。

---

*最后更新：2026-07-22*
