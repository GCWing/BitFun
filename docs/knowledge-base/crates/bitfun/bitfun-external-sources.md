# bitfun-external-sources

**路径**: src/crates/assembly/external-sources
**描述**: Ecosystem-neutral external source lifecycle coordination。协调器消费 provider 特定契约，不做生态身份分支。

## 模块

- `control_plane` — 外部源控制平面
- `hook` — Hook 目录协调器
- `mcp` — MCP 外部源协调器
- `refresh` — 延迟发现、发现批次
- `subagent` — 子 agent 协调器
- `tool` — 工具协调器

## 核心类型

- `ExternalSourceCoordinator` — 主协调器：管理 provider generation、抑制、降级、冲突选择
- `ExternalSourceControlPlane` — 控制平面
- `ExternalSourceDiscoveryRequest/Result` — 发现请求/结果
- `ExternalHookCatalogCoordinator`, `ExternalHookDiscoveryResult` — Hook 协调
- `ExternalMcpCoordinator`, `ExternalMcpCoordinatorSnapshot`, `ExternalMcpDiscoveryRequest/Result` — MCP 协调
- `ExternalSubagentCoordinator`, `ExternalSubagentCoordinatorSnapshot` — 子 agent 协调
- `ExternalToolCoordinator`, `ExternalToolCoordinatorSnapshot` — 工具协调
- `DeferredDiscovery`, `DiscoveryBatch` — 延迟发现
- `ExternalSourceCatalogSnapshot` — 目录快照（来源、命令、冲突、MCP、子 agent）
- `PromptCommandDefinition`, `PromptCommandCatalogEntry`, `PromptCommandConflict` — 命令契约

内部类型：
- `ProviderGeneration`, `SourceGeneration` — provider 状态跟踪

## 功能

外部源生命周期协调 crate。消费 product-domains 中定义的 provider 契约，实现多 provider 发现、来源抑制/启用、命令冲突检测与选择、版本降级回退。provider 中性——通过 trait 注入具体 provider，不做生态身份分支。
