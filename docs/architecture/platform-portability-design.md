# HarmonyOS PC 原生 CLI/TUI 平台规约

本文只定义 BitFun 面向 HarmonyOS PC 的产品边界、已知问题、后续工作域、风险与证据口径。稳定产品和运行时边界以
[产品运行时架构](product-architecture.md)为准，CLI 产品语义以
[CLI 产品线设计](cli-product-line-design.md)为准。

本文不是技术方案或实施计划，不预先决定具体文件、provider、依赖补丁、打包方式、阶段编号或 PR 拆分。任何
鸿蒙化工作启动前，都必须独立建立专题，补充该问题所需的平台证据、技术设计、验证方式和实施计划。

## 1. 产品裁决

HarmonyOS PC 是 BitFun CLI/TUI 的目标平台之一，TUI 优先于 HarmonyOS PC GUI。目标产品是用户在
HarmonyOS PC 真实系统终端中安装并本地执行的原生 `bitfun`（`bitfun-cli` 仅为废弃兼容入口）：

- TUI 直接使用真实 TTY；
- Agent Runtime、模型访问、工作区文件和命令执行均在该 PC 本机运行；
- 安装、升级和日常使用不依赖开发机、Desktop 主机或 Remote 代执行。

以下形态不能作为 HarmonyOS PC 原生 CLI/TUI 的完成证据：

- 在 HAP 内绘制终端风格界面；
- 只通过 `hdc shell` 推送、启动或操作二进制；
- 把现有 `src/apps/mobile/harmonyos` 手机 Remote App 改称为 PC 本地产品；
- 在其他设备运行 Runtime，再由 HarmonyOS PC 远程控制。

完整的 HarmonyOS PC 支持同时包含本地 CLI/TUI 与 GUI。GUI 是另一项必需交付形态，但由独立产品专题设计和
验收，不在本规约中借用 TUI 路线提前决定技术实现。现有 HarmonyOS 手机 Remote App 保持当前能力和演进路径，
本规约不修改其产品定位、架构或发布节奏。

## 2. 对旧设计的闭环

此前文档曾把“可安装 HAP 样例、native bridge、HAP 内输入/绘制”作为 HarmonyOS 本地候选的首要可行性路线。
该设计混淆了三种不同产品：

1. HarmonyOS PC 系统终端中的原生 CLI/TUI；
2. HAP/ArkUI/ArkWeb 应用；
3. HarmonyOS 手机 Remote App。

该旧路线现已正式退役：

- 不再以 HAP 样例证明 PC 原生 CLI/TUI；
- 不再提前为 CLI 抽象 HAP 生命周期、native bridge 或移动端接口；
- 不把 `hdc shell` 的开发调试能力写成用户终端能力；
- 不从手机 Remote App 推导 PC 本地 Agent、TTY、工作区或进程能力；
- 不保留旧路线的兼容对象、阶段或实施任务。

后续启动 HarmonyOS PC GUI 时，以及评估移动端本地适配时，必须分别建立独立专题，不能恢复旧路线后继续沿用
“本地 CLI/TUI”名称。

## 3. 已确认事实与未知前提

已确认：

- Rust 提供 `aarch64-unknown-linux-ohos` 等 OHOS target；OHOS 同时命中
  `target_os = "linux"`、`target_env = "ohos"` 和 `target_family = "unix"`。
- OHOS Rust 构建需要 OpenHarmony SDK、Clang、sysroot、linker 等外部工具链配置。
- HarmonyOS/OpenHarmony 的应用与 NDK 资料不能直接证明普通用户可把第三方 native CLI 安装到 PC 系统终端。
- 当前 Tauri 官方支持平台不包含 HarmonyOS，因此现有 Desktop GUI 技术路线不能直接推导到 HarmonyOS PC。

仍未知：

