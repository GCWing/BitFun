# Rust 构建与依赖边界

本文定义 BitFun Rust workspace 的 Cargo feature、第三方依赖、测试目标和验证职责边界。它是
[`product-architecture.md`](product-architecture.md) 的构建视图补充；运行时 owner、端口和产品分层仍以产品架构及最近的模块 `AGENTS.md` 为准。

本文只记录长期规则，不记录当前 package 数量、重复版本数量或单次构建耗时。阶段性审计和迁移顺序保留在本地工作记录或对应 issue/PR，避免把过程文档和会变化的基线提交为可不断调高的门槛。

## 1. 适用范围

以下改动必须同时遵守本文：

- workspace 或 crate `Cargo.toml` 的 dependency、feature、target 和 profile 变更；
- `bitfun-core`、Assembly 或产品入口的 capability 装配；
- 为降低构建/测试时间而做的 crate 拆分、owner 迁移或第三方库替换；
- build script、proc-macro、TLS/crypto、系统库和其他重型原生依赖的引入或扩展；
- integration test、example、binary 的 feature/target gate；
- 本地验证命令和 CI 验证职责的调整。

单纯减少依赖数量不是架构目标。依赖必须由实际能力和 owner 决定；构建收益不能作为向上依赖、重复实现或错误共享生命周期的理由。

## 2. 四类决策必须分离

| 概念 | 决策时机 | Owner | 表达内容 |
|---|---|---|---|
| Cargo feature | 编译期 | capability owner crate | 哪些代码和依赖进入编译图 |
| Delivery Profile | 产品装配/构建期 | app 与 Assembly | 一个产品入口选择的 capability 集合 |
| Runtime Config | 运行期 | domain/service owner | 已编译能力范围内的用户或组织配置 |
| Capability Availability | 运行期事实 | owner port/provider | 能力是否已编译、注册且当前可用 |

Delivery Profile 是架构概念，不是新的通用 manifest、schema 或运行时对象。当前产品入口可直接通过对 owner crate 的显式 feature 选择表达它；只有存在真实的多个构建消费者且静态声明不足时，才引入更高层装配对象。

Runtime Config 不能让未编译能力凭空可用，也不能代替 target/feature gate。Capability Availability 必须反映真实 provider 状态，不能只根据配置值或静态 catalog 推断。

## 3. Cargo feature 与依赖声明规则

### 3.1 Feature 保持加法语义

- 打开一个 feature 可以增加 API、实现或依赖，不应移除已有 API 或改变已启用能力的含义；
- 不使用互斥 feature 表达产品枚举。互斥实现优先由 target、独立产品入口或 owner provider 选择；
- `default` 只承载明确的兼容默认值，不是新产品入口的 capability 选择方式；
- 可选依赖由声明它的 crate feature 激活，消费者不得越层拼装 owner 的内部依赖；
- 一个 feature 应对应稳定能力边界，而不是临时 PR、UI 页面或单个调用点。

