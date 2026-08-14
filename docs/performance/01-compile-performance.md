# BitFun 编译与依赖治理计划

> 最近核实：2026-08-13
>
> 实现复核基线：`gcwing/main@a4d944e5b`
>
> 性能 A/B 基线：`gcwing/main@1f538b96d`
>
> 依赖闭包 A/B 基线：`gcwing/main@4781e453c`
>
> 稳定规则：[Rust 构建与依赖边界](../architecture/rust-build-dependency-boundaries.md)

这份文档只维护长期有用的信息：主要成本、已验证收益、下一步顺序和停止条件。模块边界以架构文档为准，具体本地命令由最近的 `AGENTS.md` 维护，单次 PR 的完整命令和日志留在 PR 中。

## 1. 当前结论

| 结论 | 说明 |
|---|---|
| 集成测试链接拓扑已收敛 | Services 两个 crate 的集成 target 总数从 33 降到 25；External Sources 的 adapter/assembly target 从 22 降到 7；五个 Contracts/AI/Assembly crate 又从 28 降到 10，feature、平台和外部系统失败域保持独立 |
| Agent Runtime 基线不再隐藏重型 capability | `bitfun-core/agent-runtime` 只保留生命周期和基础工具 owner；文档转换与订阅认证也改为产品显式 modifier。在最新主线 A/B 中，三平台 normal/build 闭包进一步减少 69/64/110 个版本化 package instance |
| App Server 不继承未消费能力 | App Server 保持现有 Agent/Git/外部来源 handler 边界，不再因 Core 基线携带文档转换和本地订阅凭据，三平台闭包减少 61/56/78 |
| SDK Host 使用显式能力闭包 | SDK Host 保留当前本机协议和工具能力，但不再通过 `product-full` 携带协议未暴露的 Remote Connect、SSH、Function Agent 等能力；Windows/macOS/Linux normal/build 闭包减少 66/68/76 |
| Core 默认值不再代表完整产品 | Core library 的默认 feature 集合为空，能力内部实现依赖回到实际 owner；最新三平台 feature-free 闭包继续减少 30/31/31 个 package instance |
| ACP 按实际宿主拆分角色 | 兼容默认值仍为 client + server；Desktop 只选择 client，CLI 选择两者。Desktop 独立构建不再编译 ACP 的 4,211 行 server/runtime 源码，产品协议与远程行为不变 |
| 完整产品行为保持 | `product-full` 显式组合全部 capability owner；Core 自身不再携带 Desktop host transport而减少 1 个 package，Desktop 闭包不变。CLI 显式保留原先实际生效的 Oniguruma 高亮后端，ACP 默认组合保持原能力 |
| 默认 feature 责任已集中 | 仓内空默认由被依赖 crate 和边界检查负责；workspace member 的 `default-features = false` 从 70 处降到 6 处，仅保留 ACP 两个窄 consumer 与 Relay 独立 Docker 上下文的 4 处必要声明。第三方默认策略尽可能回到 workspace 根，根 lock 只删除 11 个 package、无新增 |
| Installer 删除未使用的直接能力 | 独立 manifest 的直接 dependency 从 18 降到 10，Windows normal/build 闭包减少 6；不把 Installer 并入根 workspace，本 PR 按要求不提交其生成 lockfile |
| focused test 仍保持精确 | 同 owner、feature、平台和进程语义的源文件进入分组 target；使用 `--test <target> <module>::<filter>` 运行单模块 |

## 2. 治理门槛

目标是缩短常用开发、focused test、CI 和打包路径，同时保持产品行为与分层边界稳定。每个治理 PR 必须同时回答：

| 门槛 | 必须回答的问题 |
|---|---|
| Owner | 逻辑属于哪个现有 owner？是否有真实生产消费者？ |
| 行为 | 本地、远程、平台、进程和失败语义如何保持？ |
| 构建图 | 哪个真实产品或测试闭包退出了哪些依赖或 target？ |
| 耗时 | 若宣称提速，是否在同机器、同命令和同缓存状态下测量？ |
| 增量成本 | 是否新增 dependency、feature、test target、CI job 或长期兼容层？ |

以下做法不属于优化：

- 用 `product-full`、`all-features` 或 workspace 全量测试掩盖 feature 边界；
- 为减少重复版本数字强制 patch 平台依赖、宏生态或第三方兼容窗口；
- 为统一形式新建第二套 Runtime、状态 owner、传输层或无消费者抽象；
- 未测量就引入 sccache、替换链接器、合并独立 workspace 或增加 CI job；
- 删除跨平台、负向能力或异常进程行为保护来换取表面时长。

