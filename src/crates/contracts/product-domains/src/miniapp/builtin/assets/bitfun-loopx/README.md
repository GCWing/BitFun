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
| `worker.js` | MiniApp Worker、LoopX CLI 调用 | `node --check <本目录>/worker.js` | 真机验证前编译一次并重启 Worker |
| `meta.json` | 权限、版本、运行方式 | 检查 JSON 和权限差异 | 必须编译并 reseed |
| `esm_dependencies.json` | 浏览器 ESM 依赖 | 检查 JSON；确认 import map | 必须编译并 reseed |
| `src/crates/contracts/product-domains/src/miniapp/loopx/**` | LoopX DTO、状态、端口、纯策略 | `cargo check -p bitfun-product-domains --no-default-features --features miniapp` | 联调前编译一次 |
| `src/crates/services/services-integrations/src/miniapp/loopx_*.rs` | GitHub、Git workspace、CLI 等宿主服务 | `cargo check -p bitfun-services-integrations --no-default-features --features miniapp-loopx` | 联调前编译一次 |
| `src/crates/assembly/core/src/miniapp/loopx/**` | controller、Agent 编排、持久状态 | `cargo check -p bitfun-core --no-default-features --features tools-miniapp` | 联调前编译一次 |
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
   cargo build -p bitfun-desktop
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
纯 MiniApp UI。这些改动需要修改对应 Rust owner，并先运行上表中的一个最窄
`cargo check`。不要同时运行多个 Cargo 命令，也不要在 `desktop:dev` 正在自动编译时
再手动启动 Cargo；它们会争用同一个 target 锁。

完成所有宿主改动后再执行一次：

```bash
cargo build -p bitfun-desktop
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
cargo build -p bitfun-desktop
```

或单行（cmd）：

```bat
set CARGO_PROFILE_DEV_DEBUG=0&& set CARGO_PROFILE_DEV_INCREMENTAL=true&& set CARGO_PROFILE_DEV_CODEGEN_UNITS=256&& cargo build -p bitfun-desktop
```

- **不要**混用不同的 `CARGO_PROFILE_DEV_*` / `CODEGEN_UNITS` 配置，也不要与
  `--no-default-features` 的裸命令交替使用——那会把本可增量的重编退化成全量冷编译；
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

`meta.json` 的 `permissions` 约束的是宿主桥接原语（`app.shell.exec` /
`app.net.fetch` / `app.fs.*` 等）。本应用的 worker 是**受信内置代码**，直接使用
Node 的 `child_process` / `fs` / `https`：因此它实际会 spawn 的 `gh` / `winget` /
`powershell` / `curl` / `unzip` / `tar` / `taskkill` 等二进制，以及写入
`~/.bitfun/bitfun-loopx/**` 的行为，不需要（也不会）出现在上述 allowlist 中。
这是内置应用的特权模型；`node.enabled = true` 的应用本来也无法上架市场。

## 平台支持

- loopx 获取三平台通用，优先级：① 安装包内置的编译二进制（见下节「loopx 依赖
  与合规」，宿主经 `BITFUN_RESOURCE_DIR` 传入，无需 Python/git/网络）→ ② 一键拉
  源码到 `~/.bitfun/bitfun-loopx/vendor/loopx`（`python -m loopx.cli` /
  `python3 -m loopx.cli` 直跑）→ ③ pip 安装兜底。
- gh 获取：Windows 走 winget，失败回退 PowerShell 下载 release zip；macOS 走
  Homebrew，失败回退 curl + unzip 下载 release zip；Linux 无通用包管理器，直接
  curl + tar 下载 release tar.gz 到 `~/.bitfun/bitfun-loopx/bin/gh`。
- `gh auth login` 交互窗口：Windows `cmd start`；macOS Terminal（osascript）；
  Linux 依次尝试 `x-terminal-emulator` / `gnome-terminal` / `konsole` / `xterm`，
  全部缺失时返回明确的手动指引，不做静默降级。

## 设计假设

- 单实例桌面使用：心跳与 run-once 注册表都是进程内状态，同时开两个 BitFun
  窗口打开本应用会双心跳、可能重复启动 turn。远程控制（手机/web 遥控）下的
  审批弹窗触达亦未验证。v1 明确接受该假设。
- 每轮 turn 由宿主 Agent 执行（`app.agent.run`），loopx 只负责心跳/配额/计划
  与 todo 状态；本应用不在本地托管任何外部 CLI 宿主（codex 等）。
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
  `active_repositories` + `schedule_next_for_repository`）。
