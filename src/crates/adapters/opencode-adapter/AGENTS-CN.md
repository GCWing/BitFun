**中文** | [English](AGENTS.md)

# OpenCode Adapter

当前 crate 负责现有受管包路径使用的 P0 OpenCode 静态来源预览。目标设计中，本 crate 负责 OpenCode
生态适配和来源协调：保留来源顺序与生态语义，生成版本化来源/候选事实，并构造注入 Plugin Runtime Host
的适配器。它不得拥有产品策略、worker 监督、界面实现、凭据或最终结果写入。

## 产品来源边界

- 当前 `load_opencode_package_adapter` 在 OC-R1/OC-R2 替换其生产角色前仍只做静态预览；不得把这一 P0
  入口继续扩成另一种 OpenCode 受管包格式。
- 目标流程把 OpenCode 标准配置、全局/项目插件目录、工具目录和软件包 spec 作为实时来源。源文件保持只读，
  但有效结果无需导入 BitFun 或二次激活即可影响运行时。
- OpenCode 来源协调器拥有来源身份/顺序、来源监听、候选代次，以及请求准备或切换代次的决定；配置归属模块
  提供规范化配置快照，脚本执行服务拥有依赖/worker，Plugin Runtime Host 拥有进程健康和贡献注册。
- 第三方模块 import 前必须先解析全局/来源/target 有效策略和安全启动模式。本地默认使用兼容模式，不增加信任
  弹窗；来源策略变化不能伪装成配置导入批准。
- 最终工具生成、权限结果、权威状态和审计事实仍由工具、权限、产品和运行时归属路径完成。
- 用户本机是否安装 `opencode` CLI 与加载 OpenCode-compatible 插件无关。与已安装 OpenCode 可执行文件
  的 CLI/server 互操作属于 ACP/external-client 工作，不属于本适配器边界。

## 边界规则

- 依赖 `bitfun-runtime-ports` 等稳定接口和 `PluginHostAdapter` 边界 trait，不依赖
  `bitfun-core`、app crate、Tauri API、产品界面或具体服务管理器。
- OpenCode 配置 JSON、来源顺序、加载器兼容和来源协调保留在本 crate 内。跨 crate 输出使用类型化来源快照、
  adapter binding 和 Plugin Runtime Host DTO，不得把 OpenCode 原始 JSON 或源码语法暴露为产品接口。
- 当前源码探测只识别测试覆盖的声明式语法子集，不是通用 JS/TS 解析器；没有可识别入口的包和已识别但不支持的 hook
  必须返回诊断，其他语法不属于当前兼容范围。
- 未支持的 OpenCode 能力必须显式返回类型化诊断或不支持状态，不得静默忽略。
- 当前公开接口预算只允许 `load_opencode_package_adapter`。OC-R 实现只有在同步当前消费方、明确的来源协调器/Host
  窄接口、边界更新和聚焦测试后才能替换或增加入口；目标设计本身不表示新 API 已可用。
- 经评审的产品组装根只选择并构造已编译的 OpenCode adapter/provider，再注入 Plugin Runtime Host；它不发现
  动态来源、不准备依赖，也不 import 插件模块。
- 生产组装仅允许位于 `bitfun-core/plugin_runtime`；增加其他消费方时必须同步边界脚本和聚焦主机路径测试。
- 生产 crate 不得直接依赖 `bitfun_opencode_adapter` 内部类型。未支持能力必须诊断化，
  不得因外部插件内容导致运行时崩溃。

## 验证

- `cargo test -p bitfun-opencode-adapter --test opencode_source_adapter`
- `cargo test -p bitfun-opencode-adapter p0_c2_fixture`
- `cargo test -p bitfun-opencode-adapter host_path_projects_trusted_custom_tool_candidate_with_permission_prompt`
- `node scripts/check-core-boundaries.mjs`
