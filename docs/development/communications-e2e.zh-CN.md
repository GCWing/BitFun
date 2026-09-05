# OpenBitFun 通信能力端到端验证手册

适用范围：OpenBitFun 1.0 系列的 SSH / Docker 工作区、Remote Connect、mobile-web、飞书 / Telegram / 微信 bot、Peer Device Mode、Detached Dispatch，以及它们依赖的账号、同步、权限、文件和事件通道。补充覆盖 Server / App Server、Shared TUI IPC、MCP、ACP、模型流式连接。此文是可重复执行的验证规范；用例存在、代码审阅或单测通过均不表示真机通过。

本手册不要求兼容 0.2.xx 旧产品。1.0 的通信双方仍需协商协议、产品身份和行为能力，不能用版本字符串相同代替能力检查。拒绝不兼容连接应保留用户数据；不能借升级重置账号、会话或远程配置。

## 1. 事实源和执行位置

| 链路 | 请求和状态的所有者 | 验证时必须区分的事实 |
| --- | --- | --- |
| SSH / Docker 工作区 | [SSH 服务](../../src/crates/services/services-integrations/src/remote_ssh)、[工作区传输设计](../architecture/remote-workspace-transport.md) | Agent Runtime 在 OpenBitFun 宿主，工作区文件和子进程在所选 SSH / 容器目标；目标不需要安装 Agent 守护程序 |
| 手机房间通道 | [mobile-web](../../src/mobile-web)、[RemoteCommand](../../src/crates/services/services-integrations/src/remote_connect.rs)、[Relay](../../src/crates/services/relay-service) | 手机发 HTTP，Relay 经宿主 WebSocket 桥接；二维码配对与账号设备身份是不同通道 |
| IM bot | [共享 provider](../../src/crates/services/services-integrations/src/remote_connect/bot)、[产品路由](../../src/crates/assembly/core/src/service/remote_connect/bot) | provider 投递游标、聊天对象、设备选择、会话及交互请求各有范围；IM 平台是外部依赖 |
| Peer Device | [设计](../architecture/peer-device-mode.md)、[前端不变量](../../src/web-ui/src/infrastructure/peer-device/README.md)、[CLI host](../../src/apps/cli/src/peer_host) | A 保持本地界面，B 拥有产品执行；切换所显示设备不等于取消 B 的任务 |
| Dispatch | [设计](../architecture/detached-task-dispatch.md)、[CLI 目标](../../src/apps/cli/src/dispatch)、[控制端](../../src/web-ui/src/features/dispatch) | 目标拥有 job、session、worktree、worker、事件及权限邮箱；控制端是观察者 |
| 其他传输 | [App Server](../architecture/app-server-architecture.md)、[Runtime 部署](../architecture/agent-runtime-deployment-design.md)、[Transport](../../src/crates/adapters/transport) | Server、Relay、Shared TUI、SDK、插件进程不能被当成同一个服务或同一条 wire |

产品操作范围取自 [Product Operation Registry](../../src/crates/contracts/product-domains/src/generated/remote-surface-registry.json)，不得另写一份允许命令表。`LegacyUnaudited` 表示仍需审计，不能算作支持；`RemoteUnsupported`、CLI `unsupported` 要实测明确拒绝。`soft_empty` 只在注册表确实声明的展示占位范围验收，不能替实际能力伪造成功。

## 2. 需要补齐的环境

先准备 A、B、R、T、M 和测试模型即可开展主线；跳板机可由 T 上独立容器模拟，后续再加异构客户端和 IM 平台。

| 代号 | 最小环境 | 用途和所需权限 |
| --- | --- | --- |
| A | 开发电脑，当前提交的 Desktop + CLI | 主控制端；独立测试数据目录和 Git 测试仓库 |
| B | 另一台电脑，优先与 A 不同系统；相同提交 Desktop + CLI | 双向控制、路径差异、终端、关闭窗口后宿主行为；可重启测试应用 |
| R | 可部署测试 Relay 的 Linux 主机或隔离 VM，稳定域名及 HTTPS / WSS | 独立数据库、两组测试账号、反向代理；可重启测试 Relay、调整代理超时及查看日志 |
| T | SSH Linux 主机，普通测试用户、Git、tar、Docker | SSH、CLI daemon、dispatch 目标；可建立临时目录、用户服务和测试容器 |
| J1 / J2 | 可由 T 承载的两个独立 SSH 跳板节点 | 双跳、各跳独立账号/密钥、某跳失联；不能直接连通目标时仍能通过跳板访问 |
| C1 / C2 | Docker 测试容器：有 sshd / 无 sshd，包含 POSIX shell | auto / sshd / docker-exec；至少一个只读临时目录、非默认 UID 的容器 |
| M1 / M2 | 手机浏览器：Android Chrome 与 iPhone Safari | 扫码、摄像头权限、后台恢复、Wi-Fi / 蜂窝切换；先有一台即可开测，另一台保持待测 |
| F / G / W | 飞书测试应用、Telegram 测试 bot、微信 iLink 测试账号 | 仅测试聊天对象；可撤销凭据、查看事件投递、触发重投和账号重新绑定 |
| P | 已配置的模型服务和一个支持工具调用的测试模型 | 每个实际执行任务的宿主独立可用；记录模型名、推理档位和费用上限 |
| N | 可控故障注入位置，例如隔离代理 / 测试容器网络 | 延迟、限速、断流、丢响应；只影响测试链路，不断开开发机管理通道 |