## 3. 当前基线

### 3.1 集成测试链接拓扑

#### Services

本轮只合并 owner 和运行边界相同的测试。`session_write_lock_contracts` 依赖当前测试 executable 启动异常退出子进程，因此继续保持独立；不同 feature 的服务测试也不合并。

| 范围 | 变更前 target | 变更后 target | 集成测试数 |
|---|---:|---:|---:|
| `services-core` 全部 | 20 | 13 | 不变 |
| `services-core/local-storage` | 12 | 5 | 58 |
| `services-integrations` 全部 | 13 | 12 | 不变 |
| MCP | 2 | 2 | 45 |
| 基础 Remote SSH | 2 | 1 | 11 |

Windows、Cargo 1.97.1 的同机独立 `CARGO_TARGET_DIR` A/B 如下。冷构建、无变更重跑和
单叶文件 mtime 触发各测一次；“owner 重建”在依赖已热后对 owner package 执行三轮
clean/rebuild，表中为均值。时间是方向性证据，不是硬阈值。

| 闭包 | 冷构建前→后 | 无变更前→后 | 单叶变更前→后 | owner 重建前→后 |
|---|---:|---:|---:|---:|
| local-storage | 22.14s → 22.04s | 0.56s → 0.55s | 0.99s → 1.06s | 8.30s → 8.16s |
| 基础 Remote SSH | 27.60s → 28.35s | 0.61s → 0.62s | 1.22s → 1.30s | 3.49s → 3.49s |

这些单轮数据不支持“编译明显提速”的结论，也不足以把小幅差值与机器波动区分开。依赖编译仍占
冷路径主导；分组后单叶变更会重链整个职责 target，模块过滤只减少实际运行的测试，不减少该 target
的编译和链接。分组还会降低测试进程级故障隔离粒度，因此当前只合并相同失败域，没有继续扩大。

MCP 的 2→1 candidate 也做过同口径 A/B，但冷构建和 owner 重建均无可区分的提速；streamable HTTP
测试还拥有真实 loopback TCP/SSE/超时失败域，因此最终继续保持两个 target，不计入本轮收益。

#### External Sources adapters 与 assembly

同一 crate 内、相同依赖和运行边界的静态来源合同通过 wrapper target 收敛；测试正文逐字迁移，仍可用
`--test <target> <module>::` 聚焦到单个来源模块。OpenCode 的 MCP 子进程、受管插件服务和 Node 脚本
runtime 分别保留独立 target，避免为了减少链接次数混合不同环境、超时和故障语义。

| 范围 | 变更前 target | 变更后 target | 集成测试数 |
|---|---:|---:|---:|
| OpenCode adapter | 8 | 4 | 130 |
| Claude Code adapter | 4 | 1 | 51 |
| Codex adapter | 3 | 1 | 47 |
| External Sources assembly | 7 | 1 | 30 |
| 合计 | 22 | 7 | 258 |

这部分只减少 15 个重复链接的 test executable；未新增 dependency、feature 或 CI 命令，也不以当前证据
宣称 wall-clock 提速。Cargo 边界检查锁定显式 target、wrapper-only root、leaf 唯一引用和 crate-level cfg，
避免后续新增测试静默绕过分组拓扑。

可重复确认的产物变化如下；`test executable` 包含每个 crate 的 lib test harness，因此比 integration
target 多 1。PDB 大小会随工具链变化，只比较同次 A/B：

| 闭包 | test executable | EXE | PDB |
|---|---:|---:|---:|
| local-storage | 13 → 6 | 25.2 → 19.2 MiB | 135.7 → 91.9 MiB |
| 基础 Remote SSH | 3 → 2 | 3.9 → 2.8 MiB | 53.5 → 43.8 MiB |

#### Contracts、AI adapters 与 Product Assembly

五个纯合同/组装 owner 使用显式 wrapper target；AI 的纯协议测试与真实 loopback SSE 测试继续分成两个
失败域，Product Domains 的默认、Plugin Source、External Sources、Function Agent 与 MiniApp 也继续按
owner feature 分开。270 个 integration tests 不变，模块过滤仍可聚焦单个 leaf：

