**中文** | [English](AGENTS.md)

# OpenCode Adapter

当前 crate 负责现有受管包路径使用的 P0 OpenCode 静态来源预览、受管 Plugin Host Config Hook 结果的类型化
Agent/权限/Plugin Tool/Skill 投影，以及 Command、standalone Tool 和 Subagent 能力专属 provider 契约的 OpenCode
实现。它保留 OpenCode 来源发现、优先级、格式、参数展开和版本化兼容语义。
共享来源目录、生命周期协调、
文件观察实现、产品策略、界面、凭据、worker 监督和最终结果写入均由其他 owner 负责。

## 产品来源边界

- 当前 `load_opencode_package_adapter` 在 OC-R1/OC-R2 替换其生产角色前仍只做静态预览；不得把这一 P0
  入口继续扩成另一种 OpenCode 受管包格式。
- 当前已将 OpenCode Command、standalone Tool 和 Subagent 的标准配置与目录作为只读实时来源；完整插件目录和
  package spec 仍是后续目标，不是可执行的生产来源。源文件无需导入 OpenBitFun。低风险声明式结果按用户的自动应用/
  先询问偏好处理；可执行来源首次 import 前按来源、插件身份和执行域
  决策。import 前可见的运行条件扩大和 import 后贡献扩大是两个独立门槛，不对每个内部生命周期状态重复审批。代码
  更新只有在来源身份/完整性、来源更新策略和当前运行条件仍允许时才能自动准备。
- 全局来源偏好按来源、插件身份和执行域去重，但每次 activation/import 必须重新计算有效来源图、工作目录/环境、
  凭据和策略；workspace 只在配置归属模块或逻辑插件实例确有独立状态时参与。原始解析结果和内容完全匹配的文件缓存可以共享，
  物理健康按实际进程分组，不默认按全局或 workspace 建键。跨项目本身
  不重复询问，只有运行条件、凭据或能力扩大时确认。
- `ExternalSourceControlPlane` 负责候选版本，并通过现有来源协调模块替换 provider；本 adapter 通过窄 provider 契约提供 OpenCode 限定的来源
  身份/顺序和观察根，可复用文件观察服务只提供变化事实。本 crate 提供规范化配置快照和 Config contributor
  类型化投影，Core 保留逻辑 workspace generation 与能力原子发布；`opencode-plugin-host` 负责 Host wire/连接，`ScriptToolRuntime`
  所在的 services 实现拥有依赖、worker、进程树和物理健康；`PluginRuntimeClient` 当前只拥有请求可靠性、诊断和
  故障诊断，并消费既有归属模块提供的生命周期事实；贡献注册仍由对应能力归属模块提交。
- 第三方模块 import 前必须依据来源、插件身份、实际执行域/用户、产品/组织策略上限、凭据范围和环境范围重新计算
  当前有效策略与安全启动模式。来源发现或配置导入批准不等于执行决策；产品来源体验和既有能力归属模块提供
  来源/插件决策，本适配器只消费该结果，不拥有提示或信任状态。激活后的本地运行时默认使用兼容模式。
- 最终工具生成、权限结果、权威状态和审计事实仍由工具、权限、产品和运行时归属路径完成。
- 用户本机是否安装 `opencode` CLI 与加载 OpenCode-compatible 插件无关。与已安装 OpenCode 可执行文件
  的 CLI/server 互操作属于 ACP/external-client 工作，不属于本适配器边界。

## 边界规则

- 依赖 `openbitfun-runtime-ports` 等稳定接口和 `PluginRuntimeAdapter` 边界 trait，不依赖
  `openbitfun-core`、app crate、Tauri API、产品界面或具体服务管理器。
- OpenCode 配置 JSON、来源顺序、加载器兼容、参数展开和受管 Plugin Host Config 贡献归属保留在本 crate 内。
  跨 crate 输出使用类型化来源快照、生态无关的 Agent/权限/Plugin Tool/Skill 贡献、adapter 绑定和 `PluginRuntimeClient` DTO，
  不得把 OpenCode 原始 JSON 或源码语法暴露为产品接口。Config 投影不得生成 Agent runtime key、选择原生 Tool
  基线、修改产品 registry 或扩成通用 Config 平台；OpenCode contributor 解析和 Tool registration 转换仍留在本 crate，
  不得让 OpenCode 专属 DTO 进入通用发布模块。
- 当前源码探测只识别测试覆盖的声明式语法子集，不是通用 JS/TS 解析器；没有可识别入口的包和已识别但不支持的 hook
  必须返回诊断，其他语法不属于当前兼容范围。
- 未支持的 OpenCode 能力必须显式返回类型化诊断或不支持状态，不得静默忽略。
- 公开接口必须同步当前 Product Assembly 消费方、能力专属 provider 契约、边界更新和聚焦测试；不得暴露通用
  OpenCode JSON 访问，也不得只为目标设计完整性增加 API。
- 经评审的产品组装根只选择并构造已编译的 OpenCode adapter/provider，再注入 `PluginRuntimeClient`；它不发现
  动态来源、不准备依赖，也不 import 插件模块。
- Product Assembly 只允许从经过评审的组装模块（如 `openbitfun-core/plugin_runtime`、
  `openbitfun-core/external_sources` 或 OpenCode 专属的 `openbitfun-core/plugin_host` 组装路径）消费本 crate；增加其他消费方时必须同步
  边界脚本和聚焦组装路径测试。
- 本 crate 不得依赖 Codex、Claude Code 或其他生态 adapter。新生态是由 Product Assembly 注册的同级 adapter，
  不是本 adapter 的模式。
- 生产 crate 不得直接依赖 `openbitfun_opencode_adapter` 内部类型。未支持能力必须诊断化，
  不得因外部插件内容导致运行时崩溃。

## 验证

- `cargo test -p openbitfun-opencode-adapter --test opencode_source_adapter`
- `cargo test -p openbitfun-opencode-adapter --test opencode_mcp_adapter`
- `cargo test -p openbitfun-opencode-adapter --test opencode_static_source_contracts opencode_command_adapter::`
- `cargo test -p openbitfun-opencode-adapter --test opencode_static_source_contracts opencode_subagent_adapter::`
- `cargo test -p openbitfun-opencode-adapter --test tool_source_contracts`
- `cargo test -p openbitfun-opencode-adapter --lib p0_c2_fixture`
- `cargo test -p openbitfun-opencode-adapter --lib plugin_config_projection`
- `cargo test -p openbitfun-opencode-adapter --lib client_path_projects_trusted_custom_tool_candidate_with_permission_prompt`
- `node scripts/check-core-boundaries.mjs`