R 与 T 可以共用一台机器，但用独立容器、端口和存储；“重启 Relay”不能顺带杀死 dispatch worker。验证真实物理掉线和系统休眠仍需要 B。面向 Windows/macOS/Linux 的最终结论必须分别有原生宿主证据，VM 或 Linux 容器不能证明 Windows ConPTY/macOS WebKit 行为。

提供环境时填写：系统与 CPU 架构、SSH 配置别名、可操作的测试目录、Relay URL、是否允许重启测试服务、手机型号/浏览器、已就绪的 bot 种类、模型可用情况。凭据在目标应用或测试用户的本地配置中填写；记录只保留配置别名、指纹和脱敏标识。

Relay 部署及账号创建沿用 [Relay README](../../src/apps/relay-server/README.md)，飞书权限和事件订阅沿用 [飞书配置手册](../remote-connect/feishu-bot-setup.zh-CN.md)。自动 SSH bootstrap 需要包含当前协议和能力的已签名预编译 CLI 发布；若发布尚未就绪，先在 T 配置同提交 CLI 验证既有 runner 路径，并把自动安装单列为阻塞项，不在目标偷偷改为源码编译。

## 3. 测试数据、证据和判定

每轮生成唯一 `run_id`，记录 A/B/R/T 的 commit、二进制 SHA-256、产品身份、协议、协商能力和浏览器版本。使用专属空测试账号及临时 Git 仓库，不把真实项目当作故障注入夹具。

在 A、B、T、C1、C2 建立同名路径，分别放置内容不同的 `owner-sentinel.txt`。所有读、搜索、终端、工具、下载、同步断言都必须校验目标标记。仅看到文件内容或命令退出 0，不能证明路由正确。

Git 夹具包含：已推送提交、仅本地提交、未暂存/已暂存/未跟踪文件、被 ignore 的文件、与远端分叉的分支。文件夹具包含：空文件、二进制、UTF-8/UTF-16、多字节跨块、空格/中文/引号/换行文件名、符号链接、大小写冲突、只读文件和大文件。大文件至少分为 1 MiB、16 MiB、64 MiB 和各接口上限两侧；不要求某条有明确上限的通道传输超限输入。

需要等待的任务使用一个可控、可观察的测试工具：在专属目录写入开始标记，等待释放文件后输出结束标记；同时记录 PID、turnId 和调用次数。网络重试用同一个稳定请求身份注入重复请求，检查宿主执行记录和副作用次数，不能用模型自己声称“执行一次”作为证据。

状态仅使用 `PASS`、`FAIL`、`BLOCKED`、`NOT_RUN`、`UNSUPPORTED_VERIFIED`。不支持必须有宿主能力和明确拒绝证据；未执行不能填 PASS。每项证据至少包含：执行端、控制端、操作步骤、UTC 时间、request/turn/job ID、预期、实际、日志或截图位置。日志、HAR、截图入记录前去掉 token、配对 secret、密码、文件正文和账号私密信息。

下载和传输用双方 SHA-256、字节数判断；幂等用执行次数判断；恢复用目标 journal、streamId/cursor、权限 requestId 和终态判断。记录首次响应、恢复耗时、P50/P95、吞吐、峰值 RSS 和遗留进程数。性能先建立同夹具基线；正确性不能通过降低数据量、丢弃输出或调高审计基线换取。

导出逐项工作表（输出目录必须尚不存在，避免覆盖已填写证据）：

```bash
node scripts/diagnostics/export-communications-matrix.mjs --output /tmp/openbitfun-communications-run-001
```

输出包括全部产品操作、全部 `RemoteCommand`、本手册场景和注册表 digest。每个 bot provider、OS、设备方向和网络条件复制独立执行行；工作表只是范围清单，不是自动测试结果。

## 4. 执行顺序

1. 本地 owner 单测、协议契约及静态边界检查；确认当前构建可用。
2. A↔R、A↔T、M→A、各 bot→A、A↔B、A→T dispatch 分别验证直连正常路径。
3. 对每条链路分别注入失败；先断请求前，再断宿主已接收后、响应返回前。
4. 验证组合拓扑和跨设备相同路径/会话 ID 冲突。
5. 每次修复先跑复现用例和对应 owner 测试，再复测受影响链路及组合。记录前后证据。
6. 所有阻断问题关闭后，进行长时间运行和升级/回退演练。

### 4.1 环境与基线

| ID | 步骤 | 预期与证据 |
| --- | --- | --- |
| ENV-01 | 记录各端构建、产品身份、协议及 capability 响应 | 构建可追溯；协议相同但缺能力的目标仍不可被当作兼容 |
| ENV-02 | 检查 R 的 health、账号数据库、静态 mobile-web 和 WebSocket upgrade | 三者分别有真实响应；health 通过不代替账号及路由通过 |
| ENV-03 | 建立 A/B/T/C1/C2 同名工作区和不同 sentinel | 文件与工具输出能准确证明执行域 |
| ENV-04 | A、B 分别配置模型并独立运行一个工具任务 | 模型凭据和工具实际可用，不依赖另一端代跑 |
| ENV-05 | 新测试用户首次启动，再从有效 1.0 数据目录启动 | 首次初始化可用；已有设置、会话、连接保留 |
| ENV-06 | 限制一个测试目录为不可写、令一个测试目标离线 | 清晰错误；已有记录保留；未访问同名本地目录 |
| ENV-07 | 记录空闲进程、连接、RSS、句柄/FD 数量 | 后续重连与长测有可对比基线 |
| ENV-08 | 导出产品操作及 RemoteCommand 工作表，固定 registry digest | 所有操作均有处置行；未审计项不隐式放行 |

