# 外部参考代码图谱

> 16 个外部开源项目完整分析，按技术总纲 Phase 2-6 映射。
> 每个项目标注：核心算法、文件位置、行数、许可证、太极可复用模式。
> 生成日期：2026-07-20

---

## Phase 2: 交易引擎

### 数据管道（阻塞项，优先）

| 参考项目 | 语言 | 许可证 | 核心文件 | 行数 | 关键算法 |
|---------|:--:|------|---------|:--:|---------|
| **czsc** | Rust | Apache 2.0 | `crates/czsc-utils/src/bar_generator.rs` | 1181 | `BarGenerator::update_freq()` — 纯时间聚合（时间桶边界 + OHLCV 合并），`remove_include()` 包含关系处理，9 个 crate 分层架构 |
| **czsc** | Rust | Apache 2.0 | `crates/czsc-core/src/objects/` | 693+422+401+606 | RawBar/Freq/Market/Direction/FX/BI/ZS 完整数据结构 |
| **czsc** | Rust | Apache 2.0 | `crates/czsc-python/src/lib.rs` | 97 | PyO3 分层注册模式：每 crate 提供 `register(py, m)` → facade 聚合 |
| **openctp** | C++/Python | BSD | `ctpapi-python/` | — | SWIG 绑定，42 字段 CThostFtdcDepthMarketDataField，SubscribeMarketData→OnRtnDepthMarketData 订阅模式 |

**taiji-bar 实现路线**：
1. 从 czsc 参考 BarGenerator 时间聚合核心（1181 行）→ 适配 taiji RawBar 结构
2. 从 openctp 参考 tick 字段定义（42 字段）→ 定义 taiji TickData
3. 新增：czsc 没有的量/幅聚合模式（量价时空需要）
4. Rust↔Python 桥接 Tauri command（调用 openctp-ctp Python SDK）

### 缠论完整实现参考

| 参考项目 | 语言 | 许可证 | 核心文件 | 行数 | 关键特征 |
|---------|:--:|------|---------|:--:|---------|
| **chanlun.rs** | Rust | MIT | `algorithm/bi.rs` | 1006 | 纯 Rust，中文标识符，`分型→笔→线段→中枢` 完整链 |
| **chanlun.rs** | Rust | MIT | `business/observer.rs` | 1442 | `观察者` 流式增量计算，`投喂K线()` 逐 bar 推进 |
| **chanlun.rs** | Rust | MIT | `chanlun-py/src/lib.rs` | — | 独立 cdylib crate（`chanlun` + `chanlun-py` 分离），比 czsc 的单体 facade 更干净 |

---

## Phase 3: 交易应用

| 参考项目 | 语言 | 许可证 | 核心文件 | 关键模式 |
|---------|:--:|------|---------|---------|
| **WonderTrader** | C++ | MIT | `Includes/ICtaStraCtx.h` | Context 隔离模式：策略只通过纯虚接口与系统交互，零全局状态 |
| **WonderTrader** | C++ | MIT | `Includes/WTSDataDef.hpp` | K线切片零拷贝：`WTSKlineSlice` 分块引用设计 |
| **WonderTrader** | C++ | MIT | `Includes/RiskMonDefs.h` | 三层风控：组合盘资金/通道流量/账户资金 |
| **wtpy** | Python | MIT | `CtaContext.py` | ctypes 桥接 C++ 引擎，`WtNpKline` NumPy 结构化数组 |
| **wtpy** | Python | MIT | `demos/Strategies/DualThrust.py` | 策略生命周期：`on_init()`→`on_calculate()`→信号生成→`stra_enter_long()` |

---

## Phase 4: 内容工坊

| 参考项目 | 语言 | 许可证 | Stars/下载 | 关键模式 |
|---------|:--:|------|:--:|---------|
| **ffmpeg-sidecar** | Rust | MIT | 144 万下载 | `FfmpegCommand` Builder + `FfmpegIterator`（sync_channel 反压迭代器），自动下载 FFmpeg 二进制 |
| **biliup** | Rust | MIT | 5.3K⭐ | 4 crate 工作区，多 CDN 线路探测 + 分块并发上传（`buffer_unordered`），B站 AppKey+MD5 签名认证 |
| **biliup** | Rust | MIT | — | Tauri sidecar 模式：spawn Python 进程→emit stdout/stderr 事件→前端监听 |
| **social-auto-upload** | Python | ❌ 无许可 | 13.4K⭐ | 多平台抽象：`BaseVideoUploader` + 每平台独立目录 + Playwright Cookie 认证 |
| **youtube-uploader-mcp** | Go | MIT | 49⭐ | MCP Tool 接口：`Tool{Name,Define,Handle}` 模式，OAuth2 自动刷新，上传+缩略图+字幕分离 |

