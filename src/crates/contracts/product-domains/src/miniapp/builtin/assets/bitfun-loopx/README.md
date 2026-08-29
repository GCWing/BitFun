# bitfun-loopx（内置 MiniApp）

内置版 **bitfun-loopx**：粘贴 GitHub Issue 链接，由 BitFun 宿主 Agent 驱动本机
LoopX 引擎持续修复，心跳调度、人工审批、中途插话。

本目录就是该内置 MiniApp 的**唯一权威源码**，不依赖任何外部仓库快照。五件套
`index.html` / `style.css` / `ui.js` / `worker.js` / `esm_dependencies.json` 由
`src/crates/contracts/product-domains/src/miniapp/builtin.rs` 的 `BUILTIN_APPS`
以 `include_str!` 嵌入二进制，注册 id 为 `builtin-bitfun-loopx`（同文件契约测试
含 id 顺序断言）；`meta.json` 的权限与注册条目保持一致。

## 高效修改流程

### 先判断改动属于哪一层

| 修改文件 | 改动类型 | 开发期最小动作 | 何时需要 Desktop 编译 |
|---|---|---|---|
| `index.html` / `style.css` | MiniApp 结构、布局、样式 | 集中完成一批修改，不跑 Rust 检查 | 真机验证前编译一次 |
| `ui.js` | MiniApp 交互和宿主桥调用 | `node --check <本目录>/ui.js` | 真机验证前编译一次 |
| `worker.js` | 兼容占位（必须保持无业务逻辑） | `node --check <本目录>/worker.js` | 不单独启动 Worker |
| `meta.json` | 权限、版本、运行方式 | 检查 JSON 和权限差异 | 必须编译并 reseed |
| `esm_dependencies.json` | 浏览器 ESM 依赖 | 检查 JSON；确认 import map | 必须编译并 reseed |
| `src/crates/contracts/product-domains/src/miniapp/loopx/**` | LoopX DTO、状态、端口、纯策略 | 集中完成修改，不跑 Cargo 预检查 | 联调前单次编译 binary |
| `src/crates/services/services-integrations/src/miniapp/loopx_*.rs` | GitHub、Git workspace、CLI 等宿主服务 | 集中完成修改，不跑 Cargo 预检查 | 联调前单次编译 binary |
| `src/crates/assembly/core/src/miniapp/loopx/**` | controller、Agent 编排、持久状态 | 集中完成修改，不跑 Cargo 预检查 | 联调前单次编译 binary |
| `scripts/build-loopx.mjs` 或 LoopX pin | 随包 sidecar | `pnpm run build:loopx` | 只在 sidecar/pin 变化时 |

这里的 `<本目录>` 是：

```text
src/crates/contracts/product-domains/src/miniapp/builtin/assets/bitfun-loopx
```

### 纯 UI 快速循环

内置 MiniApp 资源由 `include_str!` 嵌入 Desktop 二进制。当前没有一个既能热替换
source、又能继续通过受信任 built-in 校验的免编译入口。因此正确的快速循环是
“多次编辑，一次编译”，而不是每保存一次就启动 Desktop 构建。

1. 保持 Web UI Vite 常驻；没有运行时才启动：

   ```bash
   pnpm --dir src/web-ui dev
   ```

2. 连续修改 `index.html`、`style.css`、`ui.js`，先把一轮视觉交互做完整。
3. JS 变化只做秒级语法检查：

   ```bash
   node --check src/crates/contracts/product-domains/src/miniapp/builtin/assets/bitfun-loopx/ui.js
   ```

4. 需要在真实 MiniApp bridge 中验收时，停止正在运行的 `bitfun-desktop`，只构建
   一次 Desktop executable（配方见下方「统一构建配方」，不要换用别的环境变量）：

   ```bash
   cargo build -p bitfun-desktop --bin bitfun-desktop
   ```

5. 保持 Vite 不退出，直接启动刚生成的 executable：

   ```text
   target/debug/bitfun-desktop.exe
   ```

6. 新二进制启动后会根据 built-in 内容哈希自动 reseed，并重新生成
   `compiled.html`。此时再进入 LoopX MiniApp 验收。