### 4.2 Relay、配对、账号和同步

| ID | 步骤 | 预期与证据 |
| --- | --- | --- |
| RLY-01 | A 创建房间，M 扫码并完成 challenge/echo | 只在完成配对后提供工作区和会话数据 |
| RLY-02 | 使用错误 secret、过期二维码、另一房间 challenge、损坏密文 | 拒绝；无业务调用，无其他房间数据 |
| RLY-03 | 二次扫码、刷新页面、A 重连后再次访问原房间 | 按有效期和配对状态恢复或明确要求重配；不出现假在线 |
| RLY-04 | 拨号地址无效、DNS 失败、TCP 可达但不完成 TLS/WS 握手 | 有限时间失败，状态离开 Connecting，可再次连接 |
| RLY-05 | 在拨号、活跃连接、2/4/8 秒重连等待期间主动断开 | socket、心跳和重连停止；等待超过原重连周期仍不复活 |
| RLY-06 | 连续连接 R1→R2→R1，向旧 socket 延迟投递响应 | 旧任务不能改写新连接状态、房间或认证上下文 |
| RLY-07 | 仅黑洞连接，不发送 FIN/RST，随后恢复网络 | 心跳/空闲检测进入重连；恢复房间与账号；不重发不确定业务命令 |
| RLY-08 | A/B 同账号上线，另一个账号的 C 上线；枚举、RPC、同步交叉访问 | 同账号设备可见；跨账号访问被拒绝 |
| RLY-09 | 删除设备、撤销/过期 token，保留原 WebSocket | 已有连接失去路由权限，在线列表最终收敛；旧 token 不能重新登记 |
| RLY-10 | 手机委派 token 请求账号管理/设备发放；完整 token 执行合法操作 | 权限范围按 token 类型执行，不因同账号扩大委派权限 |
| RLY-11 | 相同设备 ID 建立新 socket，再关闭旧 socket | 新连接保持在线，旧连接清理不删新 owner |
| RLY-12 | 重复及未知 correlation ID、错来源响应、响应晚于调用超时 | 不串答，不占用其他请求，不泄露响应正文 |
| RLY-13 | 慢读接收端、并发 RPC、大事件和大附件同时发送 | 背压/过载有明确结果；无静默截断；记录 RSS、队列及恢复时间 |
| RLY-14 | 直连与带 /relay 前缀的 HTTPS 反向代理运行同一流程 | HTTP 路径、静态资源、WS upgrade、body limit 和超时一致可用 |
| RLY-15 | 数据库不可写或磁盘满时登录、配对、同步、设备 provisioning | 持久化失败不宣告成功；原账号/数据保留；恢复后可重试 |
| RLY-16 | 账号登录在“云端/本地设置选择”前关闭窗口，再重新登录 | 未完成选择不提交半成品登录；成功后持久化一致 |
| RLY-17 | A/B 并发改不同设置、同步过程中本地再次修改、旧快照缺字段 | 不用过期拉取覆盖较新本地写入；动态删除和保留字段符合同步规则 |
| RLY-18 | 会话上传/下载中断、重复拉取、删除/恢复、历史按窗口加载 | 无重复或残缺成功；恢复读取实际所有者，记录完整性 |
| RLY-19 | 只重启 R、只重启 A，再重启两者 | 在线状态和配对按其持久化合同恢复；后台目标任务不因 R 重启被取消 |
| RLY-20 | 未授权 Origin、错误 CORS、未登录 HTTP、公开健康端点分别访问 | 浏览器来源检查与账号鉴权分别成立；公开健康不暴露凭据 |

### 4.3 SSH / Docker 工作区

以下正向路径至少参数化执行 direct SSH、双跳、容器 sshd、远程 docker-exec、本地 docker-exec；客户端覆盖 A 与异构 B。

