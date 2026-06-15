# BitFun Core 拆解与运行时迁移计划

本文只维护后续执行计划。稳定目标以
[`core-decomposition.md`](../architecture/core-decomposition.md) 和
[`agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md)
为准；已完成事实归档在
[`core-decomposition-completed.md`](core-decomposition-completed.md)。

## 1. 执行原则

- `bitfun-core` 最终收敛为 compatibility facade、Product Assembly 边界和少量迁移期 adapter。
- `src/crates` 保持六层物理布局：`interfaces -> assembly -> adapters -> services -> execution -> contracts`，依赖只能自上而下。
- 新抽象必须同步删除、迁移或显著简化旧 core 主体路径；纯 facade、纯 guard、纯文档或空接口不算完成。
- 设计文档默认保持稳定。阶段状态、剩余工作和风险只写入本计划、completed 归档和 issue。
- 任何会改变权限、工具 schema、事件语义、session 生命周期、remote 行为、MiniApp 行为或交付形态的变更必须暂停并单独评审。

## 2. 当前基线

- workspace 已按六层目录展开，旧 `surfaces` / `providers` 目标层级不再使用。
- `bitfun-core --no-default-features` 已裁掉 workspace-search owner、debug ingest HTTP server、AI provider adapter runtime 和 direct `reqwest`。
- Desktop / CLI / ACP 仍通过 `bitfun-core/product-full` 获取完整能力；Server / Web / Mobile Web 不直接依赖 core，但交付形态级 feature / dependency trimming 仍未闭环。
- Runtime Services、Agent Runtime、Tool Contracts、Tool Execution、Harness、Product Domains、Services Core、Services Integrations 等 owner crate 已建立，部分逻辑仍由 core concrete manager 或产品命令路径持有。
- 本轮 PR-B 收口 Agent lifecycle 与 tool side-effect owner：turn skill/agent snapshot DTO / diff / render / store、file-read session state、session evidence ledger 与 compression-contract projection、dialog-turn cancellation token store、tool confirmation / user-question wait channel state 已迁入 `agent-runtime`；background exec output capture、tool cancellation token store 已迁入 `tool-execution`；core 保留 resolver、产品事件、具体工具执行、IO 编排和旧路径兼容 re-export。

## 3. 已完成但仍需保持的边界

- `services-core` 已承接 session metadata store、session index rebuild、lineage / branch metadata shaping、JSON file store、session layout 和 legacy session-store merge。
- `runtime-services` 已承接 typed runtime service assembly、capability validation、provider registry、backend event delivery owner。
- `agent-runtime` 已承接 provider-neutral scheduler decisions、dialog lifecycle port contracts、background delivery decisions、thread-goal facts、prompt-cache facts、turn skill/agent snapshot state、file-read session state、session evidence ledger、dialog-turn cancellation token store、tool confirmation / user-question wait channel state、DeepReview provider-neutral policy / queue / retry / diagnostics shaping。
- `tool-contracts` / `tool-execution` 已承接 tool manifest / catalog / admission、batching plan、retry policy、state counting、cancellation-state/token-store policy、background exec output capture、shell helper 和部分 local / remote IO helper。
- `services-integrations` 已承接 remote-connect primitives、workspace search concrete owner、remote SSH/SFTP/PTY owner、MiniApp host dispatch / storage / worker IO、DeepResearch report IO。
- `product-domains` 已承接 MiniApp workflow planning、compile / permission path adaptation、function-agent prompt / parser / response policy 和部分 Git snapshot/fallback 逻辑。
- boundary scripts 已覆盖核心 owner 防回流、six-layer path 解析、facade-only 文件和重点 feature gate。

## 4. 剩余大块 PR

| PR | 目标 | 主要范围 | 准出标准 |
|---|---|---|---|
| PR-B | Agent lifecycle 与 tool side-effect closure | session state / turn state、dialog-turn cancellation token store、tool confirmation / user-question channel state、tool cancellation token store、terminal / exec output capture、core 兼容 facade 与防回流 boundary；concrete scheduler 和 prompt-cache persistence IO 保持在 core，待 SDK / product-shape 阶段通过 port 设计整体收口 | 保持 Flow Chat finalize / retry / round history、session metadata、cancel、non-TTY exec output、tool event / card 语义；core 旧路径显著简化；补 focused tests 和 boundary |
| PR-C | Harness 与 product workflow concrete migration | DeepReview launch / provider wait / report persistence、DeepResearch concrete workflow、MiniApp AI acquisition / larger workflow execution、function-agent AI provider acquisition | 保持 DeepReview queue/report、MiniApp create/update/import/storage/PPT live、AI request trace、function-agent Git/AI 行为；core 只保留 legacy facade / adapter |
| PR-D | Product shape / Agent SDK / core facade closure | 内部 Agent Runtime SDK façade、fake provider 最小 session / turn / event stream、Product Assembly capability matrix、delivery profile feature trimming、`bitfun-core` facade 收口 | cargo metadata / cargo tree 有 no-default/product-full 对比；各产品入口验证通过；SDK 不暴露 `bitfun-core`、product-full、concrete manager 或全局 mutable state |

## 5. 固定执行流程

1. 同步最新 `main`，检查主干新增的 CLI、tool、terminal、session、scheduler、remote、MiniApp、ACP 或 product interface 变更。
2. 对照设计文档和 Issue #970 明确本次 owner 边界，不从旧计划标签继承完成判断。
3. 先补等价保护，再迁移实现主体。
4. 删除、迁移或显著简化 core 中对应旧路径。
5. 运行 focused verification、boundary check 和必要的 feature / dependency 对比。
6. 从独立第三方角度审查功能漂移、性能劣化、依赖回流、产品形态遗漏和文档一致性。
7. 合入后只更新 completed 摘要和 issue 状态；设计文档只有目标架构变更时才修改。

## 6. 验证矩阵

| 触达范围 | 最小验证 |
|---|---|
| docs / boundary / layout | `pnpm run check:repo-hygiene`，`node --test scripts/check-core-boundaries.test.mjs`，`node scripts/check-core-boundaries.mjs` |
| Workspace layout / Cargo path | `cargo metadata --no-deps --format-version 1` |
| Runtime Services / backend events | `cargo test -p bitfun-runtime-services`，`cargo check -p bitfun-core --no-default-features` |
| Services Core session migration | `cargo test -p bitfun-services-core merge_legacy_session_store`，core workspace-runtime focused tests |
| Agent lifecycle / scheduler | `cargo test -p bitfun-agent-runtime`，core scheduler / session focused tests |
| Tool / terminal | `cargo test -p bitfun-agent-tools`，`cargo test -p tool-runtime`，terminal / exec-command focused tests |
| Harness / Product Domains | `cargo test -p bitfun-harness`，`cargo test -p bitfun-product-domains`，DeepReview / MiniApp focused tests |
| Product shape / SDK | `cargo test -p bitfun-agent-runtime`，`cargo test -p bitfun-runtime-services`，SDK fake-provider smoke，cargo tree / metadata 对比 |
| 大范围 owner 迁移 | `cargo check --workspace`，必要时补 `cargo test --workspace` |

## 7. 暂停条件

- 新 owner crate 必须依赖回 `bitfun-core` 才能编译或测试。
- Execution / contracts crate 吸收 Tauri、产品命令、AI provider、MCP client、process execution、Git provider 等 concrete dependency。
- Product Assembly 变成无类型 service locator 或全局 mutable app state。
- PR 只新增抽象，没有迁移、删除或显著简化旧 core 主体路径。
- SDK façade 必须暴露 `bitfun-core`、`product-full`、concrete service manager 或产品命令 registry 才能完成基本 agent 执行。
