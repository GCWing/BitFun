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
- Desktop / CLI / ACP 仍通过 `bitfun-core/product-full` 获取完整能力；Server / Remote / Web / Mobile Web 不直接依赖 core。Product Assembly 已按入口矩阵裁剪能力计划：完整兼容入口保留 product-full 能力，无直接 core 入口不再 materialize product-full capability packs、feature groups、runtime services、tool groups 或 harness routes。
- Runtime Services、Agent Runtime、Tool Contracts、Tool Execution、Harness、Product Domains、Services Core、Services Integrations 等 owner crate 已建立；Agent Runtime SDK 内部 facade 已能注入 runtime services、tool registry、harness registry、hook registry 和 workspace-scoped agent registry，部分 concrete 生命周期仍由 core concrete manager 或产品命令路径持有。
- PR-B 已收口 Agent lifecycle 与 tool side-effect owner：turn skill/agent snapshot DTO / diff / render / store、file-read session state、session evidence ledger 与 compression-contract projection、dialog-turn cancellation token store、tool confirmation / user-question wait channel state 已迁入 `agent-runtime`；background exec output capture、tool cancellation token store 已迁入 `tool-execution`；core 保留 resolver、产品事件、具体工具执行、IO 编排和旧路径兼容 re-export。
- PR-C 已收口 Harness / product workflow 的低风险 owner：MiniApp AI / Agent permission、rate-limit、model/message/session/workspace/turn-text 规则迁入 `product-domains`；DeepResearch 后处理 gate 迁入 `agent-runtime`，report IO 继续由 `services-integrations` 持有；function-agent AI concrete acquisition 收拢为 core port adapter，旧 `runtime_services` 路径删除。

## 3. 已完成但仍需保持的边界

- `services-core` 已承接 session metadata store、session index rebuild、lineage / branch metadata shaping、JSON file store、session layout 和 legacy session-store merge。
- `runtime-services` 已承接 typed runtime service assembly、capability validation、provider registry、backend event delivery owner 和无副作用 capability marker ports。
- `agent-runtime` 已承接 provider-neutral scheduler decisions、dialog lifecycle port contracts、background delivery decisions、thread-goal facts、prompt-cache facts 与持久化写入决策、turn skill/agent snapshot state、file-read session state、session evidence ledger、dialog-turn cancellation token store、tool confirmation / user-question wait channel state、DeepReview provider-neutral policy / queue / retry / diagnostics shaping 与 queue event payload shaping。
- `tool-contracts` / `tool-execution` 已承接 tool manifest / catalog / admission、batching plan、retry policy、state counting、cancellation-state/token-store policy、background exec output capture、shell helper 和部分 local / remote IO helper。
- `services-integrations` 已承接 remote-connect primitives、workspace search concrete owner、remote SSH/SFTP/PTY owner、MiniApp host dispatch / storage / worker IO、DeepResearch report IO。
- `product-domains` 已承接 MiniApp workflow planning、compile / permission path adaptation、function-agent prompt / parser / response policy 和部分 Git snapshot/fallback 逻辑。
- Product Assembly 已承接当前 delivery profile 的能力计划裁剪；下层 owner crate 不按产品形态分支。
- boundary scripts 已覆盖核心 owner 防回流、six-layer path 解析、facade-only 文件和重点 feature gate。

## 4. 后续大块专项

| 专项 | 目标 | 主要范围 | 准出标准 |
|---|---|---|---|
| H1 | Concrete lifecycle owner 迁移 | concrete scheduler lifecycle、tool pipeline scheduler glue、concrete prompt assembly、AI client factory / provider acquisition；prompt-cache persistence IO 仍由 core 执行 | 先补行为等价测试；迁移后 core 只保留兼容 adapter；不同 OS、remote、本地和 product-full 行为不变 |
| H2 | DeepReview / MiniApp concrete adapter 收口 | DeepReview Task launch、session metadata cache persistence；MiniApp workflow 的 UI asset / desktop scheduler / AI factory 调用；DeepReview queue event 仍由 core coordinator 发送 | 不改变审查队列、报告持久化、MiniApp 执行和权限语义；provider-neutral 规则不得回流 core |
| H4 | 外部 Agent Runtime SDK 发布准备 | 版本策略、公开 API 冻结、最小 feature 依赖证明、示例和兼容承诺 | SDK 不依赖 `bitfun-core`、app crate、Tauri、concrete service manager 或产品命令 registry；fake provider / service / tool / harness / hook / workspace-scoped agent registry smoke 保持通过 |

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
| Product shape / SDK | `cargo test -p bitfun-product-capabilities`，`cargo test -p bitfun-core product_tool_runtime`，SDK fake-provider smoke，cargo tree / metadata 对比 |
| 大范围 owner 迁移 | `cargo check --workspace`，必要时补 `cargo test --workspace` |

## 7. 暂停条件

- 新 owner crate 必须依赖回 `bitfun-core` 才能编译或测试。
- Execution / contracts crate 吸收 Tauri、产品命令、AI provider、MCP client、process execution、Git provider 等 concrete dependency。
- Product Assembly 变成无类型 service locator 或全局 mutable app state。
- PR 只新增抽象，没有迁移、删除或显著简化旧 core 主体路径。
- SDK facade 必须暴露 `bitfun-core`、`product-full`、concrete service manager 或产品命令 registry 才能完成基本 agent 执行。