| ID | 步骤 | 预期与证据 |
| --- | --- | --- |
| SSH-01 | 密码、私钥、带口令私钥、ssh-agent、证书、交互式 OTP 分别连接 | 实际认证成功；不支持的组合说明原因；临时口令/OTP 不落配置 |
| SSH-02 | 首次主机密钥、已知密钥、更换密钥后连接 | 主机身份验证符合策略，不静默接受变化 |
| SSH-03 | 导入 SSH config 别名、IPv6、非默认端口及两跳 ProxyJump | 每跳参数准确；失败报告定位到实际跳点 |
| SSH-04 | 第一跳/第二跳/最终目标分别拒绝认证、超时、断线 | 前序连接释放；无跳过失败节点或本地回退 |
| SSH-05 | auto 模式先无 sshd，启用 sshd 后重连，再禁用 sshd | effective target 随探测更新；保存的 auto 配置不被固定替换 |
| SSH-06 | Docker 列表、停止的容器、无 docker 权限、错误 shell | 明确阶段错误；不能在 Docker 宿主代替容器执行 |
| SSH-07 | Read/Write/Edit/Delete/LS、相对路径、绝对路径及多根会话 | 每次校验目标 sentinel；算法与目标 FS 状态一致 |
| SSH-08 | Grep/Glob、literal/regex、Unicode、ignore、无 rg/flashgrep | 匹配语义和计数可对照；后端失败不是零结果；回退成本可观察 |
| SSH-09 | 大文件头/尾/窗口读取、UTF-16、多字节跨块、二进制 | 返回字节/行范围正确；不会因传输分块插入替换字符 |
| SSH-10 | 上传/下载文件及目录，中途取消和断线 | 完成文件 SHA-256 相同；失败不覆盖有效目标为残片 |
| SSH-11 | 换行/引号/中文路径、大小写冲突、非法本地文件名 | 支持则精确往返；不能支持则逐项显式拒绝，不丢文件 |
| SSH-12 | 符号链接、目录链接、硬链接及只读文件上 Edit/Write | 工具与传输各自保留已声明的链接/权限语义；越界不被跟随 |
| SSH-13 | PTY 输入、resize、交互 shell、Ctrl+C/Ctrl+Z、退出码 | 事件及信号在目标有效；长输出无编码损坏 |
| SSH-14 | 非 TTY Exec stdin/stdout/stderr、长任务、非零退出 | stdin 与输出完整；异常退出不能显示成功 |
| SSH-15 | 在 channel-open 未确认、运行中、写入提交时分别取消 | 晚到 channel 关闭且不执行已取消命令；兄弟会话继续工作 |
| SSH-16 | 容器内父子进程、setsid、只读临时目录下取消 Exec | 按支持条件终止目标进程；记录降级边界及遗留进程，不能只杀本地 docker 客户端就算通过 |
| SSH-17 | Git status/diff/log/branch/worktree 与本地同内容夹具对照 | 数据来自目标；不对控制端 Git 配置或 checkout 产生副作用 |
| SSH-18 | 端口转发建立、重复端口、取消、SSH 断开后重建 | 流量到目标服务；端口冲突可见；关闭后无监听泄漏 |
| SSH-19 | 两个 SSH 主机都有 /work/project，切换时延迟旧 FS/search 响应 | connection ID 与路径联合定位；旧响应不污染新工作区 |
| SSH-20 | 保存连接后退出；删除凭据或使主机离线再启动 | 保留配置与工作区，显示缺凭据/离线，不自动删除 |
| SSH-21 | 遍历大目录、慢 SFTP、失效 channel 后并发重试 | 超时/取消有界，过期 SFTP 实例不能使新连接失效 |
| SSH-22 | 同一 SSH 连接运行两个会话，取消或断开其中一个工作区 | 仅影响声明范围；其他工作区进程和数据保持可用 |
| SSH-23 | 远程 MCP/ACP 子进程读写测试 workspace sentinel | 进程和文件位于目标；不可用时明确拒绝，不启动本地替代 |
| SSH-24 | 远程历史、快照、Undo/恢复入口逐项探测 | 支持的操作实际复原；尚未支持的完整 Undo 明确 gated，不把单文件快照等同完整回滚 |
| SSH-25 | 遍历注册表 RemoteRouted / WorkspaceAgnostic / LocalOnly / RemoteUnsupported / LegacyUnaudited 操作 | 每项有路由/拒绝/待审结论；所有新增事实回写既有 registry owner |

### 4.4 Mobile Web

房间配对和账号直接登录各跑一轮；每个关键流程重复切换到另一台同账号目标。每项记录浏览器与是否后台/锁屏。

| ID | 步骤 | 预期与证据 |
| --- | --- | --- |
| MOB-01 | 扫码、手填链接、摄像头拒绝、扫码后刷新和重新打开 | 可恢复或可重新配对；错误有行动入口 |
| MOB-02 | 初始同步、workspace/assistant 列表、选择本地及 SSH 工作区 | 会话列表与工作区绑定一致，携带正确 remote_connection_id |
| MOB-03 | 创建、搜索、分页、重命名、删除和重新打开会话 | 宿主状态一致；失败不会被解释为无会话而自动新建 |
| MOB-04 | 选择模型和 reasoning preset，清除覆盖，再刷新 | 目标目录能力真实；缺能力时禁止发送无效配置 |
| MOB-05 | 文本、图片、多图、结构化文件上下文及超限附件提交 | 一次接受产生一个 Turn；超限/不支持明确失败 |
| MOB-06 | 流式文本、thinking、工具开始/结束、计划 Build、steer、取消 | 状态从宿主确认；完成尾部、错误与非成功终态准确 |
| MOB-07 | 触发确认、拒绝、AskUserQuestion、自定义答案，后台后再回答 | 应答命中拥有者；过期或重复应答不推进别的工具 |
| MOB-08 | 查看/切换权限模式，恢复页面后再次查询 | 共享策略与显示一致，不因页面重载回到隐式自动批准 |
| MOB-09 | 网络正常/慢速下 poll，宿主持久化落后于 TurnCompleted | 不用旧历史覆盖新流式内容；完成尾部最终收敛 |
| MOB-10 | 锁屏、切后台 1 分钟、Wi-Fi→蜂窝、飞行模式再恢复 | 连接状态与禁用态一致；恢复会话、输出及仍待答交互 |
| MOB-11 | 设备 A→B→A 快速切换，故意延迟 A 第一轮响应 | epoch 拒绝 ABA 陈旧结果；同 ID 会话也不混淆 |
| MOB-12 | 文件分块下载途中切换设备 | 后续分块停止；不能把不同目标的字节拼成文件 |
| MOB-13 | 委派身份刷新期间请求、两个刷新乱序返回 | 只提交最新身份；凭据未确定时不发副作用请求 |
| MOB-14 | 真实 HTTP 401，分别刷新到同账号和另一账号 | 同账号按策略重试一次；跨账号业务请求不重发 |
| MOB-15 | 返回 HTTP 200 的加密业务错误，内容含 Unauthorized 或 HTTP 401 | 保留业务错误；mutation 只发送一次，不触发身份刷新 |
| MOB-16 | POST 已被宿主接受后丢响应；读请求则连续 502/504 | mutation 不盲目重试；读重试有总预算和真实网络 deadline |
| MOB-17 | 只发响应头、不结束 body | body 读取被同一 deadline 中止；不永久转圈 |
| MOB-18 | 配对宿主退出账号、改登录另一账号，再获取设备列表 | 原账号数据、缓存与目标清理；不沿用旧设备授权 |
| MOB-19 | 无工作区、无可用模型、目标 CLI 不支持的能力 | 明确缺失状态；不显示空成功或控制端代跑 |
| MOB-20 | 回到首页、断开、重新登录、重新打开同一 URL | 缓存作用域准确；断开不取消宿主已接收的后台工作 |

