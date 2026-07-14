# BitFun Core 拆解与运行时迁移计划

本文件只维护 Core 边界债务、迁移顺序和退出条件。稳定架构以
[产品运行时架构](../architecture/product-architecture.md)为准；Agent Runtime、产品定制和 OpenCode 扩展分别由
[运行时设计](../architecture/agent-runtime-services-design.md)、
[产品定制设计](../architecture/product-customization-blueprint.md)和
[OpenCode 扩展兼容设计](../architecture/extensions/opencode-extension-compatibility.md)负责；OpenCode 交付阶段与
退出条件见[扩展兼容计划](opencode-extension-compatibility-plan.md)。已完成事实归档在
[core-decomposition-completed.md](core-decomposition-completed.md)。

## 1. 执行原则

- 依赖方向固定为产品入口 / interfaces → assembly → adapters / services / execution → contracts。
- DTO 或端口抽取不等于运行时 owner 已迁移；只有生产入口切换、行为等价成立且旧写入方退出后才算完成。
- 每次只迁移一条真实纵向调用链，不按目录或类型数量拆 PR。
- 新接口必须有当前生产消费方、版本边界、验证方式和退场条件；空 profile、re-export、测试桩或未来矩阵不算消费方。
- 入口、Remote 和 SDK 的不支持状态必须类型化且可解释，不得静默回到 `product-full` 或本机执行。
- Core 拆解与生态兼容并行演进。任何一条路线不得为了等待另一条路线而预建通用接口。

## 2. 已核实基线

| 事实 | 当前状态 | 结论 |
|---|---|---|
| 产品能力组装 | `DeliveryProfile`、`ProductAssembler`、能力计划、服务可用性和测试已存在 | 这些是可测试的 assembly facts，不代表产品入口已接入 |
| CLI / Desktop / ACP | 三者仍启用 `bitfun-core/product-full`；CLI 已提交 `DeliveryProfile::Cli` 并消费 Runtime Parts/SDK，Desktop 与 ACP 尚未切换 | CLI 已建立产品组装边界但仍保留 Core owner；三个入口均未完成 owner 迁移 |
| Server | 当前生产路由只形成 health/info/ping 基线 | 没有插件状态或独立产品组装闭环 |
| Server / Remote / Web / Mobile Web / SDK profile | 当前为空计划、未接入入口或仅有 preview 测试 | 不得据枚举值宣称产品能力已交付 |
| Agent Runtime SDK | 已有无 `bitfun-core` 依赖的 v1 preview 门面和 smoke test | 发布边界仍需真实嵌入方证明 |
| 插件运行时 | 现有路径只覆盖 BitFun 原生包和 OpenCode custom tool 静态名称预览 | 不能据通用 envelope 或静态候选扩张稳定 ABI |
| Relay | `assembly/core` 直接依赖 `apps/relay-server` 以复用嵌入式 relay | 依赖方向反转，且当前边界检查未阻止该问题 |
| CLI CI | 独立 Linux job 运行 CLI test，通用三平台 workspace check 覆盖 CLI 编译；发布工作流负责打包 | 参数/序列化/前置失败和组装已有 focused contract；真实模型/PTY、Patch I/O 失败与常规 package smoke 仍需补齐 |

## 3. 目标依赖与归属

| 层 | 负责 | 禁止 |
|---|---|---|
| apps / interfaces | 选择唯一入口形态，提交 profile，投影协议或界面 | 成为共享运行时 owner，复制会话/工具/权限逻辑 |
| assembly | 选择能力、提供方和兼容门面，输出类型化 runtime parts | 依赖 app crate，持有平台进程/协议实现，重新解释动态配置 |
| adapters / services | 协议转换、平台 I/O、可复用具体实现 | 反向依赖 assembly 或产品入口 |
| execution | Agent、Tool、Harness、Plugin Host 的可移植执行语义 | 读取交付形态，依赖 app/adapter 具体实现 |
| contracts | 稳定 DTO、事实和端口 | 依赖上层或持有运行时行为 |

需要同时被独立应用和嵌入式模式复用的能力，先下沉为 services/adapters owner，再由 app 与 assembly 同向消费。
Relay 是该规则的首个修复对象；不能把 `apps/relay-server` 改名后继续作为下层库。

## 4. 迁移顺序

### 4.1 先修边界保护

1. 抽取 relay router、room 与 asset-store 的可复用 owner。
2. 让 standalone relay app 和嵌入式入口都依赖该 owner，删除 `assembly/core -> apps/relay-server`。
3. 为 crate 层级依赖增加通用边界检查和反向用例，避免只保护已知 crate 名称。

退出条件：生产行为与 standalone/embedded relay 测试等价，Cargo 图不再包含 assembly → apps。

### 4.2 切换 CLI 纵向路径

CLI 是首个入口迁移对象，因为它已有独立产品诉求、显式设计和最小 CI 命令。

当前纵向切片已经完成：入口只提交一次 `DeliveryProfile::Cli`，通过现有 `ProductAssembler` 获得计划、服务可用性、
Harness 和禁用的插件 binding；TUI、Exec、Session 与 Usage 共用一个 `CliRuntimeContext`。会话创建/列举/
删除、轮次提交和取消走 Agent Runtime SDK；SDK v1 缺口集中在一个 Core 兼容门面。Agentic Event Queue 仍是唯一
owner，各入口只建立独立广播订阅，有界兼容队列满载不再阻断广播。TUI 与 Exec 审批均为调用级策略，不写全局
配置；CLI 本地路径不获取具体 PersistenceManager。交互、执行和管理入口分别控制 Peer Host/MCP 生命周期，管理查询不启动
这两类外部服务。结构化输出复用现有 Agentic envelope；会话 ID 与
存储路径绑定并在删除前校验；TUI 终端恢复由 RAII guard 覆盖错误和 panic 展开路径。

