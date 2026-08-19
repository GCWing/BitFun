# 普通对话模型设置：bindSheet 与模型选择

Date: 2026-08-19

Scope: `src/apps/mobile/harmonyos` 设置里的「普通对话 / 模型」面板，以及同一套手搓蒙层的语言面板。不改 ComposerBar 选择器、relay、桌面端。

## 问题

`ModelServiceSettingsPanel` 不是系统 Sheet，而是塞进 `SettingsSheet` Stack 的手搓蒙层（`78%` 死高度 + 自绘圆角 + 自绘 ×）。设置页那个 × 是同一个 Stack 的 overlay，所以两个 × 同时出现；内容只有三行，底下是大片空白。

`checkmark_circle_fill` 在「当前使用 / 账号同步 / 本机自定义」上各出现一次。后两个表达的是来源归属，和「当前使用」那一行重复。

账号同步行没有 `onClick`。用户看见「已同步 12 个」却进不去、也换不了模型。真正的切换只在 ComposerBar 的 `bindSheet` 里。语言面板是同一套手搓蒙层，同样会和设置页 × 叠在一起。

## 目标

1. 设置子面板收敛到和 `ComposerBar` 一样的 `bindSheet`（`showClose: false`、`dragBar: true`、高度按内容算）。一次去掉双 ×、死高度空白、手搓蒙层。
2. `✓` 只表示「当前选中的模型」。来源行不再带 ✓。
3. 账号同步行可点，带 `>`，打开真实的账号模型列表，点一项即 `SettingsController.selectModel`。
4. 信息架构收成两组：「当前使用」+「模型来源」。账号同步和本机自定义是同级入口，放在同一分组容器里，中间一条 divider；分组标题和行文字共用 16vp 水平边距。

语言面板做同一套 `bindSheet` 收敛，不再单独留一手搓模态。

## 非目标

- 不改 ComposerBar 里那套对话中模型选择器。
- 不改账号模型的同步/拉取合同。
- 不把设置面板做成 ComposerBar 选择器的第二份拷贝；设置侧分「状态 / 来源 / 本机配置」三层，选择器只负责挑一个 id。

## 交互

设置页「模型」行 → `bindSheet` 打开概览。

```
[普通对话模型                    ×]

当前使用
[ ✓  deepseek-v4-pro              ]
[    云端账号                     ]

模型来源
[ ☁  云端账号模型              > ]
[    已同步 12 个                ]
[ ------------------------------- ]
[ 🔧 本机模型名                > ]
[    本机                        ]
```

- 账号行 → 再挂一层 `bindSheet`，列表高度按 `min(条数, 7)` 算，和 ComposerBar 同一公式。点一项调用 `selectModel`，关掉列表，概览上的「当前使用」跟着变。0 条也进得去，显示空态。
- 本机行 `>` → 再挂一层 `bindSheet` 打开本机 API 编辑器（自适应/固定表单高度，保留键盘避让）。已配置时，点行主体会选中本机模型；未配置时点整行进编辑器。保存成功后沿用现有 `save` 路径。
- 概览「当前使用」只读。`✓` 只出现在这里，以及账号列表里当前那一项。

设置页「语言」行同样改 `bindSheet`，高度按语言条数算。

## 结构

| 单元 | 职责 |
|---|---|
| `ModelServiceSettingsPolicy` | 当前模型解析、账号模型过滤、来源分组、三页 Sheet 高度。组件不内嵌这些规则。 |
| `SettingsSheet` | 在根节点上 `bindSheet` 语言 / 概览 / 账号列表 / 本机编辑器。不再把子面板叠进 Stack。 |
| `ModelServiceSettingsPanel` | 只画当前页：`overview` / `account` / `local`。发出 `onOpenAccountModels` / `onOpenLocalEditor` / `onSelectModel`。 |
| `LanguageSettingsPanel` | 只画语言列表，不再自绘蒙层。 |
| `SettingsPresentationActions.selectGeneralModel` | 接到 `SettingsController.selectModel`。 |

设置页本身已经是 `AppShell` 上的 `bindSheet`。子面板再 `bindSheet` 一次，真机上点击会改状态但第二张 Sheet 不会出现（`5ZU0226202001116` 上点「模型」后仍只有一张 `SheetPage`）。因此语言 / 概览 / 账号列表 / 本机编辑器改为**替换设置列表**，不嵌套系统 Sheet。设置页的 × 只在主列表显示，避免双 ×。

## 高度

公式放在 policy 里，组件只读数字：

- 概览：header + 「当前使用」一行 + 「模型来源」两行分组，约 345vp。
- 账号列表：`min(480, chrome + min(n, 7) * row + 行距)`，空态用一行占位高。
- 本机编辑器：560vp，够放下三字段、测试/保存和反馈；聚焦时仍走现有 `KeyboardAvoidMode.RESIZE`。
- 语言：`chrome + 条数 * 64`。

三页各绑一张 Sheet，打开时按当时内容算高度，避免同一张 Sheet 切页后高度不刷新。

## 测试

`ModelServiceSettingsPolicy` 覆盖：当前模型解析顺序、只把 `cloud:` 且 `enabled` 的项算进账号列表、来源行不承担选中语义、三页高度随内容变且账号列表封顶 7 行。`ArchitectureUnit` 不应被这次改动破坏。i18n 中英 key 锁步。

## 远程场景

这次只动本机设置壳。账号模型数据仍来自已有云端同步；选择写进 `GeneralChatConfigStore`，和 ComposerBar 走同一条 `selectModel`。不新增远程命令，也不把本机 API Key 泄漏到远端控制器。