### 4.5 IM Bot

每一行分别执行 Feishu、Telegram、Weixin，不能用其中一个 provider 的通过代替其他两个。外部平台投递若不可控，标 BLOCKED 并保留待验证条件。发送操作只针对预先指定的测试 chat。

| ID | 步骤 | 预期与证据 |
| --- | --- | --- |
| BOT-01 | 正常配置、错误凭据、缺平台权限、连接失效 | 正确连接或具体配置错误；没有永久假在线 |
| BOT-02 | 正确/错误/过期配对码，其他聊天对象使用配对码 | 绑定范围和配对有效期可验证；未授权聊天不能控制会话 |
| BOT-03 | menu/help/settings、中文/数字/全角输入、返回和翻页 | 数字选择与当前 PendingAction 对应，不误作消息或配对 |
| BOT-04 | workspace/assistant 切换及创建/恢复会话 | 使用共享会话 owner；SSH 工作区保留 connection identity |
| BOT-05 | 切换模型、专业/助理模式、verbose/concise | 已支持选项生效；不支持的产品模式有明确说明 |
| BOT-06 | 普通文本、长中文、Markdown、图片及生成文件返回 | 按 provider 上限分块，顺序完整；文件摘要正确，无静默截尾 |
| BOT-07 | 相同 webhook/update 重投、乱序、长轮询超时后重启 | 同一平台消息不创建重复任务；游标恢复不跨 bot/chat |
| BOT-08 | 运行中追加消息、指定 Turn 取消、过期 cancel | 只影响目标 Turn；不能取消新 Turn 或其他聊天会话 |
| BOT-09 | 触发工具确认/拒绝、问题多选/自填，然后应用重启/断网 | 仍待答交互可重建或明确不可恢复；不能静默卡住任务 |
| BOT-10 | 两个应答端同时回答同一请求，随后重复旧按钮/数字 | 宿主只接受一次，第二次结果明确，不覆盖新请求 |
| BOT-11 | /devices 列表、切换 B、B 离线、再回到 A | 数据和命令在所选设备；离线不回退本机 |
| BOT-12 | bot 选 B 执行时切换 A 的桌面工作区 | B 的绑定不跟随 A UI 改变；输出仍属于原 chat/session |
| BOT-13 | 平台 429/5xx、下载附件失败、发送完成消息失败 | 遵循 provider 返回的恢复条件；失败可见，已运行任务不被再执行 |
| BOT-14 | 解绑/撤销平台 token 后重投旧更新并重启应用 | 旧绑定不能恢复授权；其他 provider 正常工作 |
| BOT-15 | 会话事件超出接收窗口，尤其丢 AskUserQuestion/结束事件 | 历史/交互必须收敛或显式报缺口；仅打印 lag 日志不能判 PASS |
| BOT-16 | 在 bot 请求没有实现的 reasoning、附件或 dispatch 操作 | 明确不支持及可用替代入口；不能按普通聊天执行意外含义 |

### 4.6 Peer Device 多端互控

至少覆盖 A→B Desktop、B→A Desktop、A→T CLI host；再增加两个 controller 同时附着。读写操作按注册表逐项展开。

