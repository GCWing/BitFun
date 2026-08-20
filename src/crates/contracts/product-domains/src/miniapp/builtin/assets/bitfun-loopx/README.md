# bitfun-loopx（内置 MiniApp）

内置版 **bitfun-loopx**：粘贴 GitHub Issue 链接，由 BitFun 宿主 Agent 驱动本机
LoopX 引擎持续修复，心跳调度、人工审批、中途插话。

本目录就是该内置 MiniApp 的**唯一权威源码**，不依赖任何外部仓库快照。五件套
`index.html` / `style.css` / `ui.js` / `worker.js` / `esm_dependencies.json` 由
`src/crates/contracts/product-domains/src/miniapp/builtin.rs` 的 `BUILTIN_APPS`
以 `include_str!` 嵌入二进制，注册 id 为 `builtin-bitfun-loopx`（同文件契约测试
含 id 顺序断言）；`meta.json` 的权限与注册条目保持一致。

## 修改流程

直接编辑本目录文件（`index.html` / `style.css` / `ui.js` / `worker.js` /
`meta.json`）：

1. **开发期无需 bump version**：种子机制按内容哈希判定，重启应用即自动 reseed
   （跳过条件 = 内容哈希一致 且 已装 version ≥ 内置 version；内容一变哈希必变）。
2. **发布期（每个发布版）**：把 `builtin.rs` 的 `BUILTIN_APPS` version 与
   `meta.json` 的 `version` **同步 +1**。version 只是单调计数，唯一作用是给
   "本地定制"用户发更新通知，不代表内容新旧。
3. 本地验证 reseed：重启应用（或删除
   `~/.bitfun/miniapps/builtin-bitfun-loopx/.builtin-manifest.json` 后重启），
   确认新资源落盘；
4. `cargo test -p bitfun-product-domains --features product-full builtin_miniapp` 全绿后提交。

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
- v1 不设 turn 预算/成本上限：心跳节奏由 loopx 配额控制，宿主 Agent 消耗的
  模型额度由用户在 BitFun 侧自行管理。

## loopx 依赖与合规

- **内置编译二进制（随安装包分发）**：打包流程（`scripts/desktop-tauri-build.mjs`，
  即 `pnpm run desktop:build*` 的 bundle 路径）会先执行 `scripts/build-loopx.mjs`：
  构建期拉取 pin `v0.2.13` 的 loopx 源码并用 PyInstaller 编译单文件二进制，随
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
  或 pip 安装；pin 保持一致（`LOOPX_VENDOR_REF` = `v0.2.13`），升级必须伴随
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