Peer Host 的 Runtime 接入和跨 Relay/Desktop/Web 的协议切换保持独立；本切片不改变其 HostInvoke、身份、确认或
重连语义。

下一步按独立纵向切片推进：

1. 以真实调用方和行为等价测试补齐 SDK 端口，逐项缩小固定 ID、恢复视图、消息、分支、用量和工具确认兼容面。
2. 迁移 ACP 的会话/权限/事件投影，但保留 ACP stdio 生命周期在接口入口。
3. 继续拆分 TUI 副作用边界并补 package smoke，不以大规模重写替代现有回归保护。

当前 assembly 切换条件已经满足：CLI 生产入口消费真实组装结果，目标链路没有第二套状态，独立测试与三平台
编译门禁存在。CLI-P0 整体退出条件尚未满足；真实模型/PTY 协议矩阵、兼容门面退出、ACP/Desktop 切换和
package smoke 需分别验收。

### 4.3 依次切换 ACP 与 Desktop

- ACP 在 CLI 之后迁移，优先收敛协议投影和权限/会话桥接，不把 ACP 生命周期下沉到 Agent Runtime。
- Desktop 最后迁移，因为当前 `product-full` 覆盖最广；按服务簇逐步切换，保留 Tauri 与窗口行为在 app/adapter。
- 每个入口独立提交自己的 profile；禁止 assembly 根据调用栈、feature 或全局状态再次猜测交付形态。

退出条件与 CLI 相同：生产消费、行为等价、单一 owner、旧路径退出和入口级验证缺一不可。

### 4.4 最后晋级 Server、Remote 与 SDK

- Server 先从现有 health/info/ping 基线选择一个真实 API 消费方，不预建完整产品 surface。
- Remote 必须在实际工作区执行域完成能力协商，不以本地 provider 代替。
- SDK 只有在外部或仓库内独立嵌入方无需 `bitfun-core/product-full` 即可完成最小 session/turn/event 流程后，才从 preview 晋级。
- 空 capability plan、disabled stub 和单元测试用于保护降级，不构成产品完成证据。

## 5. 与插件兼容的交叉点

Core 只为插件兼容提供已有 owner 的窄接口：真实工具、类型化 Hook 变换、公开事件、权限请求和诊断。OpenCode
来源发现、执行准备与兼容语义由对应架构设计和适配器 owner 负责；计划只维护交付顺序与退出条件。

首个可执行切片应只闭环一种 standalone custom tool：真实来源 → worker → 原始校验 → Tool Runtime → 调用结果。
在该切片完成前：

- 不扩张 `PluginDispatchEnvelope` / `PluginEffectCandidate` 去承载 Hook、Client 或 TUI；
- 不为未来生态新增公共注册表或多用途 DTO；
- 不把静态名称、`ready` 或 adapter fixture 当作工具可调用；
- 不让 SDLC Harness 定义第二套插件接口。

## 6. 固定执行流程

1. 同步最新 `gcwing/main`，记录入口、依赖图和生产消费方。
2. 选择一个用户可见纵向切片，写清当前 owner、目标 owner、唯一写入方和删除条件。
3. 先补行为等价与边界失败用例，再切换生产调用方。
4. 删除或冻结被替代路径，复核 Remote、错误、取消和恢复语义。
5. 运行最小可信验证，再由独立审查者检查过度设计、旧路径残留和能力过度声明。
6. PR 明确当前能力、变更后的能力、未覆盖项、用户影响和回退方式。

## 7. 验证矩阵

| 范围 | 最小验证 |
|---|---|
| 文档与仓库边界 | `pnpm run check:repo-hygiene`，`node --test scripts/check-core-boundaries.test.mjs`，`node scripts/check-core-boundaries.mjs` |
| 入口 profile 迁移 | 对应 app 的 check/test、入口级 smoke、profile/服务可用性断言、旧路径等价用例 |
| Relay owner 迁移 | standalone 与 embedded focused tests、Cargo 依赖方向失败用例 |
| Agent Runtime / SDK | `cargo test -p bitfun-agent-runtime`，最小 no-`bitfun-core` 嵌入测试 |
| 插件首个执行切片 | runtime ports、Host、adapter、Tool Runtime 与真实冻结 fixture 的端到端调用 |
| CLI | `cargo check -p bitfun-cli`，`cargo test -p bitfun-cli`，结构化协议和 package smoke |

## 8. 暂停条件

出现以下任一情况时，不继续扩接口：

- 只有枚举、空计划、re-export、测试桩或未来矩阵，没有生产消费方；
- assembly 新增 app 依赖，或下层读取 profile/产品入口状态；
- 同一事实在兼容门面与目标 owner 中同时计算或写入；
- 泛 envelope、候选效果或描述符开始承载工具、Hook、Client、TUI 等不同语义；
- Remote 不支持时静默回本机，或 SDK 仍需要 `product-full` 却被描述为独立可用；
- 为迁移一次性重写全部 CLI、Desktop 或 Core，而没有可单独验收的纵向切片。
