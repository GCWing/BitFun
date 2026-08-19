# bitfun-loopx (built-in bundle)

内置版 **LoopX 控制台**（品牌 bitfun-loopx）：粘贴 GitHub Issue 链接，由 BitFun 宿主
Agent 驱动本机 loopx CLI 持续修复，心跳调度、人工审批、中途插话。

## 上游来源

- 独立仓库：[xielixing/loopx-console](https://github.com/xielixing/loopx-console)（MIT）
- 本目录是其 `source/` 的快照，五件套由 `BUILTIN_APPS` 以 `include_str!` 嵌入二进制：
  `index.html` / `style.css` / `ui.js` / `worker.js` / `esm_dependencies.json`。
- 注册点：`src/crates/contracts/product-domains/src/miniapp/builtin.rs` 的
  `BUILTIN_APPS`（id=`builtin-bitfun-loopx`）与同文件契约测试的 id 顺序断言。

## 同步流程（上游仓库更新后）

1. 从 loopx-console 仓库拷贝 `source/` 五件套覆盖本目录同名文件（`meta.json` 权限变更
   需同步到本目录 `meta.json`，id 保持 `builtin-bitfun-loopx`）；
2. 在 `BUILTIN_APPS` 里把 `version` +1（种子机制靠 version + 内容哈希判定更新；
   用户的 `storage.json` 跨版本保留）；
3. `cargo test -p bitfun-product-domains builtin_miniapp` 全绿后提交。

## loopx 依赖与合规

- loopx 不随包分发：运行时由应用自己获取（已装 CLI → 一键拉源码到
  `~/.bitfun/loopx-console/vendor/loopx`，固定 pin `v0.2.13`，`python -m loopx.cli`
  直跑 → pip 指引兜底）。因此本仓库不包含 loopx 源码，不触发其 MIT 再分发义务。
- loopx 为 MIT（Copyright (c) 2026 LoopX contributors）；名称按 loopx
  [TRADEMARKS.md](https://github.com/huangruiteng/loopx/blob/main/TRADEMARKS.md)
  描述性使用，本应用是第三方集成，非 LoopX 官方出品。
- 本应用自身的 GitHub 凭据只存于本机应用存储（gh CLI 或粘贴的 PAT），不写入 git config。
