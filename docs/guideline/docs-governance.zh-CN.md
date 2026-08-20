# 本仓文档规范

用途：约定 BitFun **代码仓**里文档如何存放、如何撰写、如何索引。
适用范围：仓库内 `docs/`、根目录 `AGENTS` / `CONTRIBUTING`、以及模块旁的 `AGENTS.md`。
状态：stable（目标结构已定：`architecture/`、`guideline/`、`specs/`、`plans/`）
权威语言：中文（本文件）。英文摘要见 [`docs-governance.md`](docs-governance.md)。

## 不可违反的语义保持规则

1. 文档重组必须保持规范语义。拆分、合并、重命名和重建索引只能改变呈现，不能改变 owner、要求、
   当前/目标状态、失败行为或验收条件。
2. 移动或重命名文档前，必须盘点 Markdown、源代码、配置、测试、打包和产品外链中的全部入站引用；
   同一次修改更新所有引用，不能依赖读者猜测兼容路径。
3. 只有证明代码、运行时行为、构建/打包、测试和用户可见产品链接都不依赖仓库路径后，文档才能迁出代码仓。
   否则必须保留；若确需迁移，则稳定替代 URL、代码引用和聚焦测试必须在同一次修改中完成。
4. 合并内容时必须原样保持 current/proposed/completed 等成熟度标签。移动文字不等于获准改变其权威级别。
5. 重组若删除或合并权威文档，PR 必须提供旧内容到新位置的映射。链接检查只能证明可达，不能证明语义等价；
   映射仍须人工评审。
6. 若本仓启用产品级 4+1 / 架构文档检查脚本，经批准迁移 Authority 时，必须在同一修改中更新检查目标及相关映射；不能只删标题或把局部 L1 当作产品 L0。本仓当前若尚未提供 docs:architecture:check，以人工对照索引与本规范为准。

## 流程产物

- 不要把临时过程草稿跟踪进 docs/。可长期保留的规格/设计放 docs/specs/，实施计划放
  docs/plans/，临时过程文档仅保留在本地（或未跟踪的 *.local.md）。架构事实写回
  docs/architecture/，用户可见说明放到所属应用 README。
- 已废弃的 `docs/superpowers/**`、`docs/features/**`、`docs/development/**` 路径不得新增权威正文。只有已发布产品或长期公共 URL 仍指向旧路径时，才可保留最小兼容页；兼容页只能链接权威新位置，并登记在 `docs/README.md`。

## 本仓文档范围

本代码仓应跟踪：

- 改本仓代码时必读的边界与操作约定
- 架构约束、验证矩阵、命令列表
- 随 PR 推进的规格与实施计划（进行中或已稳定）
- 为已跟踪 Spec 或 Plan 提供依据的长期调研与技术审阅；它们是非规范参考，必须标注证据日期
- 模块旁 `AGENTS.md` / `LOGGING.md`

把随 PR 演进的进行中 Spec 与实施计划纳入版本控制，是当前有意采用的流程政策。临时提示词、调研草稿、
评审草稿和个人笔记不属于仓库文档，必须保持未跟踪；本地需要文件名时使用 `.local.md` 后缀。

飞书远程连接配置与发布签名校验是明确列出的、与代码耦合的操作指南例外，保留在本仓
[`docs/guideline/feishu-bot-setup.zh-CN.md`](feishu-bot-setup.zh-CN.md)
（[English](feishu-bot-setup.md)）与
[`docs/guideline/verify-downloads.zh-CN.md`](verify-downloads.zh-CN.md)
（[English](verify-downloads.md)），分别由 `RemoteConnectDialog.tsx` 与根 `README.md` 引用。

## 本仓 `docs/` 结构

```text
docs/
  README.md         # 目录地图与放置路由；不承载规范正文
  architecture/     # 稳定架构；ADR 也放这里（不另建顶层 ADR 目录）
  guideline/        # 开发规范：命令、验证、宿主/远程、agent-loop、本文
  specs/            # 需求与设计（what & why；索引见 README）
    README.md
    templates/
  plans/            # 实施计划 + 收尾（how & when）
    README.md
    templates/
```

以上四个目录是唯一权威文档桶。已废弃路径下的兼容页不拥有正文，只负责指向权威新位置。

## 文件夹边界

