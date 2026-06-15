# BitFun Core 拆解已完成内容归档

本文只记录已完成事实摘要。后续执行路径以
[`core-decomposition-plan.md`](core-decomposition-plan.md) 为准；稳定架构目标以
[`core-decomposition.md`](../architecture/core-decomposition.md) 和
[`agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md)
为准。

## 1. 基础边界

- 已建立 `product-full` 作为完整产品能力保护开关，产品入口显式启用完整能力。
- 已抽取 `bitfun-core-types`、`bitfun-events`、`bitfun-runtime-ports`、`bitfun-agent-stream` 等基础契约。
- 已建立 `bitfun-services-core`、`bitfun-services-integrations`、`bitfun-agent-tools`、`tool-runtime`、`bitfun-tool-packs`、`bitfun-agent-runtime`、`bitfun-runtime-services`、`bitfun-harness`、`bitfun-product-domains`、`bitfun-product-capabilities` 等 owner crate。
- `src/crates` 已按 `interfaces / assembly / adapters / services / execution / contracts` 六层布局整理，DeepReview path classifier、boundary rules、Cargo workspace path 和根/层级 AGENTS 已同步。

## 2. 已迁移 owner

- `services-core` 已承接 session layout、metadata store CRUD / index rebuild、metadata pagination、metadata construction / mutation、lineage / branch shaping、JSON file store、filesystem primitives、diagnostic redaction、session usage/token usage 基础服务。
- `services-core` 已承接 workspace-runtime legacy session-store merge、metadata 冲突选择、index rebuild 和 legacy path copy/move fallback；core workspace-runtime 只保留路径计算、runtime layout ensure 和错误兼容映射。
- `runtime-services` 已承接 typed runtime service assembly、capability availability、provider registry、capability validation 和 backend event delivery；core backend event system 只保留兼容 re-export。
- `bitfun-events` 已承接 backend event DTO、agentic event DTO 和 platform-neutral `EventEmitter` trait。
- `services-integrations` 已承接 remote-connect primitives、wire command routing / response assembly、workspace search concrete owner、remote SSH/SFTP/PTY owner、DeepResearch report IO、MiniApp host dispatch / storage / worker / import IO。
- `tool-contracts` 已承接 provider-neutral tool DTO、manifest/catalog/admission/result presentation、confirmation facts、truncation recovery presentation。
- `tool-execution` 已承接 local / remote IO helper、Bash shell helper、batching plan、retry policy、state counting、cancellation-state/token-store policy、background exec output capture 和部分 result rendering。
- `agent-runtime` 已承接 scheduler/background delivery 纯决策、dialog lifecycle port contracts、session management/cancellation port contracts、thread-goal facts、prompt / prompt-cache facts、turn skill/agent snapshot DTO/diff/render/store、file-read session state、session evidence ledger 与 compression-contract projection、dialog-turn cancellation token store、tool confirmation / user-question wait channel state、custom subagent discovery/loading、post-call hook routing、DeepReview provider-neutral policy/queue/retry/diagnostics shaping、DeepResearch citation renumber。
- `harness` 已建立 descriptor、route plan 和 legacy provider registry。
- `product-domains` 已承接 MiniApp state/workflow planning、compile / permission adaptation、import lifecycle、function-agent prompt/parser/response policy 和部分 Git snapshot/fallback 逻辑。
- Product Assembly 已承接 `DeliveryProfile`、`CapabilitySet`、product-full provider plan、service availability report 和 profile-scoped harness registry 入口。

## 3. 已建立保护

- owner crate 不得依赖回 `bitfun-core`。
- `product-full` 保持完整产品能力集合。
- boundary check 覆盖 owner crate 禁止依赖、旧路径 facade-only、feature gate、six-layer path 解析、Product Assembly 收口和高风险 owner 回流。
- focused baseline 覆盖 tool manifest、GetToolSpec、execution admission、workspace search、remote workspace fallback、MCP config/catalog、prompt cache、custom subagent、thread-goal tools、AskUserQuestion、DeepReview policy、tool confirmation、session restore、MiniApp storage/builtin/import、function-agent Git、scheduled-job state 等路径。

## 4. 明确未完成

- `bitfun-core` 仍是完整 product runtime 组装点，尚未退化为纯 compatibility facade。
- 产品入口仍主要通过 `bitfun-core/product-full` 获取完整能力，交付形态级 feature / dependency trimming 未完成。
- concrete scheduler lifecycle、prompt-cache persistence orchestration、tool pipeline scheduler glue、concrete prompt assembly、AI client factory / provider acquisition 仍在 core 或产品路径。
- DeepReview launch / provider wait / report persistence、MiniApp AI acquisition / larger workflow execution、function-agent AI provider acquisition 仍未完成 owner 迁移。
- Agent Runtime SDK 具备候选原语，但尚未形成可独立发布的稳定外部 SDK 边界。
