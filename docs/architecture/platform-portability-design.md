# 平台适配与 HarmonyOS 本地运行可行性

本文说明 BitFun 跨平台代码的边界，并定义 HarmonyOS 本地 CLI/TUI 候选形态需要先证明什么。稳定的产品和运行时
边界以[产品运行时架构](product-architecture.md)为准；实施顺序见
[产品架构演进计划](../plans/product-architecture-evolution-plan.md)。

本文不声明 HarmonyOS 本地产品已经可用，也不把现有 `src/apps/mobile/harmonyos` ArkTS 远程端改写为本地
Runtime。Rust target 可编译、`hdc shell` 能运行二进制和 HAP 内能完成本地编码工作流是三种不同证据，不能互相
替代。

## 1. 已确认的平台事实

- Rust 当前把 `aarch64-unknown-linux-ohos`、`armv7-unknown-linux-ohos` 和
  `x86_64-unknown-linux-ohos` 列为 Tier 2 with host tools，把 `loongarch64-unknown-linux-ohos` 列为 Tier 3。
- OHOS target 需要 OpenHarmony SDK、Clang、sysroot、`llvm-ar` 和显式 linker 配置；SDK 不会自动完成 Rust 集成。
- OHOS 的 Rust 条件是 `target_os = "linux"`、`target_env = "ohos"`、`target_family = "unix"`。只判断
  `target_os = "linux"` 或 `cfg(unix)` 的依赖可能错误启用桌面 Linux 实现。
- HAP 是 OpenHarmony 应用的安装与运行单元；NDK 用于在应用内实现或复用关键原生能力，不等于独立 native
  CLI 的通用发行方式。
- 官方资料没有保证 HAP 内存在可供 crossterm 直接使用的真实 TTY，也没有保证 PTY、交互 shell、后台进程、
  动态库、文件范围和网络行为与桌面 Linux 相同。这些都必须通过目标系统版本和真机样例验证。

参考：

