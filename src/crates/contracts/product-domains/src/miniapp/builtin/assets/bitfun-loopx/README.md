# bitfun-loopx（内置 MiniApp）

内置版 **bitfun-loopx**：粘贴 GitHub Issue 链接，由 BitFun 宿主 Agent 驱动本机
LoopX 引擎持续修复，心跳调度、人工审批、中途插话。

本目录就是该内置 MiniApp 的**唯一权威源码**，不依赖任何外部仓库快照。五件套
`index.html` / `style.css` / `ui.js` / `worker.js` / `esm_dependencies.json` 由
`src/crates/contracts/product-domains/src/miniapp/builtin.rs` 的 `BUILTIN_APPS`
以 `include_str!` 嵌入二进制，注册 id 为 `builtin-bitfun-loopx`（同文件契约测试
含 id 顺序断言）；`meta.json` 的权限与注册条目保持一致。

## 修改流程

直接编辑本目录文件。每次修改 `index.html` / `style.css` / `ui.js` / `worker.js` /
`meta.json` 任一文件后：

1. 把 `builtin.rs` 中 `BUILTIN_APPS` 对应条目的 `version: N` → `N + 1`
   （种子机制靠 version + 内容哈希判定更新；用户侧 `storage.json` 跨版本保留，
   `meta.json` 里的 `version` 仅用于展示，不驱动 reseed）；
2. 本地验证 reseed：删除 `~/.bitfun/miniapps/builtin-bitfun-loopx/.builtin-manifest.json`
   （或整个目录）后重启应用，确认新资源落盘；
3. `cargo test -p bitfun-product-domains --features product-full builtin_miniapp` 全绿后提交。

version 只是一个单调计数，没有任何语义，高频迭代时每次都 +1 即可，不必纠结
数值。README 等未嵌入的文档改动无需 bump version。

规范依据：`MiniApp/Skills/miniapp-dev/SKILL.md` 的「内置小应用（builtin/assets/*）
维护规范」。

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