| 范围 | 变更前 target | 变更后 target | 集成测试数 |
|---|---:|---:|---:|
| `core-types` | 4 | 1 | 10 |
| `runtime-ports` | 5 | 1 | 21 |
| `product-domains` | 9 | 5 | 179 |
| `ai-adapters` | 7 | 2 | 29 |
| `product-capabilities` | 3 | 1 | 31 |
| 合计 | 28 | 10 | 270 |

对应五个 lib test harness 的 test executable 总数从 33 降到 15；workspace integration target 从
91 降到 73。该变化减少 18 次重复链接，但单叶变更会重链所属分组，因此这里只报告确定的拓扑收益，
不在缺少同机多轮 A/B 时宣称 wall-clock 提速。边界检查锁定 exact leaf、owner feature 和空
`required-features` 的默认 target，避免以后用 `product-full` 扩大测试闭包。

### 3.2 依赖与 feature

闭包使用 `cargo tree -e normal,build` 按目标平台统计版本化 package instance；它衡量进入编译图的
package/version，不等同于实际秒数。路径 package 因 A/B worktree 路径不同不参与集合差值。

| 产品闭包 | Windows | macOS | Linux | 说明 |
|---|---:|---:|---:|---|
| Core `agent-runtime` | 449 → 343 | 435 → 330 | 485 → 375 | 基线退出具体 service/tool capability，不改变 Runtime 生命周期 owner |
| Core `product-full` | 570 → 570 | — | — | 完整产品显式恢复所有 owner；Windows 抽样闭包不变 |
| CLI | 649 → 649 | — | — | 入口显式选择其现有能力，Windows 闭包不变 |
| ACP | 599 → 589 | 587 → 574 | 616 → 594 | 退出过去由 Core 基线暗带、但 ACP 未选择的能力 |
| Desktop | 792 → 792 | 807 → 807 | 892 → 892 | 完整产品继续使用既有跨平台截图行为，本轮不以扩大根 lock 依赖宇宙换取单平台闭包下降 |
| Installer | 333 → 327 | — | — | Windows 独立 workspace；直接 dependency 18 → 10 |

下表前五项延续 `gcwing/main@734e5b05f` 的已核实 A/B，SDK Host 行以
`gcwing/main@22f5411e7` 为变更前基线。三平台 target 分别为 `x86_64-pc-windows-msvc`、
`aarch64-apple-darwin` 和 `x86_64-unknown-linux-gnu`；计数先移除 Cargo tree 的重复展示标记
`(*)`，再按 package/version 去重：

| 本轮闭包 | Windows | macOS | Linux | 行为边界 |
|---|---:|---:|---:|---|
| Core `agent-runtime` | 343 → 274 | 330 → 266 | 375 → 265 | 文档扩展识别保留；转换和本地订阅凭据明确不可用 |
| App Server | 490 → 429 | 477 → 421 | 508 → 430 | 现有 handler/DTO 保持，未消费的两个能力退出 |
| Core `product-full` | 570 → 570 | 557 → 557 | 601 → 601 | 显式恢复 `document-read` 与 `subscription-auth` |
| CLI | 649 → 649 | 649 → 649 | 672 → 672 | 显式保持原有能力 |
| ACP | 589 → 587 | 574 → 572 | 594 → 592 | 保持原有能力，同时退出 Reqwest 未使用的 `mime_guess`/`unicase` |
| SDK Host | 578 → 512 | 565 → 497 | 609 → 533 | 保留本机 SDK profile、九组工具 owner、外部静态来源和 ring TLS；退出未暴露的 Remote Connect、SSH、Function Agent 与完整产品附属能力 |

本轮没有新增 crate 或第三方 package；SDK Host 只把已有测试依赖 `rustls` 调整为进程入口实际使用的
normal dependency，根 lock package 集合不变。前两类收益来自 `anydoc` 及其文档解析/压缩依赖，
以及订阅凭据的 keyring/加密/本地存储依赖；SDK Host 的收益来自未公开远程能力对应的
SSH、密钥和连接子图退出。完整产品 package 集合不变，
因此这里只报告依赖图收敛，不宣称 `product-full` wall-clock 提速。

以下是以 `gcwing/main@3d8ee4bc0` 为变更前基线、使用同样三个 target triple 和去重口径复算的最新 A/B：

