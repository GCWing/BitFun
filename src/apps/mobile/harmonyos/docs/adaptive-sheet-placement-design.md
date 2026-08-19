# 折叠屏与宽屏的 Sheet 放置统一设计

Date: 2026-08-19

Scope: `src/apps/mobile/harmonyos` 的全部覆盖式 Sheet（设置、连接、会话详情、远程视图设置）在窄屏、宽屏、双折叠、三折叠和悬停态下的放置。不改会话 master-detail 几何、文件预览分栏、relay 协议和桌面端。

## 问题

这个 App 现在跑着**两套布局语言**。

会话面用的是一套成熟的折痕引擎：`ConversationLayoutPolicy` 读 `display.getCurrentFoldCreaseRegion()`，区分合盖、悬停、单折展开、双折展开、三折展开、无折痕宽屏六档，`FilePreviewPlacementPolicy` 还能让预览分界对齐第二道折痕。

Sheet 面用的是一个布尔：

```ts
// AppShell.ets:203
private shouldUseWideSheetLayout(): boolean {
  return this.useWideLayout;
}
```

由此产生三个实际缺陷。

### 1. 540vp 宽度地板在折叠屏上必然溢出

```ts
// AppShell.ets:175
private settingsSheetWidth(): number {
  return Math.min(680, Math.max(540, Math.round(this.shellWidth * 0.48)));
}
```

`0.48` 的比例本身没问题：平板 1440vp 得 680，占 47%。出事的是 `540` 这个下限。Mate X7 展开态约 630–740vp，比例算出来是 302–355，被地板顶到 540 —— 占窗口 73%–86%，配合 `wideSheetHeight()` 的 `Math.max(560, h - 80)` 再占掉约 90% 的高度。一个「设置面板」变成了近乎全屏的遮挡。

**任何最小宽度在折叠屏上都是一个待触发的跨铰链缺陷。** 折痕围出来的物理叶片宽度不受设计意愿支配，视觉最佳宽度必须让位给物理边界。

### 2. `SheetType.CENTER` 把内容居中到铰链上

会话面花了很大力气让 master/detail 分界对齐折痕、让点击热区避开折痕安全区。Sheet 面则把一个 540vp 宽的面板**居中**到一块中间有铰链的屏幕上 —— 正好让分割线穿过面板正中。

这不是设置页独有的。现存 4 个函数、5 个 Sheet 全部如此：

```
AppShell.ets:167          wideCenterSheetOptions          → 设置 Sheet + 连接 Sheet
AppSidebar.ets:678        sessionDetailsSheetOptions      → 560 × 560
RemoteSessionList.ets:750 sessionDetailsSheetOptions      → 560 × 560
AppRootPresentation:452   remoteViewSettingsSheetOptions  → 560 × 520
```

五个 Sheet 的宽度都 ≥ 540vp，都居中，在任何双折/三折展开态上都骑折痕。

（`AppSidebar.ets:668` 与 `RemoteSessionList.ets:740` 的 `sessionActionSheetOptions` 是 380/300 的普通底部 Sheet，不在此列。）

### 3. 悬停态的底部 Sheet 横跨折痕

`useHoverOperate()` 判定悬停后 `largeScreenLayout` 为 false，Sheet 走 `bottomSheetOptions()` 的 `SheetSize.LARGE`（`AppShell.ets:159`），从屏幕底部一直铺到顶部，横穿那道横向折痕。

会话面对同一问题已有解：

```ts
// ConversationView.ets:138-155
FolderStack() {
  Column() { this.ConversationBody() }   // 第一个子节点 → 上半 display
  Column() { this.Composer() }           // 第二个子节点 → 下半 operate
}
.alignContent(Alignment.Bottom).enableAnimation(true).autoHalfFold(true)
```

上半显示、下半操作。Settings 全部是点击、选择、进二级页，是彻头彻尾的 operate 内容，应该落在下半屏。但那个 `FolderStack` 长在 `ConversationView` 里，而 Settings 是 `AppShell` 根上的 `bindSheet`，在它外面。

