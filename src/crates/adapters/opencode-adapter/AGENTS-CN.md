**中文** | [English](AGENTS.md)

# OpenCode Adapter

本 crate 拥有 OpenCode-compatible 来源发现和受信任候选映射能力。它验证
`opencode.json` 和 `.opencode/plugins/*.js|ts` 等导入形态，并通过窄 Plugin Runtime Host
主机适配器暴露来源事实、诊断和类型化候选项。它不得拥有产品策略、主机生命周期、
沙箱、界面实现或最终权限/工具结果写入。

## 产品来源边界

- BitFun 插件包和安装来源是生产插件加载入口。OpenCode 配置是可选兼容导入源，
  不是主插件注册表或运行时状态。
- 导入 `opencode.json`、`.opencode/plugins/*.js|ts` 或未来 OpenCode 全局插件目录时，必须先生成类型化
  导入事实、候选 BitFun 插件来源记录、清单、哈希、诊断和信任状态，
  这些结果才能交给产品侧启用或执行链路；适配器自身不直接启用或执行。
- `load_opencode_workspace_adapter` 必须通过既有 `PluginSourceRef` 来源快照和信任 epoch 接收 BitFun 来源信任快照；OpenCode 目录扫描结果不得自行升级为可信。
- 受信任 custom tool 声明只能映射为提供方候选；生成最终工具、权限结果和审计事实仍由工具 ABI、
  权限控制和产品归属路径完成。
- 用户本机是否安装 `opencode` CLI 与加载 OpenCode-compatible 插件无关。与已安装 OpenCode 可执行文件
  的 CLI/server 互操作属于 ACP/external-client 工作，不属于本适配器边界。

## 边界规则

- 依赖 `bitfun-runtime-ports` 等稳定接口和 `PluginHostAdapter` 边界 trait，不依赖
  `bitfun-core`、app crate、Tauri API、产品界面或具体服务管理器。
- OpenCode 配置 JSON 导入和工作区插件导入解析保留在本 crate 内。跨 crate
  输出必须通过 `load_opencode_workspace_adapter` 和 Plugin Runtime Host DTO，不得把 OpenCode 原始 JSON
  或源码语法暴露为产品接口或稳定结构化对象。
- 未支持的 OpenCode 能力必须显式返回类型化诊断或不支持状态，不得静默忽略。
- 当前公开接口预算只允许 `load_opencode_workspace_adapter`。新增或修改公开入口签名/语义必须同步预算、
  当前消费方和聚焦主机路径测试。
- 本 crate 可以提供私有 OpenCode 兼容导入映射器和验证样例用于适配器验证；
  公开入口仍限制为 `load_opencode_workspace_adapter`，并由 Plugin Runtime Host 调用。
- 本 PR 不包含生产产品组装接入。后续如需注册该适配器，必须在同一变更中同步边界脚本和聚焦主机路径测试。
- 生产 crate 不得直接依赖 `bitfun_opencode_adapter` 内部类型。未支持能力必须诊断化，
  不得因外部插件内容导致运行时崩溃。

## 验证

- `cargo test -p bitfun-opencode-adapter --test opencode_source_adapter`
- `cargo test -p bitfun-opencode-adapter p0_c2_fixture`
- `cargo test -p bitfun-opencode-adapter host_path_projects_trusted_custom_tool_candidate_with_permission_prompt`
- `node scripts/check-core-boundaries.mjs`