| 最新闭包 | Windows | macOS | Linux | 行为边界 |
|---|---:|---:|---:|---|
| Core `--no-default-features` | 104 → 102 | 93 → 91 | 92 → 90 | 删除 Core 不再消费的 `tokio-stream`、`urlencoding` 直接边 |
| Core `product-full` | 570 → 570 | 557 → 557 | 601 → 601 | 完整产品仍从真实 adapter/service owner 获得两项依赖 |
| CLI | 649 → 643 | 649 → 642 | 672 → 665 | 删除未调用的 `syntect-tui`/`dashmap`；显式保留既有 Oniguruma 高亮后端 |
| Desktop | 792 → 790 | 807 → 805 | 892 → 887 | 删除从未注册、没有调用方的 global-shortcut 插件和 ACL |
| MiniApp Market | 205 → 204 | 208 → 207 | 206 → 205 | 删除服务从未消费的 `urlencoding` 直接边 |
| Page Function tests | 38 → 35 | 38 → 35 | 38 → 35 | 删除同步 Rust 测试未使用的 dev-only Tokio 闭包 |

以下继续以 `gcwing/main@7345619ac` 为变更前基线，记录 Core 默认值与 ACP 角色边界收敛。三平台、
依赖类型与去重口径与上表一致：

| 本轮闭包 | Windows | macOS | Linux | 行为边界 |
|---|---:|---:|---:|---|
| Core 隐式默认 | 570 → 93 | 557 → 82 | 601 → 81 | library 默认不再冒充完整产品；只保留 feature-free facade 与 build dependency |
| Core `--no-default-features` | 102 → 93 | 91 → 82 | 90 → 81 | 四条 Core 直接边退出并回到实际 owner；package 集合净减 9 |
| Core `product-full` | 570 → 570 | 557 → 557 | 601 → 601 | Desktop/Server 等完整产品入口仍显式恢复全部能力 |
| ACP 默认兼容组合 | 587 → 587 | 572 → 572 | 592 → 592 | 默认仍精确组合 client + server，独立 ACP 测试与外部兼容行为不缩小 |
| Desktop | 790 → 790 | 805 → 805 | 887 → 887 | package 集合不变；ACP 仅编译 client 模块，server/runtime 4,211 行退出该 package build |
| CLI | 643 → 643 | 642 → 642 | 665 → 665 | 显式选择 ACP client + server，既有 CLI-hosted server 行为不变 |

ACP 两个新角色的当前独立闭包为 client 397/390/391、server 533/518/539（Windows/macOS/Linux）。
它们不能直接相加：Cargo 会对共同依赖去重。该拆分的确定收益是 Desktop 独立构建不再编译 ACP server
模块，而不是 Desktop package 数下降；因此不宣称完整 Desktop wall-clock 提速。

Core 空闭包减少的 9 个 package instance 主要来自 `base64` 与 `futures` 的独有子图；`regex` 和
`tokio-util` 的 Core 直接边虽然已经移除，但 package 仍由 feature-free contracts/services 路径传递保留。
因此本轮证明的是 direct owner 边界收敛，不能把四条 direct edge 都描述成 package 完全退出。

Core 的 bare/default 编译契约本轮发生了有意变化：仓内产品消费者此前已经全部关闭默认 feature 并显式
选择 owner，因此运行行为不变；仓外若有 path/git consumer 依赖旧的隐式完整表面，需要显式选择
`product-full`，或改为列出实际使用的 owner。该迁移属于编译期契约变化，不能描述成对未知外部 consumer
完全无影响。

Syntect 不能机械地只删适配层：旧 feature union 同时启用 `regex-fancy` 与 `regex-onig` 时，实际由
Oniguruma 后端处理。当前 manifest 直接选择 `regex-onig`，因此运行后端、默认 syntax/theme 和
Syntect→Ratatui 样式转换保持不变，同时让未生效的 fancy 后端与未消费的 YAML loader 退出。

Package instance 会低估“同一个大 crate 少编译了多少 feature 代码”。在 Windows
`agent-runtime` 闭包中，`bitfun-services-integrations` 的 Cargo active feature 从 61 个降到 6 个，
只保留 `workspace-search` 及其 5 个直接依赖 feature；`bitfun-product-domains` 从 13 个降到 5 个，
只保留 Agent Runtime 实际使用的 external-subagent contract slice。Function Agent、MiniApp、
Plugin Source 由各自 owner 选择，完整产品仍经 `product-full` 显式恢复。

