# BitFun 编译与依赖治理计划

> 最近核实：2026-08-10
>
> 实现复核基线：`gcwing/main@734e5b05f`
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
| 服务测试链接拓扑已收敛 | Services 两个 crate 的集成 target 总数从 33 降到 25；选中的 `local-storage`、MCP、基础 SSH 闭包从 16 个集成 executable 降到 8 个 |
| Agent Runtime 基线不再隐藏重型 capability | `bitfun-core/agent-runtime` 只保留生命周期和基础工具 owner；文档转换与订阅认证也改为产品显式 modifier。在最新主线 A/B 中，三平台 normal/build 闭包进一步减少 69/64/110 个版本化 package instance |
| App Server 不继承未消费能力 | App Server 保持现有 Agent/Git/外部来源 handler 边界，不再因 Core 基线携带文档转换和本地订阅凭据，三平台闭包减少 61/56/78 |
| 完整产品行为和闭包保持 | `product-full` 显式组合全部 owner，Windows normal/build 闭包保持 570；CLI 保持 649。ACP 只退出未选择或未使用的隐含能力，累计在 Windows/macOS/Linux 分别减少 12/15/24 |
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

### 3.1 服务层测试拓扑

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

可重复确认的产物变化如下；`test executable` 包含每个 crate 的 lib test harness，因此比 integration
target 多 1。PDB 大小会随工具链变化，只比较同次 A/B：

| 闭包 | test executable | EXE | PDB |
|---|---:|---:|---:|
| local-storage | 13 → 6 | 25.2 → 19.2 MiB | 135.7 → 91.9 MiB |
| 基础 Remote SSH | 3 → 2 | 3.9 → 2.8 MiB | 53.5 → 43.8 MiB |

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

在最新实现复核基线 `gcwing/main@734e5b05f` 上，本轮继续把两个重型能力从 Core 基线改为弱
modifier。计数先移除 Cargo tree 的重复展示标记 `(*)`，再按 package/version 去重：

| 本轮闭包 | Windows | macOS | Linux | 行为边界 |
|---|---:|---:|---:|---|
| Core `agent-runtime` | 343 → 274 | 330 → 266 | 375 → 265 | 文档扩展识别保留；转换和本地订阅凭据明确不可用 |
| App Server | 490 → 429 | 477 → 421 | 508 → 430 | 现有 handler/DTO 保持，未消费的两个能力退出 |
| Core `product-full` | 570 → 570 | 557 → 557 | 601 → 601 | 显式恢复 `document-read` 与 `subscription-auth` |
| CLI | 649 → 649 | 649 → 649 | 672 → 672 | 显式保持原有能力 |
| ACP | 589 → 587 | 574 → 572 | 594 → 592 | 保持原有能力，同时退出 Reqwest 未使用的 `mime_guess`/`unicase` |

本轮没有新增 crate 或第三方 dependency。收益来自两类现有重闭包退出窄入口：`anydoc` 及其
文档解析/压缩依赖，以及订阅凭据的 keyring/加密/本地存储依赖。完整产品 package 集合不变，
因此这里只报告依赖图收敛，不宣称 `product-full` wall-clock 提速。

Package instance 会低估“同一个大 crate 少编译了多少 feature 代码”。在 Windows
`agent-runtime` 闭包中，`bitfun-services-integrations` 的 Cargo active feature 从 61 个降到 6 个，
只保留 `workspace-search` 及其 5 个直接依赖 feature；`bitfun-product-domains` 从 13 个降到 5 个，
只保留 Agent Runtime 实际使用的 external-subagent contract slice。Function Agent、MiniApp、
Plugin Source 由各自 owner 选择，完整产品仍经 `product-full` 显式恢复。

根 `Cargo.lock` 与实现复核基线保持一致，package 记录不增加；Installer 自己生成的
`BitFun-Installer/src-tauri/Cargo.lock` 本 PR 不提交。

| 状态 | 范围 | 处理结论 |
|---|---|---|
| 已稳定 | 根 `Cargo.lock`、Reqwest Rustls 单栈、workspace Tokio 最小基线 | 不重复治理 |
| 本轮完成 | Core Agent Runtime capability、文档转换与订阅认证 modifier、Installer 未使用直接依赖 | 以真实入口 closure 收敛，不建立新的产品 umbrella，也不扩大根依赖宇宙 |
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
| 重型可选能力 | 文档转换和本地订阅凭据由弱 modifier 细化已有 runtime owner；Core 基线和 App Server 退出未消费闭包 |
| Installer 闭包 | 删除 8 个未使用直接 dependency；独立 workspace 和发布生命周期不变，本 PR 不提交其生成 lockfile |
| Agent Runtime 测试 | 28 个 integration executable 已收敛为 5 个职责/平台 target |
| Services 测试 | 两个服务 crate 使用显式 target；选中闭包少 8 个 integration executable，进程/feature/external-system 边界保持独立 |

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