- [Rust OpenHarmony platform support](https://doc.rust-lang.org/stable/rustc/platform-support/openharmony.html)
- [OpenHarmony NDK overview](https://gitee.com/openharmony/docs/blob/master/en/application-dev/napi/ndk-development-overview.md)
- [OpenHarmony HAP package](https://gitee.com/openharmony/docs/blob/master/en/application-dev/quick-start/hap-package.md)
- [OpenHarmony hdc guide](https://gitee.com/openharmony/docs/blob/master/zh-cn/device-dev/subsystems/subsys-toolchain-hdc-guide.md)

首个验证目标只选择 `aarch64-unknown-linux-ohos`。没有真实设备或发行需求时，不增加其他架构承诺。

## 2. 仓库边界

平台不是新的仓库层，也不需要一个包含所有 OS 能力的 `PlatformManager`。

| 位置 | 负责 | 不负责 |
|---|---|---|
| app 入口 | 构造本入口需要的 OS 资源和具体能力实现；管理 HAP/窗口/终端生命周期 | 会话、工具、权限等共享业务状态 |
| `assembly` | 校验入口提供的能力并选择产品组合 | 依赖 app crate、构造 app-local OS 资源、持有平台句柄 |
| `adapters` / `services` | 外部格式转换和 FS、Process、Terminal、Git、Network 等具体 I/O | 产品身份、界面状态、Agent Runtime 业务语义 |
| `execution` | 消费已有窄端口，保持工具和工作流语义平台无关 | 探测平台、选择交付形态、硬编码 shell 或路径 |
| `contracts` | DTO、不可变事实和已有端口 | 环境探测、进程启动、目录查找或具体平台实现 |

规则：

1. target triple 只选择 ABI 或 target-specific dependency，不表达产品能力。
2. Cargo feature 只控制确实可选的依赖或能力，不使用 `no-*`、单一 `ohos` 大 feature 或共享 crate 中互斥的
   平台实现表达产品组合。
3. 具体能力实现可以并存，由 app 入口选择并注入；`assembly` 不反向依赖 app。
4. 新增端口前先检查现有端口能否表达真实调用；没有当前调用方时不提前抽象。
5. 平台不可用必须返回明确原因，不能静默回退 Desktop、Remote 或开发机执行。

## 3. 先复用现有端口

| 能力 | 当前事实 | 本轮裁决 |
|---|---|---|
| TUI input/resize/restore | CLI 直接使用 crossterm；`TerminalPort` 不表达 TUI 设备 | 在第二个真实宿主出现前保持 app-local；可行性样例确认共同语义后再决定是否抽取宿主内部接口 |
| command/process/PTY | 已有 `TerminalPort` 和 `WorkspaceShell` | 先复用并验证缺口，不并行新增笼统 `ProcessPort` |
| Git | `GitPort` 当前只是能力标记，仓库仍有领域 Git 调用 | 不能把 marker 写成可用实现；出现 HarmonyOS Git 调用方后再设计具体实现和行为测试 |
| system paths | 路径逻辑仍有分散和上层探测 | 优先由入口注入不可变路径事实，不先建可变路径服务 |
| Network/TLS | 已有网络端口和 AI HTTP 客户端 | DNS、proxy、socket、证书和流式请求留在具体网络实现，不单独提升 TLS 接口 |
| file watch / clipboard | 目前是具体 service/app 能力 | 保持局部；缺失时报告原因，不为矩阵完整性抽象 |
| session store | 已有 `SessionStorePort` | 复用既有端口并为目标存储实现补真机测试 |
| OpenCode 脚本 | 当前没有 JS/TS 执行实现 | 属于插件执行设计，不升级为通用平台接口；只有首个真实 tool 调用需要时才增加窄内部端口 |

## 4. 当前代码事实与风险

基线为上游 `5e48999f94daf119c1217ed6b71ba878d564f5dd`。下表只记录已经从 manifest 或代码确认的事实；
目标依赖解析结果仍需由 target-specific `cargo tree` 证明。

| 已确认事实 | 风险或影响 | 最小处理 |
|---|---|---|
| CLI 直接启用 `bitfun-core/product-full` | 不能据此形成可裁剪的 HarmonyOS 产物 | 先建立目标依赖清单，再按真实阻塞项拆分 |
| `assembly/core` 依赖 `apps/relay-server` | 编译依赖方向反转 | 抽取可复用 relay owner，保持 standalone/embedded 行为等价 |
| `assembly/core` 直接包含 bundled `rusqlite` | 交叉编译包含原生 C 风险 | 在本地会话样例需要时验证或替换存储实现 |
| 非 Windows `git2` 使用 vendored OpenSSL | OHOS 原生构建和动态链接风险 | Git 只在编码就绪阶段取证，不阻塞最小界面 |
| CLI 无条件依赖 `arboard` | 可能带入桌面 Linux/X11 依赖 | 剪贴板改为可选能力，不阻塞核心路径 |
| CLI 使用 `syntect` / `syntect-tui` | 可能带入 Oniguruma 原生依赖 | 高亮作为可选能力，先验证纯 Rust 或禁用方案 |
| `notify`、crossterm、portable-pty 使用 Unix/Linux 路径 | inotify、`/dev/tty`、termios、openpty、signal 和 shell 假设未被 HAP 证明 | 分别做真机样例，不能由 cross-check 推断可用 |
| contracts/Product Domain 仍有少量环境/进程探测；Agent Runtime 的现有 OpenCode 路径 helper 只服务多生态 Skill 根 | 上层代码可能按平台分叉；把 Skill helper 误作 custom tool resolver 会迁错 owner | 环境/进程探测迁到 service；Skill helper 留在 Skill owner；OpenCode adapter 另建 `.opencode/tools/` resolver，不新增平台总管 |

这些问题不要求在一个 PR 中全部替换。只处理进入目标最小产物的依赖，或已经违反仓库依赖方向的项。

## 5. HarmonyOS 可行性与交付阶梯

| 阶段 | 要回答的问题 | 退出证据 |
|---|---|---|
| 可行性裁决 | HAP 中采用什么 Rust artifact/native bridge；怎样输入、绘制和恢复终端式界面；工作区、进程、shell/PTY、网络和存储是否可达 | 一个可安装 HAP 样例、真机记录和 ADR；明确可行方案、否决方案、支持的系统/SDK 版本及 go/no-go 结论 |
| 最小宿主 | 固定工具链和构建入口，HAP 可安装、启动、恢复和退出 | 可复现构建；输入、CJK/Unicode、paste、resize、suspend/resume、异常退出无锁死或残留状态 |
| 本地核心预览 | 本地模型 turn、read/edit、one-shot shell、取消和会话恢复能否工作 | DNS/TLS、路径、存储、进程取消和重启恢复的真机证据；不回退远端执行 |
| 本地编码就绪 | Git status/diff、stdio MCP、后台结果、watch、交互 PTY 或等价能力是否满足常用编码工作流 | 每个必需项有设备契约和端到端测试；缺一项则保持“预览”状态 |
| OpenCode 资格 | 与 Desktop 相同的无外部依赖契约样例能否在目标平台执行 | 单独通过，或明确显示不支持/能力受限；不影响本地编码核心的判定 |

可行性裁决通过前，只允许工具链、依赖和 HAP 样例工作，不开始大规模端口拆分。如果 HAP 生命周期、文件范围或
进程模型无法承载本地核心，结论应是暂停该产品形态或缩小目标，而不是增加远端回退后继续称为本地运行。

`hdc shell` 可以辅助验证 linker、动态库和系统命令，但不能替代 HAP 的安装、生命周期、输入/绘制、权限与存储
证据。现有 ArkTS Remote App 继续作为独立远程产品入口，也不能替代本地证据。

## 6. 依赖准入和验证

进入 HarmonyOS 必需依赖清单的新增或升级依赖必须记录：

1. 真实调用方和所属阶段；
2. `cargo tree --target aarch64-unknown-linux-ohos` 的依赖路径；
3. build script、`links`、C/C++/汇编、动态库、外部命令和 OS API；
4. cross-check 结果、HAP 真机结果和失败原因；
5. 不可用时是关闭可选能力、复用现有实现还是替换依赖；
6. 许可证、产物体积和维护成本。

最低验证：

| 范围 | 验证 |
|---|---|
| 仓库边界 | 通用 Cargo DAG 检查覆盖 workspace/独立 manifest、normal/build/dev/optional/target dependency |
| 目标依赖 | portable、最小宿主、本地核心、本地编码和 OpenCode 样例分别保存 `cargo tree` 与原生 `links` 快照 |
| 编译 | target-specific `cargo check`；明确区分“解析成功、编译成功、产品可用” |
| 设备 | HAP 安装/启动/恢复、输入/绘制、路径/权限、DNS/TLS、存储、进程终止和动态库 smoke |
| 降级 | 缺失能力有稳定原因；不会启动 Desktop/Remote 代执行，也不会把可选项误报为必需项 |

## 7. 本轮不展开的议题

- 新权限语言、应用沙箱、trusted workspace、凭据和组织策略；
- 商店签名、OEM 预装和非 aarch64 产品；
- Remote plugin execution；
- OpenCode 原始 OpenTUI renderer 或完整 Node/Bun 兼容层。

这些事项可以后续单独设计，但不能被写成当前安全保证或 HarmonyOS 已完成能力。