快速循环中不要使用 `pnpm run desktop:dev` 反复启停。该入口会执行资源准备、
Cargo watch 和 target GC；在 Windows 上可能导致本来可增量的 UI 修改退化为大范围
冷编译。它适合需要持续修改 Desktop Rust 的完整开发会话，不适合只调 MiniApp CSS。

### 为什么不能直接改 AppData

不要把下面的运行目录当成源码：

```text
%APPDATA%/bitfun/data/miniapps/builtin-bitfun-loopx/source/**
%APPDATA%/bitfun/data/miniapps/builtin-bitfun-loopx/compiled.html
```

- `compiled.html` 是生成物，刷新、recompile 或重启后会被覆盖；
- `source/**` 必须与二进制内嵌的 built-in source 一致；直接修改会使
  `builtin_source_matches` 失败，受信任的 LoopX controller bridge 将被禁用；
- `miniapp_sync_from_fs` 适用于普通 MiniApp 或明确的本地定制流程，不能作为受信任
  built-in LoopX 的临时热补丁；
- 只改运行目录会造成“代码已经改了，但界面仍旧”或“界面变了，但 bridge 不可用”。

唯一权威源码始终是本 README 所在目录。

### 宿主行为修改

GitHub 认证/限流、Git workspace、环境预检、Agent 会话、任务恢复和持久状态不是
纯 MiniApp UI。这些改动需要修改对应 Rust owner。快速真机反馈不跑前置
`cargo check`；最终 binary build 会完成同一份 Rust 编译验证并直接产生可试用结果。
不要同时运行多个 Cargo 命令，也不要在 `desktop:dev` 正在自动编译时再手动启动
Cargo；它们会争用同一个 target 锁。

完成所有宿主改动后再执行一次：

```bash
cargo build -p bitfun-desktop --bin bitfun-desktop
```

### 统一构建配方（重要）

`bitfun-desktop` 的默认 features 为空，`--no-default-features` 没有实际差异；真正
影响全量/增量的是 **profile 环境变量**。Cargo 指纹包含 `CARGO_PROFILE_DEV_*` 设置，
一旦与上一次构建不同，整棵依赖树都会重编。请始终使用与
`scripts/dev.cjs`（`desktop-preview` 快速重建）**完全相同**的配方：

```powershell
$env:CARGO_PROFILE_DEV_DEBUG = "0"
$env:CARGO_PROFILE_DEV_INCREMENTAL = "true"
$env:CARGO_PROFILE_DEV_CODEGEN_UNITS = "256"
cargo build -p bitfun-desktop --bin bitfun-desktop
```

或单行（cmd）：

```bat
set CARGO_PROFILE_DEV_DEBUG=0&& set CARGO_PROFILE_DEV_INCREMENTAL=true&& set CARGO_PROFILE_DEV_CODEGEN_UNITS=256&& cargo build -p bitfun-desktop --bin bitfun-desktop
```

- **不要**混用不同的 `CARGO_PROFILE_DEV_*` / `CODEGEN_UNITS` 配置，也不要与
  `--no-default-features` 的裸命令交替使用——那会把本可增量的重编退化成全量冷编译；
- 快速反馈不要在 build 前追加 `cargo check`、测试或 Web UI type-check。指定
  `--bin bitfun-desktop` 明确请求 binary target；但当前同包 lib 同时声明
  `staticlib/cdylib/rlib`，Cargo 仍会生成并链接 `bitfun_desktop_lib.dll`。要去掉该阶段，
  需要把移动端/FFI wrapper 与 Desktop rlib 拆成独立 package/target；
- `src/web-ui/**` 由 Vite HMR 直接刷新，不需要 Rust build；内嵌 MiniApp source
  仍须一次 binary build 才能 reseed；
- 需要断点调试信息时（`CARGO_PROFILE_DEV_DEBUG=2`）、或需要同时持续改 Desktop
  Rust 时，改用 `pnpm run desktop:dev` 完整会话，不要在同一 target 上混跑；
- 装 sccache 可进一步让配方/feature 切换也不触发全量重编（可选优化）。

### 提交或发布前

开发期不需要 bump version，内容哈希变化会触发 reseed。发布时才做以下动作：