---

## Phase 5: Agent/教学

| 参考项目 | 语言 | 许可证 | 核心模式 |
|---------|:--:|------|---------|
| **vibe-trading** | Python | MIT | `BaseTool.__subclasses__()` 自动发现 → `check_available()` 条件注册 → `build_registry()` |
| **vibe-trading** | Python | MIT | Skill 目录 + YAML frontmatter 渐进式披露（`get_descriptions()`→按需 `get_content()`） |
| **vibe-trading** | Python | MIT | ReAct 循环 5 层上下文管理（microcompact→collapse→auto_compact→compact_tool→iterative_update） |
| **vibe-trading** | Python | MIT | `AlphaMeta` AST 解析元数据（`load_alpha_meta_from_py()` 不导入模块） |
| **vibe-trading** | Python | MIT | Swarm YAML 预设：agent 定义→task DAG→`depends_on`/`input_from` 上下文注入 |
| **vibe-trading** | Python | MIT | 3 层安全：Prompt 注入扫描（ZWSP 中性化）→MCP 传输守卫→工作区策略 |
| **pa-agent** | Python | MIT | 二元决策树引擎：markdown 模板 + `{{variable}}` 渲染 + JSON 输出合约（`gate_trace`/`decision_trace`） |
| **pa-agent** | Python | MIT | 两阶段门控流水线：诊断（§0-2）→门控→决策（§3-14），门控短路省成本 |
| **pa-agent** | Python | MIT | 增量指标：`EmaState`/`AtrState` dataclass + `_incremental(state, value)` 逐 bar 推进 |
| **pa-agent** | Python | MIT | TradingView Lightweight Charts 前端：2691 行完整仪表盘 |

---

## Phase 6: 自动化运营

| 参考项目 | 语言 | 许可证 | 关键模式 |
|---------|:--:|------|---------|
| **biliup** | Rust | MIT | 多平台直播录制下载器（bilibili/douyin/douyu/huya/twitch/youtube 等 15+ 平台） |
| **social-auto-upload** | Python | ❌ 无许可 | 全平台发布抽象 + Cookie 管理 + biliup 二进制包装 |

---

## 跨 Phase 通用模式

| 模式 | 来源 | 描述 |
|------|------|------|
| **PyO3 分层注册** | czsc, chanlun.rs | 每 crate 独立 `register(py, m)` → facade CDYLIB 聚合，条件编译 `#[cfg(feature = "python")]` |
| **Context 隔离** | WonderTrader, stolgo | 策略只通过抽象接口与系统交互，禁止全局状态 |
| **流式增量计算** | chanlun.rs, pa-agent, czsc | BarGenerator/CZSC `update_bar()` / EmaState / 观察者.投喂K线() — 逐 bar 推进，不重算历史 |
| **Template 渲染** | pa-agent, biliup | `{{variable}}` 模板 + markdown 决策树 + JSON 输出合约 |
| **Builder 模式** | ffmpeg-sidecar | 流畅 API 封装 CLI 参数，直观可发现 |
| **Concurrent Stream** | biliup | `StreamExt::buffer_unordered` 并行 HTTP 上传 |
| **MCP Tool 接口** | vibe-trading, youtube-uploader-mcp | JSON-RPC stdio 暴露能力给 AI Agent |
| **安全三层** | vibe-trading | 注入扫描→传输守卫→工作区策略 |
| **无未来函数** | stolgo | `BarDataView._limit` 截断 + `LookaheadError` + `ThenRule.shift(1)` |

---

## 数据管道关键路径

```
Python: openctp-ctp (SWIG) → OnRtnDepthMarketData (42 字段 tick)
   │
   ▼ Tauri command (Rust↔Python 桥接，待建)
Rust:   TickData 结构体 (参考 openctp CThostFtdcDepthMarketDataField)
   │
   ▼ taiji-bar: BarGenerator (参考 czsc, 纯时间 + 量/幅扩展)
Rust:   RawBar → RawBar (多周期)
   │
   ▼ taiji-dvmi: 极值 + 趋势线 (参考 trendln + pytrendline)
   ▼ taiji-magnet: 磁体定位 (参考 support_resistance)
   ▼ taiji-thrust: 三推计数 (参考 smc-toolkit)
   ▼ taiji-risk: 风控 (参考 stolgo)
```