| ID | 步骤 | 预期与证据 |
| --- | --- | --- |
| PEER-01 | 登录同账号、发现设备、attach、切换本机、显式 detach | attachment 与当前显示 surface 分离；状态与实际在线一致 |
| PEER-02 | A/B 相同路径和相同 session ID，快速来回切换 | workspace、draft、消息、工具、错误及缓存按 device+identity 隔离 |
| PEER-03 | 发消息尚在异步准备时切换设备；ACK 前后分别丢响应 | 未接受请求回到原 surface；已接受 Turn 不重复提交 |
| PEER-04 | 相同 sessionId/turnId 并发重复 start_dialog_turn / ACP submit | 有 capability 时宿主合并；无 capability 时单次发送或显式拒绝 |
| PEER-05 | 选择工作区、目录 picker、创建/恢复会话、模型和配置页 | 路径和数据来自 peer；配置 hydrate 不被低优先级 IO 饿死 |
| PEER-06 | 编辑、文件树、搜索、Git、终端持续并发工作 | 命令符合 registry；终端仍可交互，记录 P95 延迟 |
| PEER-07 | 下载文件/目录，在 A 选择保存目录，B 为另一操作系统 | 字节从 B 读取、文件写到 A；不把 A 路径交给 B 写入 |
| PEER-08 | B 的本地 PTY 与 B→T 的 SSH PTY，输入/resize/中断 | 两类终端事件都能到 A；关闭一个不影响其他终端 |
| PEER-09 | B 运行中切到 A，再回 B；B 继续输出超过一次持久化周期 | 回来立即重建当前 Turn，无只剩 prompt、工具冻结或尾部丢失 |
| PEER-10 | restore 在途时投递旧/新 cursor 事件；替换 streamId | 只在同一 stream 比较 cursor；事件与 snapshot 不重复、不倒退 |
| PEER-11 | 主动丢 ToolEnd/TurnCompleted、令订阅失败、隐藏窗口 | gap 触发重建；订阅重建和 attach 重试不会互相禁用 |
| PEER-12 | pending permission/AskUserQuestion 时断连、切设备、再 attach | interactionSnapshot 重建，正确 session 应答；旧 snapshot 不覆盖新答案 |
| PEER-13 | B 关闭最后一个 controller 连接，等待任务结束再连 | 已接受 Turn 持续运行；只有真实宿主事件流失效按其策略失败 |
| PEER-14 | 两个 controller 附着、抢答同一交互、提交并发请求 | 宿主 lease/owner 仲裁准确，不存在两个独立 Session writer |
| PEER-15 | 大历史窗口、搜索旧轮、turn rail、目标回滚 | 有界窗口按需读取；回滚 capability gate，不能在 A 本地执行 |
| PEER-16 | account_login/logout、updater、window、relay_deploy 等 controller_local 操作 | 在控制端正确执行且 peer 明确拒绝对应代理尝试 |
| PEER-17 | git_trust_repository、未注册命令、退休前缀、CLI unsupported 操作 | 区分 operator_only / retired / unsupported / unknown，不伪造成功 |
| PEER-18 | MiniApp、ProductControl、context files 及 native/presentation 子能力 | 按实际 peer 能力执行；CLI 缺 native/UI 时不改控制端 |
| PEER-19 | peer 断网、更换进程/降级能力后重连 | 能力缓存失效；旧成功能力不沿用；已存在会话保留 |
| PEER-20 | 连续读取超时及 mutation 结果不确定 | 读按预算重试；非幂等 mutation 单次，不把 UI timeout 当网络已取消 |
| PEER-21 | B 本地发起 Turn，A 尝试观察和回答权限 | 按 Desktop/CLI 各自支持范围验收，不能用 CLI 仅跟踪 peer Turn 的限制声称全面同步 |
| PEER-22 | 逐项执行 registry 的 proxied / controller_local / host_control_plane / operator_only | 全部命令都有证据或明确阻塞；不手写第二份路由表 |

### 4.7 Detached Dispatch

SSH 与账号设备 RPC 两种传输各跑一轮；目标分别为 CLI daemon 和具有已安装 CLI runner 的 Desktop。任务使用 Git 专属夹具。

| ID | 步骤 | 预期与证据 |
| --- | --- | --- |
| DSP-01 | probe 正常、无 CLI、旧协议、缺单项 capability、不同 productId/dataNamespace | probe 只读；不兼容在准备 workspace 前被拒绝 |
| DSP-02 | SSH 自动安装，分别模拟错误签名/摘要、错误架构、网络慢/断 | 只接受匹配平台已签名产物；失败不提交任务；安装审计持久化 |
| DSP-03 | 账号设备缺 runner，控制端在线/离线分别尝试 | 明确缺 runner；不能经 Relay 临时安装或回到控制端执行 |
| DSP-04 | HEAD、有未推送提交、无 remote 的仓库提交 | 精确 base commit；必要时 Git bundle 上传，目标 managed worktree 正确 |
| DSP-05 | includeUncommitted 打开/关闭；含 staged/unstaged/untracked/ignored 文件 | 基线内容符合选项；用户 checkout/index/branch 不改；ignore 不随意传输 |
| DSP-06 | 修改 base ref 后重试同一 job；相同 ID 不同 payload | 原 immutable base 不漂移；冲突请求明确拒绝 |
| DSP-07 | bundle begin/chunk/commit 断点、重复 chunk、错 offset/大小/摘要 | 同一上传身份可恢复；未校验完成不导入；拒绝越界路径 |
| DSP-08 | provision/sync 慢 Git 子进程超过单次 RPC，期间控制端退出 | 目标进程继续；同一操作 poll 恢复，不重复创建 worktree |
| DSP-09 | submit 正常 ACK 后立即退出 A、停止 R、关闭 SSH | T 独立执行并持久化；A 上不创建目标 session 或代理文件读写 |
| DSP-10 | 目标已持久接受但丢 submit 响应，再 status/同 ID retry | 标记 submission_unknown 并收敛为原 job；副作用只执行一次 |
| DSP-11 | worker 启动前、接受 Turn 后、执行中分别重启目标 | 使用持久记录判断状态；已接受 prompt 不被自动重放 |
| DSP-12 | auto / reject-and-report / remote 三种审批策略触发同一需确认工具 | 分别遵循共享 auto、明确拒绝、持久权限邮箱；无人值守不死锁 |
| DSP-13 | A 离线时产生权限，B 采用同一 job 并回答；重复/冲突答案 | 目标持久仲裁，继续执行且只处理一次，不依赖原提交端 |
| DSP-14 | 运行中 append，同 messageId 重复及不同内容冲突 | 只追加一次，冲突拒绝；当前 Turn 持续 |
| DSP-15 | 完成后 continue，同 turnId 重试；更换模型/推理档及附件 | 复用目标 session/worktree，新增一个 Turn，事件 cursor 持续增长 |
| DSP-16 | queued/running/pending permission 时取消；模拟 PID 被替换 | 取消意图持久化；只信号确认的 worker/process group，不误杀无关进程 |
| DSP-17 | 同 execution root 并发两个任务，等待中取消其中一个 | 目标 workspace lock 串行化，排队可见且可取消 |
| DSP-18 | status 小页/旧 cursor/超大事件/日志轮转 | next cursor、reset、truncated、omission 和 completeness 如实返回 |
| DSP-19 | A 缓存后重启；删/损坏/超限 transcript cache | cursor 与投影成对恢复；无有效缓存从可用起点重放并保留缺失标识 |
| DSP-20 | B list/adopt 同一任务，A/B 同时观察 | 观察游标独立；不创建第二 worker、不抢 session writer |
| DSP-21 | 运行中/完成后 sync，重复同 operationId，稍后同 knownHead 再新 sync | 同次 poll 幂等；新点击可发现后续变化；目标与基线正确快进 |
| DSP-22 | 同步期间断网、Git index lock、基线分叉/删除/换 branch | 不 reset/rewrite 用户历史；保留可重试 artifact；knownHead 不提前推进 |
| DSP-23 | 准备中崩溃、过期 preparation 与 live retry 并发、outbound 读取失败 | claim 不被错误释放；有 durable owner 或读结果不确定时保守保留 |
| DSP-24 | 模拟保留期到期：terminal 与 queued/running 混合任务 | 只按各自保留规则清理；释放 claim 成功后才能移除 outbound |
| DSP-25 | 在 Peer 模式提交 dispatch，并断开最后一个 Peer controller | 控制端 dispatch 命令留本地；目标专用 verb 不需 attach lease；job 不被取消 |
| DSP-26 | 目标用户服务 bootstrap 成功及发凭据后失败、失败后目标已换账号 | 在线可见才成功；回滚按预期身份执行，不登出后来建立的账号 |