## 目标

1. 所有覆盖式 Sheet 的放置由**折痕数量与折叠姿态**决定，不由单一宽度布尔决定。
2. 存在可用折痕时，Sheet 完整落在一块物理叶片内，不跨铰链。
3. 悬停态 Sheet 完整落在 operate（下半）区，不横跨折痕。
4. 放置决策收敛到一个可单测的纯 Policy，四处调用点共用同一个实现。
5. 折叠形态切换时，Sheet 的当前子页与草稿不丢失。

## 非目标

- 不建注入式 `AdaptiveLayoutContext`。布局上下文是 Policy 的**入参值对象**，组件永远拿不到它。理由见「设计 / 为什么不注入」。
- 不引入 `safeAreaInsets` 字段。`bindSheet` 与 `FolderStack` 自行处理安全区，当前无消费者；每多一个字段就多一个重建触发源。
- 不为「物理 pane 小到放不下最低内容布局」设降级档位。720vp 三折的右叶约 352vp，与手机宽度同量级，现有紧凑行布局原样可用，加了是死代码。
- 不把 Settings 塞进 `ConversationView` 的 `FolderStack`，也不为此重排根布局。
- 不在本设计内改 `ConversationLayoutPolicy.useMasterDetail()` 的六参签名。参数摊平确实是技术债，但收编它会把会话布局的回归风险带进 Sheet 改动。等新 Policy 稳定后单独处理。
- 不改会话 master-detail 几何、文件预览三栏分界、单屏抽屉动画。

## 当前逻辑

### 折痕探测已经集中，不需要重建

一个常见的误判是「折痕探测分散在各处，需要统一」。实际上探测只在一处：

```
AppRootPresentation.currentVerticalCreases()   // :416
  └─ ConversationLayoutPolicy.verticalCreasesFromRects()   // :124
```

`ConversationLayoutPolicy` 和 `FilePreviewPlacementPolicy` 都是接收参数的纯函数，没有各自读 display。**分叉不在探测层。**

真正的问题是参数摊平：

```ts
useMasterDetail(viewportWidth, mediaQueryMatched, isFolded, creases, isExpandedFoldable, isHover)
```

六个位置参数，中间四个是布尔和数组。再加一个 `SettingsPlacementPolicy` 就是第三份同样的实参列表，任何一处传反编译器都不会报错。抽入参值对象的价值在这里。

### `AppShell` 拿不到几何

```ts
// AppRootPresentation.ets:113
AppShell({ shellState: this.shellState, useWideLayout: this.isWideLayout(), ... })
```

`AppShell` 只收到一个布尔。`viewportWidth`、`verticalCreases`、`foldStatus`、`isHoverLayout()` 全部停在 `AppRootPresentation` 里没往下传。

### 没有真正的横折痕概念

```ts
// ConversationLayoutPolicy.ets:143-145
if (!spansViewportWidth && spansViewportHeight) {
  mapped.push(new ConversationLayoutCrease(rect.top, rect.height));
}
```

横向 rect 的 `top/height` 被塞进同一个「纵向折痕」数组，随后交给按宽度过滤的 `visibleCreases()`。这是一段旋转补偿启发式，不是横折痕支持。要算 operate 区高度必须拆出独立通路。

另外 `currentVerticalCreases()` 在合盖或悬停时直接返回 `[]`（`:417`），所以横折痕不能挂在同一个早退分支下。

## 设计

### 分档矩阵

| 环境 | 形态 | 尺寸来源 |
| --- | --- | --- |
| 窄屏竖屏 / 合盖 | Bottom Sheet | 现状 `SheetSize.LARGE` |
| 窄屏横屏 | Side Sheet | 约 45% |
| 平板宽屏，无折痕 | Side Sheet | `clamp(400, 0.32w, 520)` |
| 双折展开，一道纵向折痕 | Side Sheet | `w - c1.left - c1.width`，**无下限** |
| 三折展开，两道纵向折痕 | Side Sheet | `w - c2.left - c2.width`，**无下限** |
| 悬停 / 半折 | Bottom Sheet | `maxHeight = h - (横折痕.top + 横折痕.height)` |

