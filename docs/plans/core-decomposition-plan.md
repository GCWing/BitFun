# BitFun Core 拆解与运行时迁移计划

本文件只维护 Core 边界和运行时迁移顺序。OpenCode 扩展能力的产品阶段由
[OpenCode 扩展兼容计划](opencode-extension-compatibility-plan.md)负责；产品定制由
[产品定制设计](../architecture/product-customization-blueprint.md)负责。已完成事实归档在
[core-decomposition-completed.md](core-decomposition-completed.md)。

## 1. 执行原则

- 产品逻辑保持平台无关，再通过 Desktop、CLI、Server、ACP 和生态适配器暴露。
- 工具、事件、权限、配置、会话和界面状态各自只有一个最终归属模块；插件层不复制这些数据模型。
- OpenCode 适配器只保存外部格式、顺序和错误语义，不成为第二套 Agent Runtime。
- 新接口必须有当前消费方、版本边界和验证方式；仅为未来完整性准备的接口不进入稳定面。
- 新路径交付时同步删除、迁移或明确冻结旧路径，不能长期保留两个写入方。
- 远程工作区从接口设计开始考虑；无法远程执行的能力明确显示不支持，不静默回本机。

## 2. 当前基线

当前已有：

- 工具接口、事件清单、权限路径、运行时服务和产品能力边界；
- 插件运行时主机的查询、派发、期限、取消、故障状态和重启清理；
- BitFun 原生插件目录的内容校验、来源确认、启停记录和 CLI 诊断；
- OpenCode custom tool 的静态名称预览。

当前没有：

- OpenCode JS/TS、软件包插件、standalone tools、稳定 Hook 或 TUI target 的真实执行；
- OpenCode 主/TUI 配置的完整来源和字段兼容；
- 全局插件自动加载、依赖准备、更新回退和真实 Client/Server 门面；
- Remote 插件执行和外部产品入口兼容。

因此现有状态只能称为“来源可识别、名称可预览”，不能显示为“OpenCode 插件可运行”。

## 3. Core 与扩展的职责边界

| 部分 | 负责 | 不负责 |
|---|---|---|
| 工具归属模块 | 可调用工具集合、schema、权限、取消和结果 | 加载 JS/TS 或解释 OpenCode Zod |
| 配置归属模块 | BitFun 最终配置、来源说明和写入 | 猜测未知 OpenCode 字段 |
| 权限归属模块 | 用户/组织策略和最终授权结果 | 阻止所有脚本直接副作用的虚假承诺 |
| 插件运行时主机 | 类型化调用、期限、取消、有界队列、进程健康和诊断 | OpenCode 加载顺序、业务状态或界面渲染 |
| OpenCode 适配器 | 配置/插件/工具/Hook/TUI 的加载与参数转换 | 产品入口、第二会话模型或通用 UI 协议 |
| 产品组装 | 选择当前交付形态包含的能力、服务和内置扩展 | 解释用户插件或动态健康状态 |

插件真实贡献只有经过对应归属模块校验后才可见。静态名称或声明不能进入可调用工具集合；Hook 的合法变换也
不能被统一降成只读通知。

## 4. 迁移顺序

Core 迁移跟随 OpenCode 兼容阶段，但不复制其产品计划：

| 兼容阶段 | Core 需要先具备的边界 |
|---|---|
| OC-R0 差异可见 | 诊断、能力状态和版本信息能区分当前实现、目标与降级 |
| OC-R1 配置与更新 | 配置来源说明、字段级错误、原子发布结果和上个可用版本状态 |
| OC-R2 本地执行 | 工具真实注册、类型化插件调用、取消、迟到响应丢弃和附件结果 |
| OC-R3 稳定服务面 | Hook 顺序、每步校验、Client 路由和最终状态归属 |
| OC-R4 TUI/外部入口 | CLI/TUI 宿主操作、可退出降级界面和入口能力状态 |
| OC-R5 Remote/策略 | 实际执行域、能力协商和策略差异诊断 |

产品内置扩展只依赖产品组装结果可以固定 `id/version/hash`，以及 OC-R2 已有真实执行路径。它不依赖用户插件
安装功能，也不共享用户插件来源、启用记录、更新通道或卸载操作。

## 5. 当前拆解重点

| 优先级 | 问题 | 收敛方向 |
|---|---|---|
| P0 | `bitfun-core/product-full` 仍是部分入口的大门面 | 新入口依赖稳定能力服务；只迁移真实调用链并保留行为等价测试 |
| P0 | 插件接口可能随矩阵膨胀 | 先复用工具、事件、权限和配置接口；OpenCode 专用字段留在适配器内部 |
| P0 | 静态工具预览与真实执行可能混淆 | 产品状态分开显示；未加载真实执行函数的工具不可调用 |
| P1 | 产品能力与工具提供方存在重复描述 | 由产品组装选择提供方，由工具模块维护实际可调用集合 |
| P1 | 入口可能直接读取插件主机内部状态 | 统一通过能力服务的插件状态和诊断视图 |
| P1 | 配置、插件和依赖准备可能阻塞启动 | 后台准备、字段级错误、独立期限和上个可用结果 |
| P2 | 接口处理器仍包含具体 IO | 按真实迁移收益下沉到 services/adapters，不做纯目录搬迁 |

## 6. 固定执行流程

1. 同步最新 `gcwing/main`，确认当前实现与文档基线。
2. 沿产品入口 → 能力服务 → 归属模块 → 适配器追踪真实调用链。
3. 先补边界与行为测试，再迁移实现；没有消费方时不新增接口。
4. 删除或冻结被替代路径，检查 Remote 和所有产品入口。
5. 运行最小可信验证，再做独立对抗性审查。
6. PR 说明当前能力、目标能力、未覆盖项、用户可见变化和回退方式。

## 7. 验证矩阵

| 范围 | 最小验证 |
|---|---|
| 文档、目录和仓库边界 | `pnpm run check:repo-hygiene`，`node --test scripts/check-core-boundaries.test.mjs`，`node scripts/check-core-boundaries.mjs` |
| 插件运行时主机 | `cargo test -p bitfun-runtime-ports --test plugin_runtime_contracts`，`cargo test -p bitfun-runtime-ports --test plugin_runtime_host_contracts`，`cargo test -p bitfun-plugin-runtime-host` |
| 原生插件来源基线 | `cargo test -p bitfun-product-domains --test plugin_source_contracts --features plugin-source`，`cargo test -p bitfun-services-integrations --no-default-features --features plugin-source plugin_source --lib` |
| OpenCode 适配器 | `cargo test -p bitfun-opencode-adapter --test opencode_source_adapter`；真实执行阶段再增加冻结 OpenCode 样例和端到端调用 |
| CLI 状态与诊断 | `cargo test -p bitfun-cli --test plugin_source_cli`，并验证“静态预览”不会显示成“可执行” |

具体 OpenCode 配置、插件、Hook、TUI 与更新验收以兼容计划各阶段退出条件为准。

## 8. 暂停条件

出现以下任一情况时，不继续扩接口：

- 新增插件、Hook、事件、界面或主机公共接口，但没有真实消费方和验证样例；
- OpenCode 原始类型进入 BitFun 业务状态或前后端公共接口；
- 插件运行时主机直接写权限、审计、会话、工具结果或界面状态；
- 产品入口直接消费插件进程句柄、主机内部状态或生态原始载荷；
- 远程不支持时静默改为本地执行；
- 为兼容单个插件复制完整 OpenCode Server、Agent Runtime 或 OpenTUI 组件树。
