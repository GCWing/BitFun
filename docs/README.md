# 文档地图

> 用途：为 BitFun 代码仓内的文档提供放置路由与一级索引。
> 范围：已跟踪的 `docs/` 内容；不包含源码旁的模块文档。
> 状态：stable。
> 权威语言：中文。治理规则见 [`guideline/docs-governance.zh-CN.md`](guideline/docs-governance.zh-CN.md)。

| 目录 | 进入条件 | 索引 |
|---|---|---|
| `architecture/` | 查稳定架构、owner、依赖或已接受设计 | [`architecture/README.md`](architecture/README.md) |
| `guideline/` | 查全仓常驻开发规范：命令、验证、宿主/远程、日志、i18n、文档治理 | [`guideline/README.md`](guideline/README.md) |
| `specs/` | 写或查需求规格与设计（what & why） | [`specs/README.md`](specs/README.md) |
| `plans/` | 写或查实施计划与收尾记录（how & when） | [`plans/README.md`](plans/README.md) |

以下与代码耦合的产品操作指南作为明确例外保留在本仓 `guideline/`：
飞书远程连接 [`feishu-bot-setup.zh-CN.md`](guideline/feishu-bot-setup.zh-CN.md)
（[English](guideline/feishu-bot-setup.md)）、
发布签名校验 [`verify-downloads.zh-CN.md`](guideline/verify-downloads.zh-CN.md)
（[English](guideline/verify-downloads.md)）。

## 兼容路径

以下旧路径只保留短跳转页，供已发布版本和长期公共链接继续使用；正文只在上表四个权威目录维护：

- [`remote-connect/feishu-bot-setup.md`](remote-connect/feishu-bot-setup.md) / [`remote-connect/feishu-bot-setup.zh-CN.md`](remote-connect/feishu-bot-setup.zh-CN.md) → `guideline/feishu-bot-setup*.md`
- [`verify-downloads.md`](verify-downloads.md) / [`verify-downloads.zh-CN.md`](verify-downloads.zh-CN.md) → `guideline/verify-downloads*.md`
- [`features/agent-hooks.md`](features/agent-hooks.md) / [`features/agent-hooks.zh-CN.md`](features/agent-hooks.zh-CN.md) → `specs/agent-hooks*.md`
- [`features/remote-workspaces.md`](features/remote-workspaces.md) → `specs/remote-workspaces.md`
- [`features/session-runtime-usage-report-design.md`](features/session-runtime-usage-report-design.md) → `specs/session-runtime-usage-report-design.md`

不要在 `docs/` 根新增权威专题文章。临时调研、评审提示和个人草稿使用未跟踪的
`*.local.md`；稳定内容按上表归入唯一 owner，不在多个目录复制。