- 面向目标用户的系统终端、第三方命令安装、升级和卸载渠道；
- 真实 TTY 的输入、绘制、信号和恢复语义；
- 用户工作区、文件权限、子进程、PTY、网络、凭据和持久化能力；
- 各系统版本、设备、发行渠道和终端实现之间的兼容范围。

参考：

- [Rust OpenHarmony platform support](https://doc.rust-lang.org/stable/rustc/platform-support/openharmony.html)
- [HarmonyOS PC application development](https://developer.huawei.com/consumer/cn/multidevice/pc/get-started/)
- [OpenHarmony NDK overview](https://gitee.com/openharmony/docs/blob/master/en/application-dev/napi/ndk-development-overview.md)
- [OpenHarmony hdc guide](https://gitee.com/openharmony/docs/blob/master/zh-cn/device-dev/subsystems/subsys-toolchain-hdc-guide.md)
- [Tauri supported platforms](https://v2.tauri.app/)

## 4. 仓库规约

1. HarmonyOS PC 原生 TUI 继续使用 `DeliveryProfile::Cli`，不新增
   `DeliveryProfile::HarmonyOS`。
2. 不建立包含全部鸿蒙差异的巨型 `ohos` feature、总接口或第二套 Agent/Tool/Session Runtime。
3. 平台差异只应出现在真实存在差异的 app、adapter 或 service 边界；共享 Runtime 不按 target triple 分叉业务语义。
4. 任何进入 OHOS 闭包的 `cfg(unix)`、`cfg(not(windows))` 和
   `target_os = "linux"` 路径都必须重新取证，不能默认等同桌面 Linux。
5. 缺失能力必须显式报告 unavailable/unsupported，不得静默调用 Desktop、Remote、移动端或开发机代执行。
6. 路线不清晰的库或平台能力可以暂置，但必须记录阻塞事实、已知风险、恢复条件和受影响的产品能力。
7. 本规约列出的依赖和工作域只表示需要后续专题审计，不预设最终采用上游补丁、替换库、平台实现、能力裁剪或
   其他技术路线。

## 5. 当前问题清单

本清单基于上游 `ecad4f843`（2026-07-16）与
Cargo package `bitfun-cli` 的 `aarch64-unknown-linux-ohos` 目标依赖解析。依赖解析成功只表示问题已经可见，不表示可以编译、
运行或交付。

| 问题域 | 当前识别结果 | 主要风险 | 后续专题需要回答 |
|---|---|---|---|
| 产品依赖闭包 | CLI 仍通过 `bitfun-core/product-full` 拉入 remote、browser、canvas、plugin、watch、Git、SQLite、PTY 等能力 | 无关平台依赖阻塞构建；为过编译而破坏共享 owner | CLI 真正需要哪些能力，哪些依赖应保留、隔离或移出目标闭包 |
| Rust 与依赖解析 | 仓库无根 `Cargo.lock`；Rust 1.94.1 探针先被要求 Rust 1.95 的 `oxc-browserslist`、`oxc_sourcemap` 阻塞 | 把通用 MSRV/解析问题误判为 OHOS 问题；构建不可复现 | 仓库认可的 Rust、依赖解析和构建基线 |
| TUI/TTY | `ratatui/crossterm` 依赖 `mio`、rustix、signal-hook 和终端系统调用 | 能编译但 raw mode、输入、resize、信号或恢复不可用 | 真实系统终端支持范围与 TUI 退化边界 |
| 剪贴板与语法高亮 | `arboard -> x11rb` 带入 X11；`syntect-tui` 重新带入 `onig_sys` | 桌面 Linux/C 原生依赖进入 OHOS 产物 | 这些能力是否必需，以及各自可维护的鸿蒙化路线 |
| 进程与交互终端 | `portable-pty -> termios` 依赖 openpty、shell、信号、进程组和 `/dev` 语义 | 交互 shell、取消和子进程回收不成立 | OHOS 公开进程/PTY 能力与产品可接受的能力范围 |
| 文件监听 | `notify` 在目标解析中选择 `inotify` | `target_os=linux` 导致错误后端选择；替代实现可能增加延迟和资源消耗 | 真机 watch 语义、性能预算和不可用时的产品状态 |
| Git 与原生库 | `git2 -> libgit2-sys -> openssl-sys` 带入 CMake、zlib、OpenSSL 等原生构建 | 交叉编译、证书、凭据和行为一致性风险 | Git 能力的支持范围和可维护实现路径 |
| 网络与 TLS | 当前闭包同时存在 native-tls、rustls、OpenSSL、aws-lc/ring 等路径 | 产物膨胀、证书来源错误、代理或流式响应异常 | OHOS 网络、根证书、代理、流式响应和取消能力 |
| 存储与路径 | `rusqlite/libsqlite3-sys`、`dirs` 及多处路径探测依赖桌面/类 Unix 假设 | 会话损坏、凭据泄露、升级后路径漂移 | 配置、数据、缓存、日志、凭据与工作区边界 |
| Tokio 与平台条件 | `tokio(full)` 打开 process、signal、net、fs；源码存在大量 `cfg(unix)`、`cfg(not(windows))` | 未使用能力扩大闭包；OHOS 错走 Linux 分支 | 实际可达能力、系统调用兼容性和取消/事件语义 |
| 用户发行与支持 | 普通用户 native CLI 安装、系统终端入口和升级渠道尚未证明 | 开发探针被包装成产品；支持范围无法维护 | 支持的设备、系统、终端、渠道与生命周期 |

纯 Rust 或无 OS I/O 的依赖也不能预先标记为已支持；仍需由后续专题的实际目标构建验证。

## 6. 后续独立专题

未来工作至少可能拆分为以下专题，但本规约不规定启动顺序或实现方式：

- 用户终端、native CLI 发行和支持范围；
- Rust/OHOS 工具链、依赖闭包和真实 TTY；
- CLI 产品闭包、网络、凭据、路径与存储；
- shell/PTY、Git、文件监听、MCP 和后台任务；
- 安装升级、诊断、性能与扩展生态资格；
- HarmonyOS PC GUI 和移动端本地适配。

每个专题必须在启动时重新核对最新代码、目标系统和公开平台能力，并独立产出范围、设计、风险、验证和退出条件。
一个专题的成功不能自动证明其他专题可用。

## 7. 风险与支持声明

| 风险 | 规约要求 |
|---|---|
| 不存在面向目标用户的 native CLI 发行或真实终端路径 | 保持“不支持”；不得改走 HAP、移动端或 Remote 后沿用同一产品声明 |
| OHOS 被当作普通桌面 Linux，或原生依赖被默认视为可用 | 对可达的平台条件、系统调用和依赖逐项取证 |
| 为过编译建立鸿蒙专用 Runtime 或复制业务逻辑 | 停止该方案，回到现有 owner/port 边界重新设计专题 |
| 编译、`hdc shell` 或单次 demo 被当作产品支持 | 明确标为开发证据，不得进入正式能力矩阵 |
| GUI、移动端和 PC TUI 再次混写，或旧证据失效 | 保持独立产品结论；专题记录版本并在环境变化后重新验证 |

只有同时取得目标用户发行、真实系统终端、本地 Agent、常用本地编码流程和生命周期维护证据后，才能在 README、
发行说明或产品矩阵中声明 HarmonyOS PC CLI/TUI 支持。依赖解析、交叉编译、`hdc shell`、HAP demo、移动端
Remote 或其他设备代执行都不能单独形成该声明。

## 8. 本规约不展开

- 具体依赖、接口、provider、CI、打包或 PR 设计；
- HarmonyOS PC GUI 的 ArkUI/ArkWeb/HAP 技术选型；
- HarmonyOS 手机、平板或其他移动设备的本地 Runtime/TUI/GUI；
- 新权限语言或没有真实 OS 隔离依据的安全承诺。

这些内容只能在相应专题获得批准后设计和实施。
