# BitFun 编译与依赖治理计划

> 最近核实：2026-08-04
>
> 快照基线：`gcwing/main@061024fb2` 加权限规划 owner 迁移
>
> 稳定规则：[Rust 构建与依赖边界](../architecture/rust-build-dependency-boundaries.md)

这份文档只回答三个问题：当前主要成本在哪里、下一步先做什么、每轮治理如何证明有效。
模块边界以架构文档为准，具体本地命令由最近的 `AGENTS.md` 维护，PR 只记录实际运行过的验证。

## 1. 当前结论

| 结论 | 说明 |
|---|---|
| 本轮收益是测试隔离，不是产品构建瘦身 | 权限纯策略测试从 Core 约 449 节点的闭包迁到 Agent Runtime 约 78 节点的闭包；产品依赖图不变 |
| 不再用 `product-full` 解决 focused test | Core 权限编排测试当前最小闭包是 `agent-runtime,canvas-runtime`；纯策略直接在 Agent Runtime 验证 |
| 不新增 CI 或测试入口 | 继续使用现有 test target 和 CI job；治理 PR 不复制同一闭包的验证 |
| 下一优先级是 App Server / Server | 先核实真实生产调用链，再收敛其 Core `product-full` 边界；收益不足则停止 |
| 依赖多版本不能按数量批量清理 | 只处理仓库能控制、行为等价且能缩小真实构建图的版本路径 |

权限 owner 的长期边界和功能不变量见
[Agent Runtime 服务设计](../architecture/agent-runtime-services-design.md)。这里不重复维护行为规格。

## 2. 治理原则

目标是缩短常用开发、focused test、CI 和打包路径，同时保持产品行为与分层边界稳定。
每个治理 PR 必须同时满足以下门槛：

| 门槛 | 必须回答的问题 |
|---|---|
| Owner | 逻辑属于哪个现有 owner？是否存在真实生产消费者？ |
| 行为 | 本地、远程和平台差异如何保持？哪些等价测试保护它？ |
| 构建图 | 哪个产品或测试闭包实际退出了哪些依赖？ |
| 耗时 | 若宣称性能收益，是否在同机器、同命令、同缓存状态下测量？ |
| 增量成本 | 是否新增 dependency、feature、test target、CI job 或长期兼容层？ |

以下做法不属于优化：

- 用 `product-full`、`all-features` 或 workspace 全量测试掩盖 feature 边界；
- 为减少重复版本数字强制 patch 平台依赖、宏生态或第三方兼容窗口；
- 新建第二套 Agent、Tool、Permission Runtime 或无消费者抽象；
- 未测量就引入 sccache、替换链接器、合并 Installer workspace 或增加 CI job；
- 删除跨平台行为保护来换取表面 CI 时长。

## 3. 当前基线

### 3.1 Rust 构建图

| 路径 | 当前快照 | 判断 |
|---|---:|---|
| `bitfun-core` | 约 493 个 Rust 文件、243,900 行 | 仍是最大的高频失效面；只按真实 owner 做纵向迁移 |
| Core 直接消费者 | ACP、App Server、CLI、Desktop、SDK Host、Server | 每次只迁移一个有真实调用方的服务切片 |
| Agent Runtime focused test | 约 78 个唯一 package/version 节点 | 适合无 IO 的 Agent Runtime 纯决策测试 |
| Core `agent-runtime` check | 约 391 个节点 | 窄 owner feature 可独立编译 |
| Core 权限编排测试 | `agent-runtime,canvas-runtime`，约 449 个节点 | 保留真实 scope、Hook、请求生命周期和 Tool 执行 |
| Core `product-full` test | 约 516 个节点 | 仅用于确实需要完整产品装配的兼容路径 |
| Agent Runtime integration target | 5 个显式 target | 已完成收敛；平台和进程边界继续独立 |

节点数来自同一 Windows 环境下的 `cargo tree --locked` 相对统计，不是实际耗时，也不是跨平台阈值。
权限纯策略路径理论上少进入约 371 个节点；产品构建闭包没有变化。

### 3.2 依赖与 feature

| 状态 | 范围 | 处理结论 |
|---|---|---|
| 已稳定 | 根 `Cargo.lock`、Reqwest Rustls 单栈、Desktop 直接 `image 0.25`、workspace Tokio 最小基线 | 不重复治理 |
| 下一步核实 | App Server / Server 的 Core `product-full` | 按生产 construction path 收敛，不先写 feature 清单 |
| 可独立治理 | Installer 的 Reqwest 0.12、独立 lockfile、疑似无消费者的 `tokio/full` | 保持 Installer 独立 workspace，不顺手合并 |
| 等待上游 | `screenshots 0.8.10 -> image 0.24.9` | 只有受维护且行为等价的上游替代出现后再处理 |
| 明确保留 | `portable-pty 0.8/0.9` | 非 OHOS 与 OHOS 的平台兼容选择，不为去重破坏 |