上一轮根 `Cargo.lock` 从 1176 降到 1169，精确删除 `syntect-tui`、`custom_error`、`fancy-regex`、
`yaml-rust`、`linked-hash-map`、`tauri-plugin-global-shortcut` 和 `global-hotkey`；没有新增、升级或
降级 package。本轮 feature/角色边界调整保持该 lockfile 字节不变，也没有新增第三方 package。
Installer 自己生成的 `BitFun-Installer/src-tauri/Cargo.lock` 不提交。

以下以 `gcwing/main@a4e06cae3` 为变更前基线，完成 Core feature-free 依赖基线的剩余 owner
收敛。统计仍使用相同三个 target triple、`normal,build` 边和版本化 package instance 去重口径：

| 最新闭包 | Windows | macOS | Linux | 行为边界 |
|---|---:|---:|---:|---|
| Core `--no-default-features` | 93 → 63 | 82 → 51 | 81 → 50 | Fluent runtime、Tool Contracts、host transport、诊断/Diff 实现与非基线 Tokio 子图退出；locale/config/path 稳定契约保留 |
| Core `product-full` | 570 → 569 | 557 → 556 | 601 → 600 | 显式恢复 I18n、Agent 与全部产品 owner；仅不再经 Core 携带 Desktop host transport |
| Desktop | 790 → 790 | 805 → 805 | 887 → 887 | Desktop 原本已直接拥有 transport，完整宿主依赖图和行为不变 |
| Services Core feature-free | 22 → 15 | 22 → 15 | 22 → 15 | Regex、Similar 与 Tokio 全部退出；只保留同步稳定 contract、JSONC 与路径规范化 |
| Codex Adapter | 79 → 72 | 78 → 71 | 78 → 71 | 只使用同步 workspace path contract，不再继承 Services Core 的文本实现依赖 |
| Static Hook Support | 72 → 65 | 73 → 66 | 72 → 65 | feature-free Services Core 依赖边不再携带未消费的文本实现依赖 |
| Claude Code Adapter | 85 → 82 | 84 → 81 | 84 → 81 | Markdown owner 仍保留 Regex；Diff 与 Tokio 退出 |
| OpenCode Adapter | 141 → 140 | 140 → 139 | 140 → 139 | 既有 Markdown/Tokio owner 保留，仅未消费的 Similar 退出 |

Core 的直接 Tokio capability 从 `fs/io-util/macros/net/rt/sync/time` 收敛为 `fs/sync`；异步宏、
网络以及产品 runtime 能力由现有 `agent-runtime`、`browser-control`、`debug-log`、`lsp` owner
显式选择。feature-free Core 仍通过 `bitfun-services-core/json-io` 获得其原子 JSON 写入所需的
`rt/time`，测试使用的多线程运行时只留在 dev-dependency。`runtime-ports/permission` 没有被机械移出：`GlobalConfig` 与项目权限文件公开 DTO
确实在 feature-free facade 中使用它，继续保留比制造条件 API 更符合契约稳定性。

Services Core 的 feature-free profile 进一步不再依赖 Tokio。同步路径规范化仍可直接使用；异步受限
workspace 读取由 `workspace-text-runtime` 选择，诊断日志脱敏与本地 Diff 分别由独立 owner feature
选择。Core 继续通过同名 feature 保留原 facade，`product-full` 显式组合两者，避免把依赖收敛变成
完整产品的源码或行为回归。

本轮有意收紧了数项编译期契约。直接使用 feature-free Core 的仓外 consumer 若调用
`I18nService`，必须显式选择 `i18n-runtime`；旧的
`bitfun_core::infrastructure::events::TransportEmitter` 导入路径不再提供，host adapter 应直接从
`bitfun-transport` 导入。直接依赖 feature-free `bitfun-services-core` 的 consumer 若调用诊断脱敏、
本地 Diff 或异步 workspace 文本 API，必须分别选择 `diagnostics`、`diff` 或
`workspace-text-runtime`；通过 Core 兼容 facade 调用前两者时选择同名 Core feature。仓内真实 consumer
已全部迁移，完整产品的运行时事件、翻译和服务行为不变，但这些源码迁移不能描述为对未知外部
consumer 零影响。

该轮结束时根 lock package 为 1169，新增、升级、降级 package 均为 0。由于 Core 删除本地
`bitfun-transport` 直接边，`Cargo.lock` 的 Core dependency record 同步删除这一行；这是依赖边
收敛，不是 package 集合增长，也不通过保留无 owner 的 optional dependency 伪造字节不变。