窄屏横屏固定 45%：横屏是唯一高度稀缺的场景，宽度多给没有收益。

### 入参值对象

新增 `pages/policy/AdaptiveLayoutInput.ets`：

```ts
export class AdaptiveLayoutInput {
  readonly viewportWidth: number;
  readonly viewportHeight: number;
  readonly foldStatus: number;
  readonly isFolded: boolean;
  readonly isExpandedFoldable: boolean;
  readonly isHoverOperate: boolean;
  readonly wideLayoutMatched: boolean;
  readonly verticalCreases: ConversationLayoutCrease[];
  readonly horizontalCreases: ConversationLayoutCrease[];
}
```

由 `AppRootPresentation.refreshWideGeometry()`（`:354`）一次构造，喂给 Policy，**只把解析结果往下传**。

#### 为什么不注入

ComponentV2 里把类实例作为 `@Param` 广泛注入有两个后果：

1. 字段级变更不触发重渲染，必须整体替换对象，任何遗漏都是静默失效。
2. 过度失效。一个含 `safeAreaInsets` 的上下文会让键盘弹起重建所有持有它的组件。

本仓库已经吃过一次这个亏：

```ts
// AppShell.ets:26-32
// ... this copy stayed mounted behind an opacity of 0 — invisible, but
// rebuilt on every selection change, which on a tablet doubled the cost
// of switching sessions.
```

所以：上下文是 Policy 的入参，不是组件的注入上下文。

### 放置 Policy

新增 `pages/policy/SettingsPlacementPolicy.ets`，沿用 `FilePreviewPlacementPolicy` 的房规（枚举 + 全字段 readonly 类）：

```ts
export enum SettingsPlacementMode { Bottom, Side, FoldOperate }

export enum SettingsSheetKind { Settings, Connect, SessionDetails, RemoteViewSettings }

export class SettingsPlacement {
  readonly mode: SettingsPlacementMode;
  readonly width: number;      // Side
  readonly height: number;
  readonly maxHeight: number;  // FoldOperate
}

static resolve(input: AdaptiveLayoutInput, kind: SettingsSheetKind): SettingsPlacement
```

ArkTS 不支持 TS 判别联合（`{ mode: 'side'; width: number } | { mode: 'bottom' }`），也不支持推断类型的对象字面量入参，因此不采用那种签名。

Policy 不引用任何 ArkUI 类型，`SheetOptions` 的组装留在组件里。

### 悬停态：限高，不改 FolderStack

让 Settings 落进 `ConversationView` 的 `FolderStack` operate 半区，要么把全局 Settings 塞进会话组件（层级反了），要么为一个瞬态面板重排根布局。

等效且低一个量级的做法：**底部 Sheet 不动，把高度压进 operate 区。**

`height ≤ viewportHeight - (横折痕.top + 横折痕.height)` 的底部 Sheet 物理上就完整落在下半屏，上半屏会话原样保留。视觉结果与 FolderStack 方案一致，实现只是一个高度数字。

这也是 `horizontalCreases` 字段存在的唯一理由 —— 不是用来识别悬停态（`useHoverOperate()` 已经解决），而是用来算 operate 区高度。

## 实施顺序

```
T0 ─┬─→ T1 ─┬─→ T2
    │       └─→ T3（需 T3.1 先落地）
    └───────────→ T4（T2 / T3 各自的验收门）
T5 独立
```

### T0：止血，设置与连接 Sheet 不再骑折痕