- **worktree 成本**：同仓库所有 task 共享一份裸仓库对象库
  （`<root>/<repo-hash>/bare.git/`，首个 task `git clone --bare` 建立），每个
  task 用 `git worktree add -b bitfun-loopx/<task-hash>` 挂出独立工作区
  （`<root>/<repo-hash>/<task-hash>/`）。磁盘 ≈ 1 份对象库 + 各 task 的检出
  文件；历史版本升级前创建的旧式独立克隆（每 task 一份完整 `.git`）仍可
  正常复用，不强制迁移。空间不足时**归档（Archive）是唯一释放磁盘的入口**：
  dispose 先 `git worktree remove --force` 删除该 task 工作区，再按
  `git worktree list` 剩余条目数判断是否删除共享裸仓库（最后一个 worktree
  离开时整仓回收）。loopx 上游只 `connect` 已存在项目、不 clone，克隆策略
  是宿主侧职责，改动需同步 `loopx_workspace.rs` 与克隆契约测试。
- v1 不设 turn 预算/成本上限：心跳节奏由 loopx 配额控制，宿主 Agent 消耗的
  模型额度由用户在 BitFun 侧自行管理。

## loopx 依赖与合规

- **内置编译二进制（随安装包分发）**：打包流程（`scripts/desktop-tauri-build.mjs`，
  即 `pnpm run desktop:build*` 的 bundle 路径）会先执行 `scripts/build-loopx.mjs`：
  构建期拉取 pin `v0.5.1` 的 loopx 源码并用 PyInstaller 编译单文件二进制，随
  tauri `bundle.resources` 作为 sidecar 分发（`resources/loopx/`）。桌面宿主把
  资源目录经 `BITFUN_RESOURCE_DIR` 传给 MiniApp worker，探测时内置二进制优先，
  用户机器零依赖。`--no-bundle` 的 dev 构建不触发该步骤，走 vendor/pip 兜底。
  生成的 `resources/loopx/` 目录在 `.gitignore` 中，二进制不进仓库；`manifest.json`
  记录版本、commit、内容哈希与构建工具链。
- **MIT 再分发义务**：loopx 为 MIT（Copyright (c) 2026 LoopX contributors），
  允许以二进制形式再分发，义务为保留许可证与版权声明——`resources/loopx/` 随包
  携带上游 `LICENSE` 与 `TRADEMARKS.md`，`THIRD_PARTY_NOTICES.md` 已收录 loopx
  条目。名称按 loopx [TRADEMARKS.md](https://github.com/huangruiteng/loopx/blob/main/TRADEMARKS.md)
  描述性使用，本应用是第三方集成，非 LoopX 官方出品。
- **运行期兜底**：内置二进制缺失/损坏时回退 vendor 源码（`~/.bitfun/bitfun-loopx/vendor/loopx`）
  或 pip 安装；pin 保持一致（`LOOPX_VENDOR_REF` = `v0.5.1`），升级必须伴随
  loopx CLI JSON 契约的显式适配。
- 本应用自身的 GitHub 凭据只存于本机应用存储（gh CLI 或粘贴的 PAT），不写入 git config。

## loopx 依赖升级

loopx 不随包分发源码，只 pin 一个经过验证的版本。**不要自动追新**：只有在确实
需要新版能力/修复时才升级。

1. **读 release notes / CHANGELOG**，重点确认 CLI 的 `--format json` 输出契约
   （`quota should-run` / `heartbeat-prompt` / `todo` / `status` / `bootstrap`
   等被 worker 解析的结构）没有破坏性变更；
2. **改 pin**：`worker.js` 的 `LOOPX_VENDOR_REF`（sidecar 构建 `scripts/build-loopx.mjs`
   与 vendor 兜底共用同一个 pin）；
3. **本机冒烟**：删除 `~/.bitfun/bitfun-loopx/vendor/loopx` 后走一次「拉取 loopx
   源码」重 pin，再跑一个 issue 全流程（intake → 心跳 → turn → todo 审批 →
   PR 发布）验证 JSON 契约；
4. **回滚**：出问题只需把 `LOOPX_VENDOR_REF` 改回旧 tag；vendor 目录在下次
   ensureVendor 自动重新 pin，用户无需手动修复；
5. **随发布提交**：pin 变更与 `version` bump 一起进发布版（发布版自带的
   sidecar 二进制由打包构建重新编译）。

**升级经验（首批开发踩坑记录）**：

- loopx 示例命令里的 `${LOOPX_TURN:?}` 是 bash 语法，Windows 的 PowerShell
  下会原样报错——agent 提示词里明确"这是可选参数，直接省略"；
- 中文 Windows 下 Python CLI 的 stdout 默认 GBK，会破坏 `JSON.parse`——worker
  统一注入 `PYTHONUTF8=1` + `PYTHONIOENCODING=utf-8`；
- `python -m loopx` 不存在（loopx 没有 `__main__.py`），模块形式必须指向
  `loopx.cli`；
- 首次运行时 `quota` 的 public-boundary 扫描以 loopx 源码目录为根，冷缓存下
  单次扫描可超一分钟——控制台把扫描根指向专用的空目录保证心跳节奏。