以下以 `gcwing/main@aeb8099ae` 为变更前基线，继续收敛两个稳定契约 crate 的源码与依赖可见面。
统计仍使用 `x86_64-pc-windows-msvc`、`aarch64-apple-darwin`、`x86_64-unknown-linux-gnu`，并按
`cargo tree -e normal,build --no-dedupe` 的版本化 package instance 去重：

| 契约/消费闭包 | Windows | macOS | Linux | 边界结果 |
|---|---:|---:|---:|---|
| Runtime Ports feature-free | 21 → 13 | 21 → 13 | 21 → 13 | Agent API、服务端口、插件与脚本端口按 owner feature 退出；common marker/结果契约保留 |
| Tool Contracts feature-free | 25 → 18 | 25 → 18 | 25 → 18 | ACP/MCP bridge、Computer Use 与 element-token 源码按独立 feature 退出 |
| Terminal Core | 94 → 92 | 82 → 80 | 81 → 79 | 只选择 terminal port，不再编译无关 Session/Git/Remote workspace 表面 |
| Plugin Runtime Client | 22 → 16 | 22 → 16 | 22 → 16 | 只选择 plugin-runtime contract，不再继承完整 Runtime Ports 表面 |
| Tool Runtime | 78 → 78 | 66 → 66 | 65 → 65 | 真实 owner 仍消费稳定 handle 组合，package 闭包不变 |
| ACP client | 385 → 385 | 378 → 378 | 379 → 379 | 依赖重叠使 package 数不变；组合闭包启用 ACP bridge 与 Core 所需 Computer Use contract，不启用 MCP/element-token |
| ACP server | 522 → 522 | 507 → 507 | 528 → 528 | 组合闭包启用 Core 所需 Computer Use/MCP contract，不继承 ACP client bridge 或 element-token |

Package instance 会低估同一 crate 内源码切片的收益。Runtime Ports 变更前约 6,045 行源码全部进入任意
consumer；当前 feature-free 非测试表面约 314 行，约 95% 的 Agent/服务/plugin/script 端口源码退出最小编译。
Tool Contracts 约 6,846 行源码中，ACP/MCP bridge、Computer Use 与 element-token 共约 2,861 行
（41.8%）由各自 feature 控制。这里报告的是解析、类型检查和增量重编译输入收敛；没有重复的冷/热构建
样本，因此不宣称完整产品 wall-clock 提速。完整产品和真实 owner 显式恢复原 API，runtime ownership、
wire shape 与行为不变。

`bitfun-agent-runtime` 的直接 Runtime Ports 边也不再选择其源码未消费的 `remote-exec-port` 与
`tool-runtime-handles`；Core 的 `agent-runtime` owner 仍显式选择二者，因为具体 assembly 路径确实使用这些句柄。

这次切片包含有意的编译期可见性收紧：直接依赖 Runtime Ports 或 Tool Contracts 的仓外 consumer 需要
显式选择所用 port/bridge/Computer Use/element-token feature。启用相应 owner 后原公开路径保持不变，
仓内 consumer 已逐项闭合；不能把 feature-free 构建下的源码不可见描述为对未知外部 consumer 零影响。

本轮没有新增、升级或降级第三方 package，根 `Cargo.lock` 保持字节不变；也没有新增 CI job、矩阵或命令。
Runtime Ports 的 owner-specific integration targets 保持彼此独立，避免为了减少 executable 数重新制造
feature union。

以下以 `gcwing/main@a4d944e5b` 为变更前基线，统一默认 feature 的责任位置。该主线相对前次测量
没有 Cargo 输入变化；统计继续使用同样三个
target triple 和 `normal,build` 版本化 package instance 去重口径；它只描述编译图，不直接代表墙钟时间：

| 闭包 | Windows | macOS | Linux | 结果 |
|---|---:|---:|---:|---|
| Core feature-free | 63 → 63 | 51 → 51 | 50 → 50 | 空默认与显式 owner feature 不变；只删除 consumer 侧冗余开关 |
| Core `product-full` | 569 → 569 | 556 → 556 | 600 → 600 | 完整产品 owner 集合与运行能力不变 |
| CLI | 642 → 640 | 641 → 639 | 664 → 662 | Markdown 只使用 parser，退出未消费的 `getopts` 与 HTML renderer |
| Desktop | 790 → 790 | 805 → 793 | 887 → 887 | BitFun 的 macOS Objective-C 直接依赖边按实际 imports 选择 feature；Tauri/wry 等第三方仍合并自身所需 feature |
| Relay Service | 190 → 190 | 193 → 193 | 192 → 192 | 独立 Docker manifest 保持显式版本与默认策略，闭包不变 |
| Services Integrations feature-free | 26 → 26 | 28 → 28 | 27 → 27 | Qrcode 的 PNG/SVG 能力仍由 `remote-connect` owner 显式选择 |

