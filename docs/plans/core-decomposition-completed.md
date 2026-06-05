# BitFun Core 拆解已完成内容归档

本文只记录已完成事实摘要，不作为后续执行计划。后续执行口径以
[`core-decomposition-plan.md`](core-decomposition-plan.md) 为准；稳定架构目标以
[`core-decomposition.md`](../architecture/core-decomposition.md) 和
[`agent-runtime-services-design.md`](../architecture/agent-runtime-services-design.md) 为准。

## 1. 已完成主线摘要

### 1.1 基础边界与 owner crate 基线

- 已建立 `product-full` 作为完整产品能力保护开关，产品入口显式启用完整能力。
- 已将原 nested `terminal-core`、`tool-runtime` 移到 workspace 顶层，保持旧 package / lib 语义。
- 已抽出 `bitfun-core-types`、`bitfun-agent-stream`、`bitfun-runtime-ports` 等基础契约与轻量 owner。
- 已建立 `bitfun-services-core`、`bitfun-services-integrations`、`bitfun-agent-tools`、`bitfun-tool-packs`、
  `bitfun-product-domains`、`bitfun-runtime-services`、`bitfun-agent-runtime`、`bitfun-harness`、
  `bitfun-product-capabilities` 等 owner crate 基线。
- `bitfun-core` 已通过 facade / re-export 保持旧路径兼容，并逐步形成 `product_runtime`、
  `product_domain_runtime`、`service_agent_runtime` 等迁移期组装入口。

### 1.2 稳定契约与 Runtime Services 基线

- `runtime-ports` 已承接 workspace、session store、remote workspace/projection、tool runtime handles、
  thread goal DTO、scheduled-job state 等稳定接口或事实。
- `runtime-services` 已建立 typed service bundle、builder、capability availability、provider 注册和 fake provider
  测试基础。
- remote workspace facts、remote session metadata、remote file projection DTO、remote workspace/projection host
  trait 已归入稳定接口层，并保留旧路径 re-export。
- session restore 的 storage path resolution、turn-load request、restore timing facts 已进入 Runtime Services /
  Runtime Ports 边界；core 仍保留具体 persistence IO。
- `bitfun-core::product_assembly::CoreRuntimeServicesProvider` 已把现有 core session store path resolution、
  remote workspace/projection adapter，以及当前完整产品能力需要的 terminal / Git / Network / MCP catalog
  capability marker 注册到 `RuntimeServicesBuilder` 组合中；该边界只表达当前能力可用性，不改变具体执行逻辑。

### 1.3 Tool Runtime 与 Product Capability 基线

- `agent-tools` 已承接 provider-neutral tool DTO、manifest/catalog 策略、execution admission gate、
  collapsed unlock gate、static provider materialization 和 plan-to-registry assembly。
- `tool-runtime` 已承接本地 Write / Edit / Delete / Glob 的低风险 concrete IO primitive；core 保留 agent-facing
  `Tool` adapter、权限、checkpoint、file-read freshness、remote fallback 和具体 tool context。
- `tool-runtime` 已承接远程 Delete / Read / LS / Glob / Grep 的命令规划、stdout/stderr 规整、marker 解析、
  path display 和 offset/limit windowing 等低层 execution helper；core 仍负责 workspace shell 执行和 ToolResult 包装。
- GetToolSpec concrete adapter、manifest resolver、visible tools、readonly catalog、snapshot wrapper、collapsed unlock
  message-derived state 已收敛到 product/tool runtime owner 边界。
- `tool-packs` 已承接 tool provider group plan、按 id 选择和 unknown provider group 校验。
- `product-capabilities` 已承接 capability id、required service capability、tool provider group selection 和
  harness provider selection 等 assembly facts。
- Product Assembly 已承接 `DeliveryProfile`、`CapabilitySet`、完整 product-full provider plan 和 service availability
  report；core tool runtime 与 harness registry 已改为消费显式 Product Assembly plan，并保持旧 facade 输出等价。

### 1.4 Agent Runtime 与 Harness 契约基线

- `agent-runtime` 已承接 scheduler/background delivery 的纯决策、turn outcome lifecycle plan、thread goal runtime 的
  accounting / mutation / continuation plan、subagent visibility / availability、prompt cache facts、mode/source presentation facts、
  scheduled-job lifecycle state、custom subagent schema/default/markdown IO/discovery/loading、post-call hook routing、
  tool confirmation plan、goal/user-question tool wire contract、`SessionControl` 输入契约 / cancel route / 结果文案、
  builtin agent catalog 和部分 event fact 映射。
- core 仍保留 concrete session manager、metadata/persistence IO、scheduler lifecycle、event emitter、
  permission UI/channel wait、concrete prompt assembly、product `Tool` adapter 和具体 hook side effect。
- `harness` 已建立 workflow descriptor、legacy route plan、provider registry，并注册 Deep Review、DeepResearch、
  MiniApp 的 legacy-facade provider。
- Harness 当前只证明 route/descriptor 边界，不代表 concrete workflow execution 已迁移。

### 1.5 Product Domain 与 function-agent / MiniApp 边界

- `product-domains` 已承接 MiniApp 的纯状态、runtime detection policy、worker capacity / idle / LRU policy、
  host method、`fs.*` / `shell.exec` host call plan、fs access / shell token / cwd / timeout / env 等纯决策，
  以及 function-agent prompt / parser / response policy / ports。
- 内置 MiniApp bundle identity、版本和 embedded source assets 已归入 `product-domains`。
- function-agent Git concrete snapshot、no-HEAD diff fallback、非 Git workspace fallback、ahead/behind/last-commit
  fallback 和 project context lookup 已迁入 `services-integrations::function_agents`。
- function-agent AI provider acquisition、AI transport error mapping、MiniApp worker process、host side-effect dispatch、
  `net.fetch` / `os.info` runtime execution、PathManager integration、marker IO 和 seed 写盘仍留在 core concrete path。

## 2. 已建立的保护

- owner crate 不得依赖回 `bitfun-core`。
- `product-full` 继续保护完整产品能力集合。
- boundary check 已覆盖多个 owner crate 的禁止依赖、旧路径 facade-only 和回流约束。
- 已有 focused baseline 覆盖 tool manifest、GetToolSpec、execution admission、MiniApp storage / builtin asset、
  remote workspace fallback、MCP config/catalog、agent-runtime prompt cache、custom subagent、thread-goal tools、
  AskUserQuestion、DeepReview hook measurement、tool confirmation、product capability pack、session restore、
  local/remote tool IO helper、function-agent Git、scheduled-job state 等路径。
- 构建脚本、installer 和发布形态不是 core decomposition 迁移的默认修改范围。

## 3. 明确未完成边界

- `bitfun-core` 仍是完整产品 runtime 组装点，不能声明已经退化为纯 compatibility facade。
- 产品入口仍主要通过 `bitfun-core` 的 `product-full` 获取完整能力；Product Assembly 已可表达当前完整能力集合，
  但尚未完成按交付形态裁剪 feature / dependency。
- concrete session manager、scheduler lifecycle、event delivery、permission UI/channel wait、prompt assembly、
  session persistence IO、AI client factory / provider acquisition 仍在 core。
- Bash tool orchestration、terminal lifecycle / PTY、indexed workspace search service owner、remote shell executor abstraction、
  remote terminal concrete impl、MiniApp worker / host / seed / marker IO、Deep Review / DeepResearch / MiniApp concrete workflow execution
  仍未完成 owner 迁移。
- feature matrix、dependency trimming、build-benefit 仍未用 `cargo metadata` / `cargo tree` / build check 数据闭环。