- `AppShell` 新增 `@Param viewportCreaseLeft: number` / `@Param viewportCreaseWidth: number`（0 表示无折痕），由 `AppRootPresentation.ets:113` 从第一道可用纵向折痕传入。
- 宽度：有折痕 → `shellWidth - creaseLeft - creaseWidth`，**不套最小宽度**；无折痕 → `clamp(400, 0.32 × shellWidth, 520)`。
- `WIDE_SETTINGS_SHEET_MAX_WIDTH` / `WIDE_CONNECT_SHEET_MAX_WIDTH` / `WIDE_SHEET_MIN_WIDTH` 三个常量退役。
- **暂不动 `SheetType.CENTER`**。宽度等于叶片宽度后，居中造成的偏差只剩半个铰链宽（8vp 量级），不再劈内容。SIDE 留到 T2 统一换。

可独立提交、独立上真机。

### T1：Policy 与入参对象

1. `AdaptiveLayoutInput.ets`。
2. `SettingsPlacementPolicy.ets`，T0 的算式原样搬入，补齐窄屏横屏与悬停两档。
3. `AppShell` 删除 `shouldUseWideSheetLayout()`（`:203`）、`settingsSheetWidth()`（`:175`）、`connectSheetWidth()`（`:185`）、`wideSheetHeight()`（`:192`）及相关常量，改收 `@Param placement: SettingsPlacement`，整体替换。
4. 单测见「测试要求」。

### T2：`SheetType.CENTER` → `SheetType.SIDE`，四处统一

- Policy 返回 `Side` 时，四处 `SheetOptions` 一并换 `preferType: SheetType.SIDE`。
- `AppSidebar.ets:678` 与 `RemoteSessionList.ets:750` 的两份 `sessionDetailsSheetOptions` 是重复代码，合并为共用实现。
- `AppRootPresentation.ets:452` 的 560×520 同样接入。
- `SheetType.SIDE` 为 `@since 20`，项目 `compatibleSdkVersion 6.0.1(21)` 满足。真机需确认贴边方向；不要把 `right` 写死，为将来的 RTL 留出口。

### T3：悬停态限高

1. **T3.1** `ConversationLayoutPolicy` 新增 `horizontalCreasesFromRects()`，与纵向通路分离。倾向新建一个横向折痕类而非复用 `ConversationLayoutCrease`：复用会让 `visibleCreases()` 这类按宽度过滤的工具在错误的轴上被误用。
2. **T3.2** `AppRootPresentation` 用独立方法暴露横折痕，不受 `currentVerticalCreases():417` 悬停早退分支影响。
3. **T3.3** Policy 返回 `FoldOperate` 与 `maxHeight`，`AppShell.bottomSheetOptions()` 用它替掉 `SheetSize.LARGE`。

### T4：形态切换连续性

见「风险与未知」。这不是一个独立步骤，而是 T2 与 T3 各自必须通过的验收门。

### T5：信息架构收敛（纯视觉，可任意时机插入）

- 设置分三组：账号 / 通用（语言 + 模型）/ 关于。
- `settings.modelService.section` 由「普通对话」改为「通用」，消除「普通对话 → 模型」的两跳标签。同步改 `ZhCnMessages` / `EnUsMessages`，`I18nUnit.test.ets` 的键对齐测试兜底。
- 关闭按钮从 50×50 带阴影降为 40×40 无阴影（`SettingsSheet.ets:109-124`）。
- **不需要**移除「个人资料」分组标题 —— 该标题本就不存在，`SettingsHome()` 中 `AccountEntry()`（`:91`）直接跟在大标题之后。

## 风险与未知

### 形态切换时 `preferType` 变更的行为未知

用户在展开态打开 SIDE Sheet，随后合盖：`largeScreenLayout` 翻转，placement 由 `Side` 变 `Bottom`。`bindSheet` 的 `preferType` 在已呈现状态下变更，可能闪烁、可能保持旧形态、可能直接关闭。**当前无任何依据可以预判。**

这不是理论风险。`wide-conversation-navigation-design.md` 的「待验证」已经挂着「三屏与双屏之间的真实折叠切换连续性」，会话层自己尚未验证；Sheet 层引入同样的形态切换，是同一个洞的第二个入口。

必须在真机上验三条：