workspace member 中显式 `default-features = false` 从 70 处降到 6 处：62 个仓内空默认重复声明删除，
MiniApp/Skin Market 的 2 个 SQLx 声明改为继承 workspace 根策略。剩余 6 处中，两处是 Desktop/CLI 对 ACP
的必要例外，因为 ACP 有意保留 `client + server` 兼容默认，而两个产品入口必须分别选择 client-only 和
双角色；另 4 处属于 Relay Server、Relay Service 和 Page Function Runtime，它们会被 Docker 单独复制、
无法继承 workspace 根，因此继续显式声明 Tokio、SQLx 与 RquickJS 的默认策略。

第三方默认收敛只处理有源码与构建证据的依赖：Futures 保留 `std`，Tracing 保留 `std`，Chrono 保留
`serde + clock + std`；Tokio Stream 的真实 consumer 只使用 feature-free 的 Receiver/iter wrappers；
Remote Connect 显式选择 qrcode 的 `image + svg`；Pulldown-Cmark 不启用 CLI 未使用的命令行/HTML renderer；
BitFun 的 macOS Objective-C binding 直接依赖边只选择源码导入的类型和所需 `std`；Cargo 的最终
feature union 仍包含 Tauri、wry、notification、updater 等第三方路径的需求。根 `Cargo.lock` 从 1169
降到 1158，新增 package 为 0；删除项仅来自
Pulldown-Cmark 的 `getopts`/HTML escape 子图和未使用的 Objective-C framework binding。未关闭 Axum、Tauri、
Clap、Tracing Subscriber、Notify 等默认即产品契约或缺少独立收益证据的依赖。

| 状态 | 范围 | 处理结论 |
|---|---|---|
| 已稳定 | 根 `Cargo.lock`、Reqwest Rustls 单栈、workspace Tokio 最小基线 | 不重复治理 |
| 本轮完成 | Core 空默认与 capability-local 工具依赖、ACP client/server 角色、Core Agent Runtime capability、文档转换与订阅认证 modifier、SDK Host 显式 owner closure、Installer/CLI/Desktop/Core/MiniApp Market/Page Function 未使用直接依赖 | 以真实入口 closure 收敛，不建立新的产品 umbrella；根 lock 不增加 package |
| 当前不动 | App Server / Server | 只为保持现有 handler 编译显式声明其已消费的 Core owner；不在改造稳定前继续拆其生产路径 |
| 明确保留 | Desktop screenshots backend | 替换方案必须同时保持三平台坐标/权限/区域捕获语义且不增加根 lock package；当前候选不满足 |
| 明确保留 | `portable-pty 0.8/0.9` | 非 OHOS 与 OHOS 的平台兼容选择，不为去重破坏 |

重复版本数量只用于发现候选，不能直接转化为治理任务。`oxc`、`rquickjs`、vendored `git2`、`sherpa-onnx` 等重依赖都有真实 capability owner；只有某个产品入口不消费对应能力时，才允许让它退出该入口的构建图。

### 3.3 CI 与本地验证

- 现有 CI 已覆盖 workspace check、Core/Desktop lib、平台敏感 owner 测试和独立 runtime/CLI 验证；本轮不新增 job、矩阵或 changed-path 分类器。
- CI 不负责穷举所有测试；新增验证只有具备独立 owner、平台矩阵或失败归因价值时才进入既有流水线，否则由最近模块的 focused command 维护。
- 本地从 owner 文档的最小 package/target/feature 入口开始；仅名称过滤不能阻止无关 target 编译。
- CI 收敛必须先有多次 job/step 耗时、缓存状态和失败历史；`SKIPPED`、未触发或只编译未运行都不算通过证据。

## 4. 已完成，不再重复实施