1. 同步增加 `builtin.rs` 中 `BUILTIN_APPS` 的 version 和 `meta.json` 的 version；
2. 运行聚焦 built-in 契约测试：

   ```bash
   cargo test -p bitfun-product-domains --features product-full builtin_miniapp
   ```

3. sidecar pin 或构建脚本有变化时运行：

   ```bash
   pnpm run build:loopx
   ```

4. 需要完整 Desktop 构建产物时使用仓库入口：

   ```bash
   pnpm run desktop:build:fast
   ```

规范依据：`MiniApp/Skills/miniapp-dev/SKILL.md` 的「内置小应用（builtin/assets/*）
维护规范」。

## 内置更新行为

- 内容哈希变化（无论 version 是否 bump）都会在下次启动时 reseed：用户机器上的
  源文件被覆盖为最新内置版本；
- 用户对源文件的**手动本地修改会被覆盖**，除非该应用处于"本地定制(local
  override)"状态——那时只记录"有可用更新"通知（可拒绝），不改动本地内容；
- `storage.json`（设置、GitHub Token、goal↔session 映射等）跨 reseed 始终保留。

## 权限与信任模型

本应用遵守 MiniApp V2 无 Node 规范：`meta.json` 明确设置 `node.enabled=false`，
`worker.js` 只有空兼容导出，UI 不执行 shell、文件或网络原语。普通和市场 MiniApp 的
`window.app` 公共 API 中没有 LoopX 控制器。

编译器只为 id 精确匹配 `builtin-bitfun-loopx` 的非 strict 构建注入私有
`app.loopx` namespace；Web UI 与 Desktop 在每次调用时继续校验 active scope、原始
built-in source、非本地覆盖和本地执行域。伪造 id、draft、市场包或修改后的内置源码
都不能取得该控制器。这个 namespace 是产品私有扩展，不得被其他 MiniApp 使用或模拟。

## 平台支持

- Desktop 安装包携带固定版本的 LoopX sidecar；开发态缺少资源时只允许使用版本完全
  匹配的系统 `loopx`，不从 MiniApp 内下载运行时或修改用户 Python 环境。
- Git workspace、GitHub intake、进程树与 sidecar 探测由 Rust service owner 实现，
  不进入 UI 或 Worker。
- 当前只支持本地 Desktop workspace。Remote Workspace、Peer Device、Remote Control
  和 Detached Dispatch 均返回明确 unsupported，不静默回退到控制端本机。

## 设计假设

- `LoopxController` 是进程级 host driver，不依赖 MiniApp iframe 生命周期；关闭或
  重开界面不会创建第二套心跳。UI 通过 cursor replay 恢复事件。
- Desktop 待机恢复由 MiniApp 的可见性/焦点/时钟间隙检测触发幂等 attach；可信私有
  attach 会让宿主刷新环境与非运行 Goal 投影，但保留仍可恢复的单一 Agent turn，避免
  伪造失败或重复启动。活动任务长时间没有事件时 UI 只做有界快照重取。
- 普通 UI attach 不重复 inspect `WaitingForUser`、终态或显式恢复态 Goal；这些状态由
  宿主审批/恢复动作直接推进，仅在真实待机恢复的 force reconciliation 中重新向 CLI
  对账。这样等待审批不会每 30 秒生成一个可能超时的 sidecar 进程。
- Issue 列表与实时日志是一级工作区；点击 Issue 打开全尺寸二级详情层，集中展示阶段、
  当前动作、有效产出、最新回合结论和原始 Issue 描述。审批门禁同时投影为持久顶部提示，
  新门禁首次出现时自动打开批准/拒绝对话框；提交决定后 UI 立即收起对话框，并以任务
  pending 状态等待宿主确认，不把 CLI 往返延迟表现成按钮卡死。
- `pending_gate_id/message/action_kind` 属于任务快照的持久投影，审批按钮不依赖可能被
  截断的历史事件回放。升级前的 `WaitingForUser` 记录若缺少该投影，普通 attach 只做
  一次 CLI reconciliation 补齐，之后重新进入等待态免轮询路径。
- 每轮 turn 由专用 `LoopxAgentPort` 通过通用 `AgentSubmissionPort` 启动临时 BitFun
  Agent session。通用 Agent loop 不包含 LoopX 分支；LoopX CLI 生成本轮 prompt 和
  writeback contract。
- LoopX registry 是 goal/todo/gate/quota/settlement 的唯一权威。BitFun 持久化的
  `LoopxTaskState` 只描述 workspace、session、取消和恢复等 host-job 生命周期，并保存
  最近一次只读 `goalState` 投影；启动环境检查和 UI attach 都会向 CLI 对账。
- Agent 负责验证真实结果并通过 LoopX CLI 写入 typed durable progress；宿主不从对话文本
  或进程退出码推断进展。只有读到与 goal、agent、Turn 和 todo/autonomous-replan binding
  全部匹配的 durable writeback 后，宿主才按该 identity 幂等追加并读回 quota receipt。
  完全缺失 durable writeback 的 turn 进入显式 recovery；宿主不额外启动
  settlement-repair Agent turn，也不伪造 progress。
- 任务快照在结算前保存有界的最后 Agent 回合总结，供详情页展示分析、方案、产出和
  下一步；该总结只是 UX 投影，不参与 Goal 状态、durable progress 或 settlement 判定。
  Subscriber 只聚合最后一个模型 round 的最新 attempt，防止中间工具回合或重试文本
  混入最终总结。
- **目标模型：一个 goal 只对应一个 issue/PR**（`goal_id_for` 生成
  `bfx-owner-repo-issue-N`，重试追加 `-attempt` 后缀），每个 item 有独立
  worktree 与 `.loopx/registry.json`；todo 是 **goal 内部** 的推进项/审批门禁
  （`todo add --goal-id` 强绑定单一 goal），不用一串 todo 把多个 issue 串在
  一个 goal 下。依据：loopx v0.5.x 的 goal 是「单一 objective 的持续 turn 载体」，
  quota/心跳/审批/结算都以 goal 为域，registry 本身支持多 goal 列表——
  多 issue 的"批量管理"由本应用 task/batch 层聚合，不压平到 loopx goal。
- **心跳调度**：本应用维护一个统一的 task 调度循环（非每 issue 一个独立
  定时器）；每轮 inspect_goal 由 loopx 返回 `scheduler_hint_ms`（含指数退避），
  宿主据此把该 goal 重新排队；同一仓库的多个 goal 串行推进（
  `active_repositories` + `schedule_next_for_repository`）。每次 durable settlement
  是公平轮转边界：有其他排队 Issue 时先让出仓库槽，不把当前 task 标记为 pending
  并在同一 worker 内自重入；轮到其他 Issue 结算后再回到当前 Goal。
- **worktree 成本**：同仓库所有 task 共享一份裸仓库对象库
  （`<root>/<repo-hash>/bare.git/`，首个 task `git clone --bare` 建立），每个
  task 用 `git worktree add -b bitfun-loopx/<task-hash>` 挂出独立工作区
  （`<root>/<repo-hash>/<task-hash>/`）。磁盘 ≈ 1 份对象库 + 各 task 的检出
  文件；历史版本升级前创建的旧式独立克隆（每 task 一份完整 `.git`）仍可
  正常复用，不强制迁移。归档会立即释放单个任务的磁盘：dispose 先
  `git worktree remove --force` 删除该 task 工作区，再按
  `git worktree list` 剩余条目数判断是否删除共享裸仓库（最后一个 worktree
  离开时整仓回收）。loopx 上游只 `connect` 已存在项目、不 clone，克隆策略
  是宿主侧职责，改动需同步 `loopx_workspace.rs` 与克隆契约测试。
- **依赖与构建缓存边界**：包管理器下载缓存（例如 Yarn Berry 全局 cache）和 Git 对象库
  可以跨 task 复用；`node_modules`、构建目录和测试进程属于可变工作树状态，不在并行
  Issue 间直接共享，避免一个 Issue 的安装脚本或平台产物污染另一个 Issue。LoopX 专用
  Agent 会被告知当前目录已是独立 worktree、先检查已有依赖状态、只在工具明确返回
  `session_id` 时使用交互式续写，并对安装、打包和烟测设置有界等待。
- **全量清空**：重置会先把整个旧 workspace root 原子改名隔离，立即创建新的
  活动 root，并搬回经过目录边界验证的 `bare.git` 对象缓存；旧 task Worktree
  在后台递归回收。这样不会复用脏工作树或失去 owner 的未结算修改，但下一次同仓库
  任务不必重新下载完整 Git 历史。要继续已有修改应使用仓库级恢复，不应先重置。
- **自主回合收敛门禁**：宿主只在 LoopX 已返回 `RunNow`、没有自然 user gate、Goal
  也未终结时计算预算；每 4 个成功 durable settlement 必须通过 typed CLI port 创建
  一个 agent-scoped `user_gate`。批准后重新获得 4 个有界回合，拒绝则停止 BitFun host
  task 并保留 worktree/证据，后续仍可用仓库级恢复继续。预算按结算回执计算，不按工具
  字符串、命令次数或模型文本猜测；升级后的旧任务也会用 LoopX authoritative receipt
  history 补算未审阅回合，因此不会把通用 Agent loop 变成硬编码工作流。
- 模型成本仍由用户在 BitFun 侧管理；自主回合门禁是收敛/控制权边界，不是费用估算器。

## loopx 依赖与合规

- **内置编译二进制（随安装包分发）**：打包流程（`scripts/desktop-tauri-build.mjs`，
  即 `pnpm run desktop:build*` 的 bundle 路径）会先执行 `scripts/build-loopx.mjs`：
  构建期拉取 pin `v0.5.1` 的 loopx 源码并用 PyInstaller 编译单文件二进制，随
  tauri `bundle.resources` 作为 sidecar 分发（`resources/loopx/`）。桌面宿主把
  资源目录由 Desktop 启动 wiring 传给 `LoopxCliProcessAdapter`，探测时内置二进制
  优先，用户机器零依赖。开发态资源缺失时只接受版本完全匹配的系统命令。
  生成的 `resources/loopx/` 目录在 `.gitignore` 中，二进制不进仓库；`manifest.json`
  记录版本、commit、内容哈希与构建工具链。
- **MIT 再分发义务**：loopx 为 MIT（Copyright (c) 2026 LoopX contributors），
  允许以二进制形式再分发，义务为保留许可证与版权声明——`resources/loopx/` 随包
  携带上游 `LICENSE` 与 `TRADEMARKS.md`，`THIRD_PARTY_NOTICES.md` 已收录 loopx
  条目。名称按 loopx [TRADEMARKS.md](https://github.com/huangruiteng/loopx/blob/main/TRADEMARKS.md)
  描述性使用，本应用是第三方集成，非 LoopX 官方出品。
- **运行期兜底**：内置二进制缺失时，开发态可回退版本完全匹配的系统 `loopx`；
  版本不一致直接拒绝。升级必须伴随 CLI JSON 契约的显式适配。
- 本应用自身的 GitHub 凭据只存于本机应用存储（gh CLI 或粘贴的 PAT），不写入 git config。

## loopx 依赖升级

loopx 不随包分发源码，只 pin 一个经过验证的版本。**不要自动追新**：只有在确实
需要新版能力/修复时才升级。

1. **读 release notes / CHANGELOG**，重点确认 CLI 的 `--format json` 输出契约
   （`turn plan` / `heartbeat-prompt` / `history` / `todo` / `bootstrap`）没有破坏性变更；
2. **改 pin**：同步修改 `scripts/build-loopx.mjs` 与 `loopx_cli.rs` 的版本常量；
3. **本机冒烟**：重建 sidecar 后跑一个 issue 全流程（intake → turn → todo 审批 →
   receipt 观察）验证 JSON 契约；
4. **回滚**：恢复上述两个 pin 并重建 sidecar；已持久化 host job 和 LoopX registry
   均保留，不删除用户工作区；
5. **随发布提交**：pin 变更与 `version` bump 一起进发布版（发布版自带的
   sidecar 二进制由打包构建重新编译）。

**升级经验（首批开发踩坑记录）**：

- `heartbeat-prompt` 中的 `${LOOPX_TURN:?}` 必须由 adapter 替换成宿主签发的稳定
  Turn id，不能让 Agent 自行生成；
- sidecar JSON stdout 与进程 stderr 必须分流，stdout 只接受一个结构化文档；
- 不能通过自然语言或进程退出码推断结算成功，只接受 LoopX history/receipt 中与
  goal、agent、todo 和 Turn id 全部匹配的持久证据。