1. 展开态 SIDE Sheet → 合盖。
2. 悬停态限高 Sheet → 完全展开。
3. 双折 ↔ 三折切换。

若观察到闪烁或残留，回退方案是**形态切换时先关闭 Sheet、再按新形态重开**，代价是必须把状态提升。该决定等真机结果，不预设。

### Sheet 状态在重建时会丢

`SettingsSheet.@Local page`（`:49`）与三个 `savedGeneralChat*`（`:50-52`）都是组件内状态。Sheet 一旦重建，用户当前停在概览 / 账号列表 / 本机编辑器哪一页、以及本机编辑器里的草稿，全部丢失。

无论 T4 的结论如何，这两项都必须在形态切换后保留，作为 T1 的验收项，不等真机撞出来再补。

## 验收矩阵

| 场景 | 预期结果 |
| --- | --- |
| 窄屏竖屏，打开设置 | 与改动前完全一致的底部 Sheet |
| 窄屏横屏，打开设置 | 侧边 Sheet，约 45% 宽 |
| 平板无折痕（MatePad Pro 1440vp） | 侧边 Sheet，461vp，比改前的 680 窄 |
| 双折展开（Mate X7），打开设置 | Sheet 左缘 ≥ 折痕右缘，完整落在右叶 |
| 双折展开，打开连接 Sheet | 同上 |
| 双折展开，打开会话详情 | 同上，不再是居中的 560×560 |
| 双折展开，打开远程视图设置 | 同上，不再是居中的 560×520 |
| 三折完整展开（两道折痕） | Sheet 完整落在第二道折痕之后的右屏 |
| 悬停态，打开设置 | Sheet 完整落在横折痕下方 operate 区，上半会话可见 |
| 折叠单屏，全部 Sheet | 与改动前完全一致 |
| 展开态开着设置 → 合盖 | 不闪烁、不残留；当前子页与草稿保留 |
| 悬停态开着设置 → 展开 | 同上 |
| 双折 ↔ 三折切换，Sheet 开启中 | 同上 |
| 折痕数据缺失 / 非法 | 回退到无折痕宽屏档，不阻断渲染 |

## 测试要求

### 自动化

在 `ArchitectureUnit.test.ets` 为 `SettingsPlacementPolicy` 增加纯逻辑测试，覆盖八档：

1. 窄屏竖屏
2. 窄屏横屏
3. 无折痕宽屏（平板）
4. 一道纵向折痕
5. 两道纵向折痕
6. 折痕偏到极端位置（叶片极窄）
7. 折痕数据非法（负宽、越界、坐标在内容区外）
8. 悬停态（横折痕）

对齐现有 `ConversationLayoutPolicy` 的标准：新增行、函数、分支覆盖率均为 100%。

另需断言：任一档位下 Policy 的返回宽度都不与折痕区间相交。

### 构建

```bash
source scripts/ohos-env.sh
"$OHPM" install
"$HVIGORW" --mode module -p module=entry assembleHap --no-daemon
```

### 真机手工验证

在 HUAWEI Mate X7（`DEL-AL10`）覆盖双折展开、折叠单屏、悬停三态；在 HUAWEI MatePad Pro（`WEB-W00`）覆盖无折痕宽屏。三折两道折痕的完整展开态目前无对应真机，与 `wide-conversation-navigation-design.md` 的同一项一并挂起。

浅色与深色均需验证。

## 完成标准

只有同时满足以下条件才视为完成：

- 五个 Sheet 全部通过同一个 `SettingsPlacementPolicy` 决定放置。
- `AppShell.shouldUseWideSheetLayout()` 及三个宽度常量已删除。
- 存在可用折痕时，任一 Sheet 的边界都不与折痕区间相交。
- 悬停态 Sheet 完整落在 operate 区。
- 折叠形态切换时 Sheet 的当前子页与草稿不丢失。
- 窄屏与折叠单屏验收矩阵全部与改动前一致。
- 八档单测通过，覆盖率达标；HarmonyOS 构建通过；两台真机手工验证有记录。