| 主题 | 当前结果 |
|---|---|
| 前端构建 | Monaco 运行时加载已统一；Web type-check/Vite 并行；Web/Mobile TS 已启用 incremental |
| 开发循环 | mobile-web 支持输入 mtime 短路；Vite 默认使用原生文件事件；前端准备步骤已并行 |
| Rust profile | release 使用 thin LTO；dev 使用 `line-tables-only` 和高 codegen-units，并保留调试逃生口 |
| 可复现解析 | 根 lockfile 已提交，普通 CI 使用 `--locked`；build.rs 输出已排序 |
| CI 拓扑 | Rust job 不再等待完整前端构建，自建 Tauri 检查所需资源目录 |
| 依赖收敛 | Desktop 直接 image 版本和 Reqwest TLS 双栈已治理 |
| Agent Runtime 闭包 | Core 基线不再暗带具体 capability；完整产品和 CLI 显式保持原能力，ACP 退出未选择闭包 |
| Core/ACP 默认与角色 | Core 默认 feature 为空；ACP 默认精确保持 client + server，Desktop client-only、CLI 双角色均由现有边界检查锁定 |
| 重型可选能力 | 文档转换和本地订阅凭据由弱 modifier 细化已有 runtime owner；Core 基线和 App Server 退出未消费闭包 |
| Installer 闭包 | 删除 8 个未使用直接 dependency；独立 workspace 和发布生命周期不变，本 PR 不提交其生成 lockfile |
| SDK Host 闭包 | 从 `product-full` 改为与当前协议/构造路径一致的显式 Core owner closure；保留 ring TLS 初始化，本机 SDK 行为不变，未交付的远程执行能力不再进入构建图 |
| Agent Runtime 测试 | 28 个 integration executable 已收敛为 5 个职责/平台 target |
| Services 测试 | 两个服务 crate 使用显式 target；选中闭包少 8 个 integration executable，进程/feature/external-system 边界保持独立 |
| External Sources 测试 | 四个 adapter/assembly crate 从 22 个 target 收敛到 7 个；MCP、插件服务和脚本 runtime 继续独立 |
| Contracts/AI/Assembly 测试 | 五个 crate 从 28 个 target 收敛到 10 个；AI loopback 与纯协议、Product Domains 各 owner feature 保持独立 |
| 未使用直接依赖 | 删除 CLI/Desktop/Core/MiniApp Market/Page Function 的失效直接边；保留 Syntect 实际 Oniguruma 后端，根 lock 只减 7 个 package |

内置 Agent 内容已经移到无第三方依赖的 `bitfun-agent-content`，减少了 Core build-script 工作；
但 Core 仍直接依赖该 crate。没有足够产品收益前，不为消除这一编译指纹引入动态 provider、
运行时文件读取或资源协议。

## 5. 后续顺序

本轮之后先观察，不立即再开同类“小修补”PR。需要真实 CI 样本或上游条件成熟后，按以下顺序重新核实：

| 范围 | 启动条件 |
|---|---|
| CI 收敛 | 先积累多次相同 owner 的 step wall-clock、cache hit/miss 和失败历史；只有能证明收益且不会静默缩小覆盖时再独立设计 |
| Desktop 截图后端 | 新候选同时满足三平台行为等价、区域捕获无性能回退、系统依赖可 feature-gate，且根 lock package 不增加 |
| App Server / Server | 当前改造合入并稳定后，重新锁定最新生产调用链和可信 owner 边界 |
| 其他产品入口重型 capability | 证明入口不消费该能力，具备 typed unsupported/fallback 行为，并能让一个真实重依赖子图退出 |
| 重复 native/sys 库版本 | 同一 owner 能升级收敛且三平台打包/ABI 有证据；不因版本数字重复强行 patch |

每一步都在前一 PR 合入后的最新 main 重新测量。无法证明边界或收益时停止，不为了完成清单继续重构。

## 6. 每轮 PR 的证据

PR 描述维护一张简表即可，不新增全仓依赖台账：

| 证据 | 变更前 | 变更后 |
|---|---:|---:|
| 真实产品 normal/build closure |  |  |
| owner focused-test closure/target |  |  |
| 冷、热或增量耗时（同机器、命令、缓存状态） |  |  |
| 产物数量/大小 |  |  |
| 新增 dependency、feature、test target、CI job |  |  |

同时记录功能不变量、远程/平台差异、实际运行的最小验证和未运行的 CI。若产品 closure 不变，只能说明测试拓扑或 owner 边界收益，不能宣称产品构建已经变快。