根 lockfile 约有 116 个名称存在多版本。这个数字只用于发现候选，不能直接转化为治理任务。
`oxc`、`rquickjs`、vendored `git2`、`sherpa-onnx` 等重依赖都有真实 capability owner；只有某个产品入口
不消费对应能力时，才允许让它退出该入口的构建图。

### 3.3 CI 与本地验证

- 现有 CI 已覆盖 workspace check、Core/Desktop lib、平台敏感 owner 测试和独立 runtime/CLI 验证；
  不再为治理 PR 叠加同闭包 job。
- 本地先运行 owner 文档维护的最小 package/target/feature 命令。广泛 build、workspace suite、打包和
  平台矩阵由 CI 承担，除非改动直接影响这些路径或需要复现 CI 故障。
- CI 收敛必须基于多次 job/step 耗时、缓存状态、平台事实和失败历史。测试名称相似不等于覆盖重复，
  `SKIPPED`、未触发或只编译未运行也不等于通过。

## 4. 已完成，不再重复实施

| 主题 | 当前结果 |
|---|---|
| 前端构建 | Monaco 运行时加载已统一；Web type-check/Vite 并行；Web/Mobile TS 已启用 incremental |
| 开发循环 | mobile-web 支持输入 mtime 短路；Vite 默认使用原生文件事件；前端准备步骤已并行 |
| Rust profile | release 使用 thin LTO；dev 使用 `line-tables-only` 和高 codegen-units，并保留调试逃生口 |
| 可复现解析 | 根 lockfile 已提交，普通 CI 使用 `--locked`；build.rs 输出已排序 |
| CI 拓扑 | Rust job 不再等待完整前端构建，自建 Tauri 检查所需资源目录 |
| 依赖收敛 | Desktop 直接 image 版本和 Reqwest TLS 双栈已治理 |
| Agent Runtime 测试 | 28 个 integration executable 已收敛为 5 个职责/平台 target |

内置 Agent 内容已经移到无第三方依赖的 `bitfun-agent-content`，减少了 Core build-script 工作；但 Core
仍直接依赖该 crate。没有足够产品收益前，不为消除这一编译指纹引入动态 provider、运行时文件读取或资源协议。

## 5. 后续顺序

### R1：收敛 App Server / Server 的 `product-full` 边界

这是下一 PR 的推荐范围，也是唯一需要优先设计的核心入口改造。

先回答：

- App Server 与 Server 的真实 construction、command 和 schema 路径分别消费哪些 Core owner？
- Server 对未实现能力应保持什么 typed unsupported 行为？
- 哪些能力由 Server 直接消费，哪些只是经 App Server 间接带入？

实现边界：

- 只替换一个端到端 capability slice 的依赖路径，不一次迁移全部 Core 调用；
- 优先显式选择已有 owner feature，或消费现有 Runtime SDK/service port；
- 不复制 Session、Tool、Permission、Hook、Event 状态，不建立第二 Runtime；
- 未迁移能力保留现有兼容路径或明确 unsupported，禁止静默本机回退。

验收必须覆盖 Server WebSocket/App Server round-trip、权限、取消、事件与恢复语义，并对比 App Server、
Server 的 normal/build/test closure。若构建图收益不足或行为等价无法证明，则不删除兼容边界。

### 后续队列

| 顺序 | 范围 | 启动条件 |
|---|---|---|
| R2 | 从 ACP 迁移一个已有 Services owner 的 host-service 切片 | 明确真实调用方，并能保持 Windows 进程树、SSH、取消和远程身份语义 |
| R3 | Installer lockfile、Reqwest 0.13 与无消费者依赖治理 | 下载、SSE/进度、取消、代理、证书失败和三平台 packaging 可验证 |
| R4 | 消除 `screenshots -> image 0.24` | 有受维护、无需 fork/vendoring 且屏幕枚举/DPI/权限行为等价的上游路径 |

每一步都在前一 PR 合入后的最新 main 重新测量。无法证明边界或收益时停止，不为了完成清单继续重构。

## 6. 每轮 PR 的证据

PR 描述只需维护一张简表，不新增全仓依赖台账：

| 证据 | 变更前 | 变更后 |
|---|---:|---:|
| 真实产品 normal/build closure |  |  |
| owner focused-test closure |  |  |
| 目标重复版本或重型依赖路径 |  |  |
| 冷、热或增量耗时（同机器、命令、缓存状态） |  |  |
| 新增 dependency、feature、test target、CI job |  |  |

同时记录功能不变量、远程/平台差异、实际运行的最小验证和未运行的 CI。若产品 closure 不变，只能说明
focused-test 或 owner 边界收益，不能宣称产品构建已经变快。