| 目录 | 必须放什么 | 明确禁止放什么 |
|---|---|---|
| `docs/architecture/` | 稳定的跨模块架构边界、owner/依赖规则、已接受设计权威、ADR | 实施任务清单、临时评审记录、用户配置指南、性能数据快照、模块局部编码规则 |
| `docs/guideline/` | 仓库操作和改代码规则，以及索引中明确列出的代码耦合操作指南 | 产品需求、功能实施计划、一般用户手册、从 `architecture/` 复制的稳定产品架构 |
| `docs/specs/` | draft/in-progress Spec、功能设计、已稳定单特性设计、已索引的非规范调研与技术审阅 | 第二套稳定跨模块架构权威、个人草稿、原始生成证据、用户/运维指南、实施任务清单 |
| `docs/plans/` | 可独立执行的实施计划和 `-completed.md` 收尾记录 | 需求、设计正文、个人草稿、稳定架构 |
| `docs/` 根 / 已废弃路径 | `README.md`、未跟踪的 `*.local.md`，以及已登记的兼容页 | 新权威正文、跟踪的专题文章、跟踪的 `.local.md`、重复索引、生成产物 |

最近的目录 README 负责完整文章清单和本目录边界。Spec 中发现的稳定结论必须迁入既有
architecture 权威，原文改为链接，不能保留竞争性的第二份规则正文。

## 二级索引

```text
AGENTS.md  →  目录 README / 单篇权威文档  →  （最多再跳一次）正文
```

- 从匹配的入口/索引到权威正文最多两跳。
- 每个含多篇文章、持续维护的文档目录必须有 README，写清范围、排除项和完整文章索引。
- 除模板外，每篇受治理文档必须至少有一个索引或任务路由入站引用；新增/重命名文档必须同步更新最近索引。
- 兼容页单独登记在 `docs/README.md`，内容只包含权威目标和保留原因。
- 高频单篇可由 AGENTS 直接链接（如 `product-architecture.md`、`verification.md`）。
- 索引只放路由摘要，不复制规范正文。

## 语言

| 类型 | 语言 | 是否双语 |
|---|---|---|
| 面向人阅读的说明、流程 | 以中文为准 | 默认不强制英文 |
| 根目录 `AGENTS` / `CONTRIBUTING` | — | 中英都要有，语义必须对齐 |
| 主要给 AI / 改代码时查阅的操作与约束（如 `guideline/*`、模块 `AGENTS`） | 以英文为准 | 默认不强制中文副本 |
| 日志 | 只用英文 | 不做中文或双语日志 |

## 格式

- 页首写清：用途、适用范围、状态（draft/stable/reference）、权威语言、相关链接。
- 能链到权威文档就不要把正文再抄一份。
- 文件名用英文 kebab-case。
- 普通文档的双语对使用 `<name>.md` 与 `<name>.zh-CN.md`。根及模块级规范入口继续使用仓库约定的
  `AGENTS.md` / `AGENTS-CN.md`；根贡献入口保留 `CONTRIBUTING.md` / `CONTRIBUTING_CN.md`。
- 独立实施计划以 `-plan.md` 结尾；收尾记录以 `-completed.md` 结尾。

## Spec / Design / Plan

- 需求与设计（what & why）：[`docs/specs/README.md`](../specs/README.md)
- Spec 模板：[`docs/specs/templates/`](../specs/templates/)
- 实施计划与收尾（how & when）：[`docs/plans/`](../plans/)
- Plan 模板：[`docs/plans/templates/`](../plans/templates/)

## 根入口

| 文件 | 位置 | 职责 |
|---|---|---|
| `AGENTS.md` / `AGENTS-CN.md` | 仓库根 | 改代码规范入口；渐进披露，细则外链 |
| `CONTRIBUTING.md` / `CONTRIBUTING_CN.md` | 仓库根 | 人如何参与；命令/验证链到 `guideline/*`，规范链到 AGENTS |

二者互相链接；CONTRIBUTING 不再维护第三套完整命令清单。

## 相关

- 命令：[`common-commands.zh-CN.md`](common-commands.zh-CN.md)
- 验证：[`verification.zh-CN.md`](verification.zh-CN.md)
- 开发文档索引：[`README.md`](README.md)
- 文档总地图：[`docs/README.md`](../README.md)
- 规范入口：[`AGENTS.md`](../../AGENTS.md) / [`AGENTS-CN.md`](../../AGENTS-CN.md)
- 贡献指南：[`CONTRIBUTING.md`](../../CONTRIBUTING.md) / [`CONTRIBUTING_CN.md`](../../CONTRIBUTING_CN.md)