### 4.8 组合、弱网和持续运行

| ID | 步骤 | 预期与证据 |
| --- | --- | --- |
| COMBO-01 | M→A，A 的会话绑定 T 的 SSH 工作区，读写 sentinel、提交和审批 | 手机控制、Runtime、FS 三处身份正确，权限可从手机解除 |
| COMBO-02 | M 先配对 A 再选择 B，B 的会话绑定 T；执行并切回 A | B/T 继续工作，A 本地文件不被读取/修改；回 B 能恢复投影 |
| COMBO-03 | F/G/W→A 再选 B，B→C 的容器工作区，返回生成文件 | 三个 provider 各自验证；文件和执行都在选定容器 |
| COMBO-04 | A→B Peer，B→T 双跳 SSH PTY，后台下载同时中断命令 | PTY 保持交互；信号到 T，下载/其他会话范围正确 |
| COMBO-05 | A 提交 T dispatch，M/B 观察，A/R 先后退出 | worker 不依赖 A/R；恢复后事件和权限邮箱可继续消费 |
| COMBO-06 | 同一账号同时运行 mobile、bot、两个 peer controller、dispatch | session、turn、request 和 source device 不串线；记录争用与错误 |
| COMBO-07 | 延迟 50/200/1000ms、限速 512/128 KiB/s、短断 5s/长断 90s | 每档分别保存结果；不重复执行、不伪造完整性；恢复耗时可解释 |
| COMBO-08 | 只丢请求、只丢响应、重复投递、乱序事件、半开 socket 分别注入 | 区分未接受和 outcome unknown；按协议幂等与 cursor 恢复 |
| COMBO-09 | 50 次连接/断开/切换；再运行 2 小时混合负载和 8 小时空闲唤醒 | 最后停止测试工作后资源收敛；无持续增加的 socket、任务、FD 或 worker |
| COMBO-10 | 1.0 两个能力不同构建混跑，R/host/client 分别单独升级与回退 | gate 和数据保留正确；跨协议拒绝清晰；不要求连接 0.2.xx 成功 |

### 4.9 其他通信边界

这些边界不因为主线成功自动算通过。未启用的产品入口记录为范围外并说明构建事实；已发布入口仍必须执行。