Cargo 会统一同一 package 在依赖图中的 feature；workspace dependency 与成员声明的 feature 也会相加。因此共同声明必须保持最小，产品入口无法撤销下层已经开启的重 feature。具体解析语义见 Cargo 的
[`Features`](https://doc.rust-lang.org/cargo/reference/features.html) 和
[`workspace.dependencies`](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-dependencies-table) 文档。

### 3.2 产品入口显式选择 Core 能力

`src/apps/*`、`src/crates/interfaces/*` 和 installer app 直接依赖 `bitfun-core` 时必须同时：

1. 设置 `default-features = false`；
2. 声明非空 `features` 列表。

入口应选择真实需要的 owner feature；`product-full` 只能描述确实需要完整产品装配的兼容入口，不能作为尚未完成 feature/owner 分解时的占位解法。缩小某个产品的 capability 集合时必须从实际 construction/command path 反推，并保留行为等价或明确 unsupported-state 测试。

### 3.3 Workspace dependency 只提供共同底座

- workspace 声明负责版本和真正跨产品共享的最小 feature；
- runtime、HTTP、TLS、crypto、codec 等产品特定 feature 留给实际 app/service/adapter owner；
- `full` feature 只有在所有真实消费者都需要且更窄集合不能稳定维护时才允许；
- target-specific dependency 放在最接近平台实现的 owner，不因单一平台需求污染跨平台 crate；
- 修改共享 dependency feature 视为构建影响变更，必须检查真实产品组合的 feature graph。

### 3.4 Reqwest TLS 后端由客户端 owner 选择

- workspace 级 `reqwest` 只统一版本以及跨产品共享的 HTTP、序列化和流能力，不启用 TLS 后端；
- 真正创建 HTTPS client 的 app、service 或 adapter 必须在自身依赖声明中显式选择 `reqwest/rustls`，只使用 `reqwest::Url` 的 contract/assembly 路径不加载 TLS；
- capability crate 的每个 Reqwest owner feature 必须独立带齐 `reqwest/rustls`，不能依赖 `product-full` 或其他 feature 的 Cargo feature-union 偶然补齐；
- 边界检查以 Cargo metadata 的解码结果看护全部直接 consumer，并检查 resolved Reqwest feature union，防止传递依赖重新激活 Native TLS；
- 不并列启用 native-tls 兼容栈。只有真实产品场景无法由 Rustls 平台证书验证承载时，才以明确行为证据评审替换方案，而不是重新叠加第二后端。

## 4. 依赖 owner 与准入检查

第三方库应位于调用外部系统或实现具体能力的最低合理 owner：

- 协议/外部数据形状翻译属于 adapter；
- OS、进程、文件系统、网络、Git、MCP 等具体实现属于 service；
- 可移植的 agent/tool/harness 原语属于 execution；
- DTO、事件和端口属于 contracts，保持行为轻量且不得依赖上层；
- Assembly 选择和连接 capability，不实现具体 adapter、OS 或 service 细节；
- app 只拥有入口、平台生命周期和产品呈现，不复制可复用服务逻辑。

新增依赖或显著扩大已有依赖 feature 时，PR 描述至少给出：

| 字段 | 评审问题 |
|---|---|
| Owner | 哪个 crate/模块拥有该外部能力？ |
| Product consumers | 哪些产品形态实际需要？ |
| Activation | 始终启用、target 还是 Cargo feature，为什么？ |
| Build cost | 是否包含 proc-macro、`build.rs`、C/C++/系统库、TLS/crypto？ |
| Version convergence | 是否新增重复版本，直接依赖升级能否安全收敛间接版本？ |
| Existing alternatives | 标准库或现有三方库为什么不能合理承载？ |
| Boundary fit | 为什么该依赖属于当前层和 owner？ |
| Smallest verification | 哪个最小 check/test 能证明能力和边界？ |

该记录保留在 PR 中，不新增全仓机器可读依赖台账。只有低误报、可长期稳定执行的结构事实进入 boundary checker。

### 4.1 重复版本与间接依赖

- 先用 `cargo tree -d` 确认重复的是可收敛版本、平台分支还是生态尚未统一的 major；
- 优先调整直接依赖的兼容版本，让 Cargo 自然收敛间接版本；
- 不用 `[patch]`、强制 lockfile pin 或降低版本来掩盖真实不兼容；
- proc-macro、TLS/crypto、HTTP types、Windows/system bindings 的重复版本需额外关注，但仍以 API/ABI 和 owner 兼容为前提；
- 版本收敛必须运行 owner 的行为测试；仅看到依赖树节点减少不足以证明安全。

### 4.2 替换不活跃或重叠库

替换前同时评估维护活跃度、安全公告、MSRV、目标平台、许可证、API 迁移成本、feature 粒度、原生构建成本和现有 adapter 行为。缺少近期 release 本身不是替换理由；稳定且边界清晰的库可以低频发布。

只有同一 owner 内职责重叠、统一后不会扩大依赖闭包且能保持行为时，才合并到一个库。API 看似相似的 HTTP client、协议 adapter、认证、runtime 或 crypto 实现不能跨 owner 强行统一。

## 5. 拆分、合并与 owner 迁移准则

拆 crate 或迁移 runtime owner 至少要满足一项：

1. 稳定能力可脱离产品装配独立编译和测试；
2. 现 owner 混合协调策略与具体 adapter/service 实现，形成向上依赖或明显大闭包；
3. 多个产品需要同一稳定 port/fact，但不应共享 UI、协议、认证、生命周期或平台实现；
4. 重型依赖只服务少数能力，拆分后不使用它的产品和测试能退出该构建图；
5. 能用行为等价测试覆盖旧路径，并给兼容入口定义删除条件。

仅为减少文件行数、追求“一 crate 一 feature”、只有单一消费者且没有闭包收益，或必须先建立通用注册框架才能成立时，不拆 crate。可以先在原 crate 内按 owner 拆模块；只有物理 crate 边界带来真实依赖或独立验证收益时再升级。

DTO/contract 抽取不等于 runtime owner 迁移。迁移 owner 必须先审查 port/provider、旧路径兼容、状态与错误语义、远程 workspace 行为和行为等价测试。

## 6. Test target 与 feature 组合

- feature-gated `[[test]]`、`[[bin]]`、`[[example]]` 使用 Cargo `required-features`，避免目标在能力不可用时仍进入构建图；
- integration test 的 crate-level feature cfg 与 Cargo `required-features` 必须精确陈述同一组正向 AND 条件，不能额外加入扩大最小测试闭包的 umbrella feature；`not(feature)` 留在 Rust cfg 中，feature OR 条件应拆成独立 target，因为 Cargo target gate 无法等价表达；
- integration test 只依赖被测 owner 的公开契约，不通过 `product-full` 获取测试便利；窄 feature 尚不能独立编译时，应将其记录为待拆分的 owner/feature 边界并保持现有 target 声明，不得新增或扩大 `product-full` 来制造已经收敛的假象；
- 纯解析、策略和状态转换优先使用无外部系统的 owner-local fixture；
- 需要真实 adapter/service 的测试单独作为 feature integration target；
- 测试常用、真实的 feature 组合，不穷举指数级组合；
- `--all-features` 用于兼容审计，不代替目标产品的最小组合测试。

Cargo target gate 语义见
[`required-features`](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#the-required-features-field)。

## 7. 本地最小验证与 CI 分工

本地验证由改动 owner 和影响面决定：

| 改动 | 默认最小本地验证 |
|---|---|
| Cargo manifest、feature、crate dependency 边界 | `pnpm run check:core-boundaries:test` 和 `pnpm run check:core-boundaries` |
| 单 crate Rust 实现 | `cargo check -p <owner>`；行为变化再加最近的 `cargo test -p <owner> <filter>` |
| 单 capability feature | `cargo check -p <owner> --no-default-features --features <feature>` |
| 单 integration target | `cargo test -p <owner> --test <target>` |
| workspace dependency、低层公开 contract、build script | 按真实影响升级到 workspace check 和相关产品构建 |

CI 负责 workspace 级检查、真实产品 feature 组合、跨平台、完整测试和最终产品构建。本地未执行的宽泛验证必须标记为未执行或 CI 覆盖，不能表述为本地通过。CI 失败时再按失败路径复现对应重命令，不要求每次本地改动预跑全部构建。

### 7.1 Hosted CI 关键路径与缓存

- 验证 job 只依赖自身的编译期前置条件。Tauri `check`/`test` 只要求配置中的前端和资源目录存在时，Rust job 自行创建空目录，不等待或传递可发布前端产物；真实静态资源仍由前端构建和产品打包 owner 负责。
- Pull Request 可以恢复可信分支产生的 Cargo 缓存，但不得写入 merge-ref 缓存。只有可信 `main` push 可以保存共享缓存；若依赖编译已经完成而后段测试失败，允许该可信构建保存依赖缓存，避免下一次跨平台构建无谓冷启动。
- 未先修改 Cargo manifest 的 PR/main 验证 job 以仓库提交的 `Cargo.lock` 为唯一解析结果并通过 `--locked` 验证，不在 cache restore 前重新生成 lockfile。依赖解析更新必须作为可评审的源码变更提交，不能让同一 commit 因上游兼容版本发布而自然产生新的 cache key；先改写版本号的发布 job 不属于该前提。
- 缓存只承载可复用依赖产物，不为追求命中率启用 workspace crate 或 incremental artifact 缓存；缓存容量、失效粒度和可信边界优先于单次命中率。
- focused test 同时选择最小 Cargo target（如 `--lib` 或 `--test <target>`）和必要 feature；仅使用名称过滤不能阻止无关 test target 进入编译图。
- 只有具备独立 owner、平台矩阵或失败归因价值的验证才拆成 job。顺序执行但共享同一依赖图的命令优先留在既有 job 中，避免用新增 job 重复结账工具链、checkout 和缓存恢复成本。

## 8. 评审证据

依赖治理 PR 按改动选择证据，不机械执行全部命令：

```text
cargo tree -d
cargo tree -e features -p <product-or-owner>
cargo tree -e normal,build -p <product-or-owner>
cargo check -p <owner> --no-default-features --features <feature>
cargo test -p <owner> --test <target>
cargo check -p <product> --timings
```

前后对比优先列出真实产品 dependency closure、关键重依赖是否退出、冷/增量耗时环境与命令、最小 feature/test 是否可独立运行。测量结果是决策证据，不是永久硬阈值；如果收益不足以证明 owner 迁移或新抽象合理，应保留现有结构。

当前硬边界由 `scripts/check-core-boundaries.mjs` 统一执行。不要为同一 Cargo 架构事实增加第二个 checker；新增规则先证明当前树满足、fixture 能捕获回归，并保持错误消息可直接定位到 owner manifest。
检查器必须保持工作树只读；读取独立 manifest 的声明事实时不得生成新的 lockfile、target artifact 或格式化改动。
