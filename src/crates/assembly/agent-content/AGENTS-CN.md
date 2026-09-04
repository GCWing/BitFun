**中文** | [English](AGENTS.md)

# 内置 Agent 内容

本 crate 只归属随产品发布的不可变内置 Agent prompt 字节，以及为 `openbitfun-core` 兼容保留的稳定查询 key。
它不依赖第三方库，只由 Core 的 `product-full` 组装启用。

## 边界规则

- prompt 选择、渲染、模式策略、Memory 与 Insights 工作流、运行时状态及错误处理继续由原 owner 持有。
- 不在这里加载用户、项目、产品定制或插件内容。
- 不新增通用注册表、provider 生命周期、运行时文件查询、文件监听或 fallback。Debug 与 release 都继续使用
  编译期嵌入并保持自包含。
- 保持现有 prompt key 和返回字节完全一致。Agent catalog 的生成过程有意保留旧 Rust 生成源码的换行归一化；
  Memory phase-1 与 Insights 的直接常量保留原 `include_str!` 行为。
- 除非真实产品需求证明边界必须变化，否则保持无 feature、无第三方依赖。

## 验证

修改 catalog、查询行为或 prompt 路径后运行
`cargo test -p openbitfun-agent-content --test prompt_catalog_contracts`。prompt 文本可以有意更新，但不得静默删除或
重命名稳定 key。