| ID | 步骤 | 预期与证据 |
| --- | --- | --- |
| EXT-01 | Server Web UI 登录、WebSocket initialize、RPC、事件及断连 | 真实 handler、鉴权、范围限制成立；无任意 Core RPC 旁路 |
| EXT-02 | App Server 错帧、超限帧、错误顺序、未知方法、慢读和 EOF | 结构化拒绝、有界队列/取消；连接内 cursor 不被宣称跨连接持久重放 |
| EXT-03 | Shared TUI 两客户端同 workspace 与同 session；关闭一个/最后一个 | 单 controller/session 与 ownership lock 成立；按 Shared 的取消及空闲退出规则，不套用 dispatch 语义 |
| EXT-04 | Shared IPC 错 token、旧协议、握手不完成、owner discovery 被替换 | 本机通道拒绝；旧进程不清理新 owner 文件，不开放 TCP fallback |
| EXT-05 | MCP stdio：启动、工具调用、并发、超大/损坏响应、取消、子进程退出 | 使用同一 MCP owner；退出无残留进程，UI 状态和可用工具目录收敛 |
| EXT-06 | MCP Streamable HTTP：认证、会话失效、重连、服务 429/5xx/OAuth 过期 | 不盲目重发有副作用工具；认证交互可到驾驶端或明确 unsupported |
| EXT-07 | ACP 本地与 SSH stdio：初始化、文件/终端请求、许可、取消、异常 EOF | 外部 agent 作用域准确；状态/输出/关闭协议可靠，不把 native Turn 幂等承诺套给未知 provider |
| EXT-08 | 模型 SSE/WebSocket：多字节/JSON 跨块、首包超时、流中断、429、5xx | 调用与执行状态保真；不把部分响应当完整，也不重复已有工具副作用 |
| EXT-09 | AI relay 模型请求来自本地/peer/dispatch 不同执行端 | 使用声明的模型 provider 和凭据域；断线不能偷偷改变执行宿主 |
| EXT-10 | Plugin Host IPC、SDK stdio：帧边界、correlation、取消、worker 崩溃、背压 | 各协议独立限制与生命周期成立；不复用客户端数复制 Runtime owner |
| EXT-11 | LAN、ngrok/自建 Relay、嵌入式 Relay 运行手机主线 | 同一共享路由契约；公网地址变化、端口占用和服务停止明确反映 |
| EXT-12 | 发布页面/附件上传、读取、账号 sync 大包与设备 RPC 并发 | 各路由认证和大小上限一致；没有把 HTTP body limit 当业务完整性保证 |

## 5. 本地自动验证入口

在仓库根目录执行；Rust 同一 target 目录的命令顺序运行，避免多个 Cargo 进程等待同一构建锁。优先复用各 owner 的最小 feature，不使用 `--all-features` 或 `product-full` 代替通信专项验证。

```bash
pnpm install --frozen-lockfile
pnpm --dir src/web-ui run gen:types
pnpm --dir src/mobile-web run type-check
pnpm run build:mobile-web
pnpm run check:web
pnpm run check:core-boundaries
```

```bash
cargo test --locked -p openbitfun-relay-service
cargo test --locked -p openbitfun-services-integrations --no-default-features --features remote-connect --lib remote_connect::
cargo test --locked -p openbitfun-services-integrations --no-default-features --features remote-connect --test remote_connect_contracts
cargo test --locked -p openbitfun-services-integrations --no-default-features --features remote-ssh-concrete --lib remote_ssh::
cargo test --locked -p openbitfun-services-integrations --no-default-features --features remote-ssh --test remote_ssh_contracts
cargo test --locked -p openbitfun-cli --bin openbitfun dispatch::
cargo test --locked -p openbitfun-cli --bin openbitfun peer_host::
cargo test --locked -p openbitfun-agent-runtime-ipc
cargo test --locked -p openbitfun-app-server --lib server::wire::tests
cargo test --locked -p openbitfun-app-server-protocol --test legacy_wire_contracts
```

```bash
pnpm --dir src/web-ui run test:run src/infrastructure/peer-device src/infrastructure/api/adapters/peer-device-adapter.test.ts src/infrastructure/api/generated/remoteSurface.test.ts src/features/dispatch src/features/ssh-remote src/features/relay-deploy src/app/components/RemoteConnectDialog src/flow_chat/services/flow-chat-manager/PeerSessionRefreshModule.test.ts src/flow_chat/session-stream src/shared/utils/remoteSessionScope.test.ts
```

真实 Docker 集成测试需显式提供测试容器名（该容器允许创建及删除测试文件）：

```bash
OPENBITFUN_TEST_DOCKER_CONTAINER=openbitfun-comm-fixture cargo test --locked -p openbitfun-services-integrations --no-default-features --features remote-ssh-concrete --lib remote_ssh::manager::tests::local_docker_workspace_round_trip -- --ignored
```

Desktop 命令闭包和 Core 行为变更还需执行其最近 [Desktop guide](../../src/apps/desktop/AGENTS.md)、[Core guide](../../src/crates/assembly/core/AGENTS.md) 指定的检查；MCP、ACP 和 SDK 分别遵循各 owner guide。清单没有运行日志时，不得把这里的命令标成已通过。

## 6. 放行与问题闭环

- P0：跨账号/设备/工作区越界、重复产生副作用、错误成功、破坏数据、错误 PID 信号。发现即停止相关发布路径，保留复现夹具并修复。
- P1：权限无法远程解除、不可恢复断连、完整性误报、持续资源增长、核心入口缺失且不提示。必须修复或明确关闭该能力入口后才放行。
- P2：可恢复体验问题和有证据的性能退化，记录条件、基线和处理决定。

闭环记录采用 `case_id + 拓扑 + 故障位置 + commit` 作为身份，保留失败原始证据、根因、最小修复、owner 回归测试、同拓扑复验和组合复验。协议变更同时更新 capability/version、各端 reader/writer 和本手册，避免只更新发起端。

最终签收要求：关键主线每种实际支持的传输、目标 OS 和手机浏览器有正向及故障证据；所有工作表行都有明确处置；P0/P1 为零或对应功能已明确 gated；混合拓扑和持续运行完成。仅本地通过、外部环境缺失、跳过测试和未审计 backlog 必须逐项列出，不能写成“全功能通信已验证”。
