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
- `runtime-services` 已承接 typed runtime service assembly、capability availability、provider registry、capability validation、无副作用 capability marker ports 和 backend event delivery；core backend event system 只保留兼容 re-export。
- `bitfun-events` 已承接 backend event DTO、agentic event DTO 和 platform-neutral `EventEmitter` trait。
- `services-integrations` 已承接 remote-connect primitives、wire command routing / response assembly、workspace search concrete owner、remote SSH/SFTP/PTY owner、DeepResearch report IO / display-map sidecar、MiniApp host dispatch / storage / worker / import IO。
- `tool-contracts` 已承接 provider-neutral tool DTO、manifest/catalog/admission/result presentation、confirmation facts、truncation recovery presentation、runtime restriction policy 和 provider-entry materialization。
- `tool-execution` 已承接 local / remote IO helper、Bash shell helper、batching plan、retry policy、state counting、cancellation-state/token-store policy、background exec output capture 和部分 result rendering。
- `agent-runtime` 已承接 scheduler/background delivery 纯决策、dialog lifecycle port contracts、session management/cancellation port contracts、thread-goal facts、prompt / prompt-cache facts 与持久化写入决策、turn skill/agent snapshot DTO/diff/render/store、file-read session state、session evidence ledger 与 compression-contract projection、dialog-turn cancellation token store、tool confirmation / user-question wait channel state、custom subagent discovery/loading、post-call hook routing、DeepReview provider-neutral policy/queue/retry/diagnostics shaping 与 queue event payload shaping、DeepResearch citation renumber 与 report post-process gate，并建立不暴露 `bitfun-core` / `product-full` / concrete manager 的内部 SDK facade。SDK facade 已支持注入 fake runtime services、tool registry、harness registry、hook registry 和 agent registry。
- `harness` 已建立 descriptor、route plan 和 legacy provider registry。
- `product-domains` 已承接 MiniApp state/workflow planning、compile / permission adaptation、import lifecycle、AI / Agent permission、rate-limit、model/message/session/workspace/turn-text bridge rules、function-agent prompt/parser/response policy 和部分 Git snapshot/fallback 逻辑。
- `bitfun-core` 的 function-agent AI concrete acquisition 已从旧 `runtime_services` 路径收拢到明确的 core port adapter；Git / AI compatibility re-export 仍保留旧 public path。
- Product Assembly 已承接 `DeliveryProfile`、当前交付形态入口矩阵、`CapabilitySet`、feature group matrix、profile-scoped capability plan、product-full provider plan、service availability report、profile-scoped harness registry 入口与 legacy-route 行为保护，以及 `ProductAssembler` 对 explicit profile input、runtime services、harness registry 和 service requirement 的验证；core 只保留兼容 re-export。ProductFull / Desktop / CLI / ACP 保留完整能力；Server / Remote / Web / MobileWeb 不再 materialize product-full capability packs、feature groups、runtime services、tool groups 或 harness routes。

## 3. 已建立保护

- owner crate 不得依赖回 `bitfun-core`。
- `product-full` 保持完整产品能力集合。
- boundary check 覆盖 owner crate 禁止依赖、旧路径 facade-only、feature gate、six-layer path 解析、Product Assembly 收口和高风险 owner 回流。
- focused tests 覆盖当前 delivery profile 能力裁剪、ProductAssembler 缺失 service 报告、无直接 core 入口的空 capability plan、SDK fake provider / services / tool / harness / hook / workspace-scoped agent registry 闭环，以及 runtime hook 顺序、timeout、错误策略和重复 id 拦截。
- focused baseline 覆盖 tool manifest、GetToolSpec、execution admission、workspace search、remote workspace fallback、MCP config/catalog、prompt cache、custom subagent、thread-goal tools、AskUserQuestion、DeepReview policy、tool confirmation、session restore、MiniApp storage/builtin/import、function-agent Git、scheduled-job state 等路径。

## 4. PR-D 完成后的后续专项

- `bitfun-core` 仍承载 compatibility facade / `product-full` assembly 和少量迁移期 adapter；不应继续新增 owner 逻辑。
- 产品入口的能力裁剪已由 Product Assembly profile plan 表达；后续新增入口必须先明确 `ProductCoreDependencyMode`、unsupported / unavailable 语义和兼容性测试。
- concrete scheduler lifecycle、tool pipeline scheduler glue、concrete prompt assembly、AI client factory / provider acquisition 仍在 core 或产品 adapter；继续迁移必须单独证明行为等价。Prompt-cache 持久化的 restore / save / delete 决策已迁入 `agent-runtime`，core 只执行实际 persistence IO。
- DeepReview concrete Task launch 和 session metadata cache persistence 仍是 core adapter，因为它们依赖 coordinator、session manager、subagent runtime 和产品事件；provider-neutral policy / queue / retry / report shaping 与 queue event payload shaping 已在 `agent-runtime`，core 只负责事件发送。
- MiniApp larger workflow 的 UI asset / desktop scheduler / AI factory 调用仍属于产品 host adapter；可复用规则已迁入 `product-domains`，不再在 desktop 命令内重复实现。
- Agent Runtime SDK 已具备内部 facade、最小 fake-provider 闭环和 runtime services / tool / harness / hook / workspace-scoped agent registry 注入基线，但尚未冻结为可独立发布的外部 SDK 包、版本策略和兼容承诺。
