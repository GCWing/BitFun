# 主题与颜色 Token 优化方案

> 当前治理基线以 `scripts/theme-color-governance-baseline.json`、
> `scripts/theme-color-governance-baseline.mobile-web.json`、
> `scripts/theme-color-governance-baseline.installer.json`、
> `scripts/theme-color-governance-baseline.cli.json` 和审计脚本输出为准。

本文档用于梳理 BitFun 前端主题、硬编码颜色、重复 token、近似色冗余、
命名漂移和后续治理方案。目标不是把所有看起来相近的颜色都合并，而是让
每一个颜色都能追溯到明确的语义角色，并保留那些会帮助用户区分区域、状态、
层级或数据含义的视觉差异。

## 范围

本方案覆盖：

- `src/web-ui` 中的主题预设、运行时 CSS 变量注入和共享样式 token。
- `src/web-ui/src/component-library/styles` 下的 token 定义。
- `src/mobile-web` 中的独立 mobile theme preset、运行时 CSS 变量和移动端组件样式。
- `BitFun-Installer/src` 中的安装器主题数据、运行时变量注入、静态变量和安装流程组件样式。
- `src/apps/cli` 中的 CLI/TUI 主题 preset JSON、terminal ANSI 适配和外部主题兼容 fallback。
- 组件 SCSS/CSS/TSX 中的硬编码颜色、fallback 色值和局部 token。
- Rust/desktop 侧由 TS 主题预设生成的最小投影 manifest。
- 旧 token 名称到新规范名称的兼容别名。
- 后续防止新增硬编码颜色的审计和约束规则。

## 治理原则

本次优化的方向需要同时满足两点：

- 色值数量要尽可能收敛。一个应用的基础色值不应该无限增长，后续需要用
  明确的预算上限来约束 palette、semantic token 和 component token。
- 合并必须有依据。除非两个颜色已经极其相似、肉眼基本不可区分，否则不能只
  因为“看起来接近”就合并；需要说明它们为什么是同一角色、为什么不会破坏
  区域区分、状态区分或数据含义。

建议把颜色分成三类预算：

| 类型 | 建议上限 | 说明 |
| --- | ---: | --- |
| Primitive palette | 80-120 | 包含核心 hue、neutral、alpha ramp；主题预设可以映射，但不应无限扩张。 |
| App semantic token | 40-70 | 覆盖背景、文本、边框、状态、交互和 app intent。 |
| Component token | 每个复杂 surface 8-20 | 只在 semantic token 不足以表达组件契约时添加。 |

预算不是为了追求某个机械数字，而是为了阻止“每个组件随手新增一个色值”。
新颜色必须进入以下流程之一：

1. 能映射到现有 token：直接复用，不新增色值。
2. 肉眼不可区分：合并到已有色值，并记录为直接合并。
3. 有独立语义：新增 semantic 或 component token，并说明为什么不能复用。
4. 属于 editor、terminal、syntax、diff 等专用域：进入 exception namespace。

第一阶段不覆盖：

- 重新设计品牌视觉方向或重做主题风格。
- 强行替换 Monaco editor、terminal ANSI、Mermaid、语法高亮或第三方内容
  的专用色板。
- 对每个页面做像素级重设计。
- 在所有调用方迁移完成前移除兼容别名。

## 主题扩展职责边界

主题扩展必须把“持久化/首屏引导”和“完整渲染契约”拆开维护，避免 Rust 与
TS 各自增长一套主题定义。参考经验：

- [OpenCode theme](https://opencode.ai/docs/themes/) 采用可扩展 JSON schema、内置/用户/项目多层加载、
  可复用色值定义和 light/dark variant；扩展点贴近 TUI 渲染层，host 只负责加载顺序和选择。
- [Claude Code theme](https://code.claude.com/docs/en/terminal-config) 通过 `/theme`、theme picker 和 auto
  light/dark 匹配终端背景，同时明确不接管终端自身配色；host 只适配环境，不复制完整 palette。
- [Codex config](https://developers.openai.com/codex/config-basic) 使用分层 config 和明确优先级；
  对 BitFun 的启发是：Rust/host 可以读取偏好与可信配置层，但不应成为 UI token 语义 owner。

OpenCode 的 agent/plugin 色彩入口保持为小型语义集合：
`primary`、`secondary`、`accent`、`success`、`warning`、`error`、`info`。
BitFun 后续适配插件体系时也必须遵循同一边界：插件消费 TS 侧生成的 compact projection，
不得直接读取完整 `ThemeConfig`、runtime CSS var 列表或 generated widget payload allowlist。
当前 `src/web-ui/src/infrastructure/theme/pluginThemeProjection.ts` 是插件语义色映射入口，
只把完整主题投影成 7 个外部可扩展 key；如插件未来确实需要 surface/text/border，
应新增另一个小型 projection，并把 key 上限和语义写入本文档与审计，而不是扩大 Rust 或插件侧自由 key。

当前职责边界如下：

| 事项 | Rust/desktop 侧 | TS/web-ui 侧 |
| --- | --- | --- |
| 主题选择持久化 | 启动时只读取 `themes.current`，解析 `system`、内置主题 id 和未知值回退；历史 `theme.id` 只在配置加载/导入时一次性归一到 `themes.current`，不能作为新扩展入口。 | 读写 `themes.current`，处理 `system`、内置主题和 custom theme 选择。 |
| 完整主题契约 | 不维护完整 `ThemeConfig`、semantic token、component token 或专用 palette；旧 Rust GUI theme struct/provider 已退役，默认导出不得出现顶层 `theme`。 | 拥有 `ThemeConfig`、主题预设、validator、import/export、runtime CSS 变量注入和审计 registry。 |
| 首屏 bootstrap | 只注入 WebView 首屏所需最小投影：`data-theme`、`data-theme-type`、核心背景/文本 CSS 变量和 `--bitfun-startup-bg`。 | JS 启动后必须重新应用完整主题，覆盖 bootstrap 投影并恢复 Monaco、Mermaid、terminal、widget payload 等专用域。 |
| 生成式 UI 主题提示 | 只读取 TS 预设生成的 prompt snapshot manifest，用于模型提示；不得手写内置主题 palette。 | 拥有 prompt snapshot 投影规则；新内置主题加入 `builtinThemes` 后由生成器同步到 Rust 只读 manifest。 |
| 插件主题色扩展 | 不维护插件色彩 schema，不把 plugin color 解析放到 Rust/desktop。 | 拥有 `pluginThemeProjection.ts`，把完整主题映射到 OpenCode-compatible 的 7 个语义色 key；插件不得直接依赖内部 CSS var 全量表。 |
| 内置主题扩展 | 不手写新增完整 palette。只有新内置主题需要首屏无闪烁时，才更新最小 bootstrap 投影。 | 新主题先进入 TS 预设和 `builtinThemes`；所有语义、组件和专用域 token 以 TS 侧为准。 |
| custom theme | 不解析 `themes.custom`，不复制 custom schema；保存的 custom id 在 Rust 启动阶段不可用时使用系统/默认首屏回退。 | 加载、校验、注册、注销、导入导出 custom theme；custom 加载完成后覆盖 Rust 首屏回退。 |
| CSS 变量和 key 命名 | 只允许新增明确写入启动主题投影 manifest 的首屏 key；不得新增 backend theme service 或平行 alias 表。 | 新 primitive/semantic/component key、兼容 alias、surface rename、dynamic family 和 widget payload key 均在 TS contract/audit 中登记。 |
| 专用渲染域 | Core/web host 不维护 Monaco、Mermaid、Prism、language identity、UI exception 等 web-ui 色板；CLI/TUI preset 和 terminal ANSI 是独立终端 surface，不能成为 web-ui 主题源头。 | web-ui 专用域由各自 TS owner 维护，按 `colorDomainScopes.*` 和 `colorDomainNearPairs.*` 预算治理。 |

Mobile Web 和 Installer 是独立产品形态，但主题扩展位置仍在各自 TS 侧维护：
`src/mobile-web/src/theme/presets` 拥有 mobile runtime key，`BitFun-Installer/src/theme`
和 `BitFun-Installer/src/styles/variables.css` 拥有安装器运行时与首屏静态变量。
Rust 侧不得因为安装器需要编译或首屏显示而复制完整 palette；安装器 Rust 只处理安装流程、
API/DTO 映射和必要的启动壳逻辑。跨 root 动态 key 必须在 `scripts/theme-css-var-contract.mjs`
登记 owner，审计脚本按 root owner 判断 stale dynamic family，避免 web-ui、mobile-web 和
installer 互相误报或重复定义。

CLI/TUI 是独立终端产品 surface，不属于 web-ui 主题源，也不是 desktop/backend 主题 owner。
`src/apps/cli/themes/presets/*.json` 拥有 CLI preset 数据，`src/apps/cli/src/ui/theme.rs`
只负责 ANSI/monochrome 降级适配、OpenCode-compatible preset JSON 的最小解析和运行时投影，
不拥有 web-ui `ThemeConfig`。跨 web-ui、installer、CLI 的近似色可以在语义一致且不会影响相邻状态区分时手动收敛，
但不能通过让 Rust/CLI 复制 web-ui `ThemeConfig` 来解决；若后续需要共享，只能新增明确的 TS 生成投影或 CLI schema
contract，并纳入审计基线。

当前 Rust 启动主题投影已改为读取 TS 内置主题预设生成的
`src/apps/desktop/src/generated/startup_theme_bootstrap.json`。Rust 侧只持有
`id`、`bgPrimary`、`bgSecondary`、`bgScene`、`isLight`、`textPrimary`、
`textMuted`、`accentColor` 这类首屏字段，不维护完整主题 schema 或 palette。
Rust 配置层只保留一个独立的历史兼容 fallback：加载、导入或旧调用路径遇到
`theme.id` 时映射到 `themes.current`，并在保存时移除顶层 `theme`；其他旧 GUI theme
字段不再解析、不再校验，也不再作为 public theme contract 暴露。
`GenerativeUITool` 的内置主题提示也必须读取同一生成链路产出的
`src/crates/assembly/core/src/agentic/tools/implementations/generated/theme_prompt_snapshots.json`，
避免 Rust 再手写一套 web-ui 主题色。两个 manifest 都只是 TS 主题预设的只读投影：
startup manifest 面向 WebView 首屏，prompt snapshot 面向模型提示，均不能反向成为主题源头。
生成器 check 需要对工作树行尾做归一化，避免 Windows checkout 把 LF/CRLF 差异误判为
manifest stale。

新增或扩展主题时按以下顺序执行：

1. 在 TS 侧更新 `ThemeConfig`、主题预设、validator 和 runtime 注入；需要外部 iframe 读取时同步
   `themePayload` allowlist 与 CSS var contract。
2. 对新增色值先走 token 预算：可复用则复用，肉眼不可区分才直接合并，有独立语义才新增
   semantic/component token，专用域进入 exception namespace。
3. 只有影响 JS 加载前首屏的字段，才更新 Rust bootstrap 投影；不能为了 Rust 读取方便新增第二套主题 schema。
4. custom theme 不要求 Rust 首屏精确还原。若后续要优化 custom theme 首屏体验，也必须由 TS schema 生成
   最小 bootstrap cache，而不是让 Rust 直接解析 custom theme。
5. 变更必须通过主题审计、CSS var contract 和必要的 focused visual review；跨 desktop/web/mobile 或
   light/dark/system 形态的行为由 TS 运行时统一验证。

主题作者不应手写完整派生色阶。内置主题和 custom theme 的长期方向是只维护少量稳定输入：
primary/accent 基色、secondary accent 基色、success/warning/error/info 等语义基色，以及确实需要保留
主题个性的少量 override。accent scale、secondary accent scale、semantic bg/border、git/runtime 派生
和 CSS var 注入都必须由 TS 侧 resolver/helper 统一生成；Rust/desktop 只消费 TS 生成的最小投影，不复制
这些派生规则。

组件样式中的颜色消费应优先读取 runtime CSS var。`tokens.scss` 可以保留作为 root token 定义层、尺寸/
字体/动效 SCSS 常量和少量历史 mixin，但普通组件、工具页和面板不应直接读取 `$color-*`、
`$element-bg-*`、`$border-*` 或 `$git-color-*` 来表达产品颜色。独立定义域，例如 Mermaid theme、
Monaco/editor、terminal 和 syntax palette，可以保留编译期 token，但必须有 namespace owner 和审计预算。
这条规则避免新主题扩展时出现“TS runtime 已变更，但 Sass 构建期颜色仍停留在默认主题”的双轨问题。

## 当前现状

基于当前审计口径，普通
app/component 层的 raw color literal、token-equivalent app literal、普通组件 near color pair
和内部旧 alias 读取都已收敛到 0。剩余色值全部落在明确 owner 的专用域：
theme preset/runtime、token contract、boundary fallback、
Mermaid、Monaco/editor、Prism syntax、terminal ANSI、language identity 和 UI exception
registry。Mobile Web 和 Installer 已纳入同一审计口径，但各自使用独立 baseline，
避免移动端或安装器的独立 token 被误算为 web-ui 游离 key。

`342` 个 web-ui 唯一颜色是前端生产文件的全域审计数，不是普通 app UI 的色值预算。
其中包含主题 preset、token contract、Mermaid、Monaco/editor、terminal、syntax、
language identity 和 UI exception 等专用 palette。language identity 已收敛为 8 个大类身份色，
不再按每种语言或文件类型保留独立色值。真正需要继续压缩的是这些专用域
内部能被证明等价的近似色，而不是把它们直接并入普通 app semantic token。前后端职责边界
已经收敛为：web-ui/TS 侧维护完整主题源；Rust/desktop 侧只读取 TS 生成的首屏和
prompt snapshot 投影；mobile-web 和 installer 颜色在各自 TS root 中维护；CLI/TUI
颜色作为独立终端产品 surface 单独治理。

language/file identity 色只允许作为类别辅助 accent。消费方展示语言或文件身份时必须同时渲染
label、icon、扩展名或文件名之一，不允许构建只靠颜色区分语言或文件类型的 UI。

补充看 resolved theme 输出而不是只看源码字面量：相邻状态、主题个性和 elevation
强度不能只按数值近似强行合并。复审后暗色主题 `effects.shadow` 保留各 preset 的
原有 ramp；本轮只压缩非相邻、非语义的 surface/token 微差。

`colorScopes.exception`、`colorDomainScopes.uiException` 和 `colorDomainScopes.boundaryFallback`
的数值上升不是新增游离色，而是把原先散在 service/component 文件中的身份色、review team
角色色、Prism palette、截图兜底色和 Monaco theme palette 归入显式 owner 后的结果。

| 指标 | 当前基线 |
| --- | ---: |
| 扫描的生产前端文件数 | 1543 |
| 忽略的测试文件数 | 224 |
| 忽略的构建生成文件数 | 0 |
| 包含颜色字面量的文件数 | 25 |
| 颜色字面量出现次数 | 535 |
| 唯一颜色字面量数量 | 342 |
| 组件或非 token 文件中的颜色出现次数 | 0 |
| 组件或非 token 唯一颜色数量 | 0 |
| App UI 颜色出现次数 | 0 |
| App UI 唯一颜色数量 | 0 |
| `var(--token, fallback)` 出现次数 | 0 |
| fallback 唯一 token 数 | 0 |
| token-equivalent app literal 出现次数 | 0 |
| token-equivalent app literal 唯一颜色数量 | 0 |
| 普通组件肉眼不可区分 near color pair | 0 |
| 普通组件需证据复核的 near color pair | 0 |
| 专用域肉眼不可区分 near color pair | 0 |
| 专用域需证据复核的 near color pair | 9 |

跨 root 基线必须分别看待；这些数字不是互相累加后的单一上限：

| root | 颜色出现次数 | 唯一颜色 | App UI raw | fallback var | unresolved / non-contract key | dynamic family | 说明 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `src/web-ui/src` | 535 | 342 | 0 | 0 | 0 | 13 | 主应用完整主题、专用 palette、widget payload 和 editor/terminal/Mermaid 域；UI exception、syntax、language identity 与 boundary fallback 已收敛到小型语义 palette，未读取的 git 同义 runtime/static key 和未消费 legacy mixin 已退役。 |
| `src/mobile-web/src` | 35 | 30 | 0 | 0 | 0 | 3 | mobile-web 图片缩略图关闭按钮已读取 `--color-static-white`，普通 app UI raw 归零。 |
| `BitFun-Installer/src` | 76 | 62 | 0 | 0 | 0 | 1 | 安装器主题数据保留主题卡可见的 primary/secondary background、accent、text 和状态反馈；runtime 只导出实际消费的 accent family、基础 text/border/element/status key，不复制主应用 purple/info/tooltip 等完整主题面。 |

CLI/TUI 使用独立审计，不参与 CSS var root 计数：

| CLI/TUI 指标 | 当前值 |
| --- | ---: |
| preset 文件数 | 6 |
| preset 颜色出现次数 | 306 |
| preset 唯一色数 | 114 |
| runtime-consumed preset 颜色出现次数 | 114 |
| runtime-consumed preset 唯一色数 | 75 |
| OpenCode compatibility-declared preset 颜色出现次数 | 192 |
| OpenCode compatibility-declared preset 唯一色数 | 87 |
| Rust fallback `Color::Rgb` 出现次数 | 0 |
| Rust fallback 唯一色数 | 0 |
| CLI/TUI 总唯一色数 | 114 |
| CLI/TUI runtime 唯一色数 | 75 |
| preset 需证据复核 near pair | 0 |
| runtime-consumed preset 需证据复核 near pair | 0 |
| OpenCode compatibility-declared preset near pair | 0 |
| Rust fallback near pair | 0 |

当前审计未发现 CSS 变量契约层面的硬错误：

| 契约指标 | 当前值 |
| --- | ---: |
| unresolved CSS vars | 0 |
| fallback-only unresolved vars | 0 |
| unregistered dynamic families | 0 |
| stale registered dynamic families | 0 |
| non-contract cross-file vars | 0 |
| non-contract dynamic inputs | 0 |
| non-contract component-private vars | 0 |

审计补充了机器可校验的治理契约，用于把“可删除债务”和“必须保留的兼容/边界”
分开：

| 治理契约指标 | 当前值 | 说明 |
| --- | ---: | --- |
| compatibility alias contracts | 0 | 显式 root/runtime 兼容别名已退役；历史 generated widget 内容通过 iframe fallback 兼容，尺寸旧 family 仅由 iframe fallback 和 family contract 识别 |
| compatibility alias 直接使用 key | 0 | 产品代码不再通过 `var(--legacy-alias)` 读取旧 key；新增直接读取会被 baseline 拦截 |
| compatibility alias 直接使用次数 | 0 | 显式旧 key 不再在 root/runtime 定义，内部样式读取统一使用 canonical token |
| stale compatibility alias contracts | 0 | 防止 registry 重新保留没有必要的静态/runtime 兼容定义 |
| compatibility alias family contracts | 2 | `--radius-* -> --size-radius-*`、`--spacing-* -> --size-gap-*` |
| compatibility alias family 直接使用 key | 0 | `--radius-*`、`--spacing-*` 旧 family 不再被内部 `var()` 读取 |
| compatibility alias family 直接使用次数 | 0 | generated widget payload 只暴露经过审核的 canonical 子集，iframe shell 通过 alias fallback 兼容 legacy family |
| stale compatibility alias family contracts | 0 | 防止 family contract 指向缺失的 canonical family |
| missing compatibility alias family canonicals | 0 | 防止新增 `--radius-x` / `--spacing-x` 但缺少对应 canonical key |
| surface token rename contracts | 9 | 显式记录已迁移的 surface-local 旧 key、canonical 目标、owner 和命名边界 |
| active surface token rename key | 0 | 防止 `--primary-color`、`--operation-color`、`--delay`、`--um-*` 等旧局部 key 回流 |
| active surface token rename occurrences | 0 | 防止旧 key 在 SCSS、CSS 或 TSX inline style 中被重新定义或读取 |
| surface token rename missing canonical | 0 | 防止 rename registry 指向不存在的 canonical key |
| generated widget payload key | 80 | widget iframe 对外主题变量 allowlist，作为外部边界单独预算，不计入内部 alias 读取；payload 只保留需要随宿主主题变化的 canonical 颜色、文本、surface、形状、间距、字体、动效和 button component token 子集，host 内部、静态黑白 overlay、派生 accent/status/border/radius key 或历史兼容 key 由 iframe fallback/static shell 派生 |
| generated widget button payload | 18 | Cyber/Tokyo/light 等主题把 button bg、border、shadow、transform 和 hover/active 作为主题身份与交互反馈的一部分；这些 key 不能仅因 iframe static shell 存在默认值就移除，否则会让生成式 widget 内按钮失去宿主主题视觉层级。后续只有证明所有内置主题 resolved button 输出等价时才可继续缩减 |
| generated widget payload compatibility alias | 0 | payload 不再直接暴露 legacy alias；历史生成内容通过 iframe 内 alias fallback 读取 canonical key |
| generated widget payload compatibility family key | 0 | payload 不直接暴露旧 `--radius-*`、`--spacing-*`；必要的 canonical 尺寸值由 payload 子集或 iframe fallback 保留 |
| generated widget payload external-only compatibility key | 0 | payload 中已清空仅因外部兼容保留且内部产品代码不再读取的 legacy key |
| generated widget payload undefined key | 0 | 防止 payload 引入没有静态、运行时或动态 family 定义的游离 key |
| generated widget payload missing compatibility canonical | 0 | 防止 payload 暴露 legacy key 但 canonical 目标不存在 |
| generated widget payload unexported compatibility canonical | 0 | 防止 payload 暴露 legacy key 但遗漏对应 canonical key |
| fallback token contracts | 0 | 组件 fallback 已通过根 token 或组件根默认值收敛；新增 fallback 会重新要求 owner、reason 和 boundary |
| uncontracted fallback tokens | 0 | 防止新增未解释的 fallback key |
| stale fallback token contracts | 0 | 防止已删除 fallback 继续留在 registry 中 |
| color domain contracts | 13 | 每个专用域都有 owner、reason 和 merge policy |
| active uncontracted color domains | 0 | 防止新增专用域但没有 owner/合并策略 |

跨 root 审计必须使用 `pnpm run theme:color-audit:all`。`theme:color-audit` 只覆盖
`src/web-ui/src`；mobile-web 和 installer 的 raw color、fallback、dynamic family、
unresolved key 和专用域 near pair 需要分别由 `theme:color-audit:mobile` 与
`theme:color-audit:installer` 校验；CLI/TUI preset 与 Rust fallback contract 由
`theme:color-audit:cli` 校验。新增 baseline 只能在债务减少时下调；不得为了让
PR 通过而放宽 `appUi`、fallback、unresolved、non-contract 或 dynamic family 上限。

剩余颜色集中在几个专用域；普通 app UI 不再保留 raw color literal：

| 区域 | 当前出现次数 | 当前唯一色数 | 说明 |
| --- | ---: | ---: | --- |
| Theme presets | 170 | 116 | 主题个性与 palette 映射；跨主题深色 neutral、弱文本、非状态浅色背景和同概念 success 色已收敛；相邻 surface、主题识别主背景、状态色和 editor lineHighlight 继续保留 |
| Token contracts | 108 | 97 | `tokens.scss` 等静态契约根；黑白 overlay alpha stop 继续保留相邻状态层级，未消费 legacy mixin 和自引用别名已移除，不按数值相近强行合并 |
| Editor | 52 | 48 | Monaco/editor 专用域，不能直接泛化到 app token；被动 selection/word highlight 已收敛，但 diff text/line/gutter 继续保留用户可见层级 |
| Mermaid | 82 | 65 | Mermaid 专用渲染域；light status fallback 已改读 app semantic status token，critical/error 背景弱重复已收敛，note 文本保留专用高对比值 |
| Theme runtime | 27 | 26 | `ThemeService.ts` 运行时注入；黑白 overlay alpha 与静态 token、payload shell 保持相同 stop，避免 early render 与 runtime 状态层级漂移 |
| Language identity | 8 | 8 | 语言/文件身份色，已收敛到 8 个大类色；具体识别继续由 key、label、icon 和扩展名承担 |
| Terminal | 37 | 29 | terminal/ANSI 专用域；工具命令空状态已复用 `--tool-command-empty-rgb`，不再保留独立 raw 色 |
| Boundary fallback | 18 | 18 | iframe/miniapp/截图兜底值，不作为普通 app token；generated widget 初始 CSS 需要覆盖 retired alias 的 canonical 目标 |
| Visual effects | 0 | 0 | StreamText/TextStroke raw literal 已迁出普通组件层 |
| UI exception registry | 17 | 17 | 已归档的 UI 例外色，包含 review team、agent capability、template context、insights 和 inspector 等固定身份色；非相邻同语义私有色已归并到小型 exception palette |
| Generated widget | 0 | 0 | 颜色默认值已迁到 boundary fallback registry |
| App UI | 0 | 0 | 普通 app/component raw color 已清零；后续新增必须先进入 token/exception 决策 |
| Syntax | 16 | 16 | Prism syntax palette，已按 foreground、muted/comment、keyword、literal、function、markup 等角色收敛；light punctuation 与 dark tag/property 保留相邻代码可读性差异 |

专用域 near color pair 已单独进入证据队列，避免把 editor、terminal、Mermaid、
theme preset 或 boundary fallback 误算成普通 app UI 债务。下表是 `src/web-ui/src`
root 的当前队列，不是自动合并指令，
而是后续截图和语义复核的候选清单：

| 专用域 | 肉眼不可区分 pair | 需证据复核 pair | 后续处理原则 |
| --- | ---: | ---: | --- |
| Theme presets | 0 | 5 | 保留 Tokyo Night 主题识别主背景、Ink Night secondary/lineHighlight、Monaco light lineHighlight、Midnight lineHighlight/elevation 等需视觉证据的 near pair；不按数值继续强合并 |
| Theme runtime | 0 | 2 | 黑白 overlay runtime stop 虽然数值接近，但直接服务 card、markdown、shadow 等相邻状态；后续只能在有视觉证据时继续压缩 |
| Token contracts | 0 | 2 | 静态 token 黑白 overlay stop 保留与 runtime 同步的状态层级；不能为了清零 near pair 破坏 hover/active/elevated 区分 |
| Boundary fallback | 0 | 0 | first-paint 兜底已收敛到较粗 overlay stop；后续新增必须重新说明边界理由 |
| Mermaid | 0 | 0 | 节点、边、文本、错误态和 light/dark Mermaid fallback 的近似队列已清零 |
| Editor | 0 | 0 | Monaco selection、diff、inline highlight 和 light/dark editor 近似队列已清零 |
| Syntax / Terminal / Generated widget / Debug overlay / UI exception / Language identity / Visual effects | 0 | 0 | 当前无 near 队列；新增会被单域 baseline 拦截 |

专用域 near pair 不是隐式豁免。当前保留项必须在
`scripts/theme-color-near-pair-decisions.json` 中有 root、owner、reason 和 reevaluateWhen；
审计测试会阻止 web-ui、mobile-web 或 installer 新增 near pair 没有决策，
也会阻止已合并 pair 的过期决策继续留在 registry 中。

剩余高频文件均为专用 palette 或集中 registry：

| 文件 | 颜色出现次数 | 后续处理策略 |
| --- | ---: | --- |
| `src/web-ui/src/component-library/styles/tokens.scss` | 95 | 根 token 契约；优先处理同语义 alias，避免把状态/层级 ramp 按数值强合并 |
| `src/web-ui/src/tools/mermaid-editor/theme/mermaidThemeFallbacks.ts` | 62 | Mermaid 专用渲染兜底；需以节点、边、文本、错误态截图为依据 |
| `src/web-ui/src/tools/editor/themes/bitfun-dark.theme.ts` | 46 | Monaco theme palette；不拆散到普通 app token |
| `src/web-ui/src/infrastructure/theme/core/ThemeService.ts` | 27 | 运行时注入；需保持 early render、system theme 和 payload 导出兼容 |
| `src/web-ui/src/tools/terminal/utils/xtermTheme.ts` | 36 | terminal ANSI palette；不与 app semantic color 合并 |
| `src/web-ui/src/tools/mermaid-editor/theme/_tokens.scss` | 20 | Mermaid SCSS token 兜底；只在 Mermaid owner 内治理 |
| `src/web-ui/src/infrastructure/theme/presets/midnight-theme.ts` | 23 | theme preset palette；保留主题个性、状态色和 alpha ramp 边界 |
| `src/web-ui/src/infrastructure/theme/presets/slate-theme.ts` | 22 | theme preset palette；保留主题个性、状态色和 alpha ramp 边界 |
| `src/web-ui/src/shared/theme/themeBoundaryFallbacks.ts` | 18 | iframe/miniapp/截图兜底 palette；只作为边界 fallback 治理 |
| `src/web-ui/src/shared/theme/uiExceptionAccents.ts` | 17 | 固定 UI 身份/角色色 registry；新增必须说明 owner/role |

组件级 `var(--token, fallback)` 已收敛到 0；原先的 7 个 fallback token 不再需要
fallback contract registry 保留。

fallback 收敛决策表：

| 原 fallback token | 决策 | 依据 | 结果 |
| --- | --- | --- | --- |
| `--surface-stagger-index` | 上移默认值 | `tokens.scss` 已提供 `0` 默认值，TS inline style 仍可覆盖动画序号 | 移除 12 处 selector fallback |
| `--mission-control-group-color` | 上移默认值并保留组别差异 | filter 仍由 inline style 驱动；thumbnail badge 的 primary/secondary/tertiary 默认值保持原 accent/success/warning 语义 | 移除 6 处背景 fallback，避免误把组别统一成同一颜色 |
| `--char-index` | 上移到组件根 | StreamText 根提供 `0`，每字符 inline style 仍可覆盖 | 移除 3 处 keyframe fallback |
| `--gallery-grid-min` | 上移到根 token 默认值 | `tokens.scss` 提供 `320px`，祖先变量和 props inline style 仍可覆盖 | 移除 grid sizing fallback |
| `--gallery-skeleton-height` | 上移到根 token 默认值 | `tokens.scss` 提供 `140px`，祖先变量和 props inline style 仍可覆盖 | 移除 skeleton height fallback |
| `--primary-color` | 改为明确的 tool-card accent token | `--primary-color` 是历史 tool card 局部入口，不提升为全局 app token；BaseToolCard 现在通过 `--base-tool-card-accent-color` 映射到 `--markdown-primary-color` | 产品代码不再定义或读取旧 key，回流由 `surfaceTokenRenames` 拦截 |
| `--scene-viewport-border-width` | 上移默认值 | 静态 token 提供 `1px`，ThemeService 继续按主题 layout 覆盖为 `1px` 或 `0` | 移除 viewport border fallback |

稳定里程碑：

| 里程碑 | 稳定要求 |
| --- | --- |
| 基线与工具 | 审计脚本必须持续区分测试文件、fallback token、dynamic family、exception domain 和 generated widget 外部兼容面。 |
| canonical token 契约 | 内部调用方只读 canonical token；root/runtime 显式 compatibility alias 保持 0。确需外部兼容时只放在对应边界，如 generated widget iframe fallback 或动态 family contract。 |
| fallback 收敛 | 普通组件不得新增 `var(--token, fallback)` 色值；确需边界兜底时先证明 owner、reason 和 boundary。 |
| 组件 token 抽取 | 复杂 surface 只能在 semantic token 不足以表达契约时新增 component token，并保持 surface namespace。 |
| 近似色治理 | 普通组件 near pair 维持 0；专用域 near pair 只作为证据队列，不能按数值相似自动合并。 |
| 防回退约束 | baseline 必须继续拦截 raw app color、token-equivalent literal、内部 alias 读取、未定义 key、stale registry 和 payload 缺失 canonical。 |

Phase 5 决策记录：

| pair | 决策 | 调用点 | 依据 |
| --- | --- | --- | --- |
| `#1f2024` -> `#202024` | merge | `ChatInputPixelPet.scss` panda body/decor；`bitfun-dark.theme.ts` editor subtle border | RGB distance = 1，非状态色，非相邻 surface 边界；panda 固定深色与 editor border 不在同一视觉层级承担区分 |
| `#6e7681` -> `#6e7781` | merge | `LanguageRegistry.ts` Plain Text identity；`prismTheme.ts` light comment | RGB distance = 1，均为 neutral muted 文本/identity 色，不表达状态严重程度或数据差异 |
| app UI / editor alpha raw values | merge to token/color-mix | `ContextMenu.scss`、`TiptapEditor.scss`、`GitDiffEditor.scss`、`AIModelConfig.scss`、`NurseryView.scss`、`AgentCard.scss`、`ImageViewer.scss` | 色相来自现有 accent/success/overlay/text/error contract，透明度仅表达层级；迁移为 token/color-mix 保留层级但移除游离 raw color |
| DiffEditor added/deleted alpha values | component-tokenize | `DiffEditor.scss` | `0.15/0.18/0.20/0.38` 表达统计徽标、行背景、强调行和字符级 diff 的层级差异，不能直接合并；改为 `--diff-editor-*` 组件 token 后保留层级并移除游离 raw rgba |
| `#ff8800` -> `#ff8c00` | merge | `StreamText.scss` rainbow/fire orange | RGB distance = 4，均为 visual-effect 暖橙，非相邻状态色，合并后不影响用户区分 |
| `#ffdd00` -> `#ffd700` | merge | `StreamText.scss` fire yellow；editor/reference yellow | RGB distance = 6，均为亮黄强调色，调用点不相邻，不承担不同业务状态 |
| `#7dd3fc` -> `#7DCFFF` | merge | `GenerativeWidgetToolCard.scss`；`bitfun-dark.theme.ts` editor link | RGB distance = 5，均为非状态 sky/cyan 强调，调用点跨 surface 且不相邻 |
| `#00b4d8` -> `#00add8` | merge | `StreamText.scss` ocean mid；Go language identity | RGB distance = 7，同为 cyan/blue identity/visual-effect 色，非错误/警告/状态强度 |
| language identity palette | category-compress | `languageIdentityAccents.ts` 内置 language/file identity registry；`LanguageRegistry.ts` 和 code snippet metadata 读取 | 原 52 个出现、50 个唯一色改为 8 个大类色；保持所有 language/file key、label、icon、extension 和 helper API 不变，颜色不再承担唯一识别职责；消费方必须同时显示 label/icon/extension/file name 之一 |
| syntax palette roles | role-compress | `syntaxHighlightAccents.ts` Prism/Markdown/CodePreview palette | 浅色 `string/number` 共享 literal blue；light `punctuation` 独立于 comment，dark `tag/property` 保持区分，避免 12px code preview 和 JSX/HTML 相邻 token 可读性下降；syntax 18/17 -> 16/16 |
| Mermaid `#dfe2e8` -> `#e0e2e8` | merge | light `nodeFillHover`；dark `nodeText` | RGB distance = 1，跨 light/dark fallback 角色，不在同一主题视口中承担相邻状态区分；合并后保留 `nodeFillHover` 和 `nodeText` 语义 key |
| Mermaid `#5a5e68` -> `#5a5e6a` | merge | dark `edgeLabelBorderHover`；dark `nodeStroke`/`edgeStroke` | RGB distance = 2，均为深色 Mermaid neutral stroke/border；edge label hover border 不表达独立状态严重程度，合并后仍通过 key 区分角色 |
| Mermaid `#6a6e78` -> `#6a6e7a` | merge | dark `textMuted`；dark `nodeStrokeHover` | RGB distance = 2，均为深色 Mermaid subdued neutral；不是 success/warning/error 或数据类别色，合并后保留 muted text 与 hover stroke 语义 |
| Cyber `#141414` -> `#151515` | merge | `bitfun-cyber` scene background；secondary background；Monaco line highlight | RGB distance = 1.73，三者都在 Cyber 暗色 neutral surface 内，不表达不同状态、严重程度或数据类别；保留 `bgScene`/`secondary`/`lineHighlight` 语义 key，实际色值统一 |
| dark card white alpha literal | merge to canonical overlay stops | `ThemeService.ts` dark `--card-bg-*` runtime injection | `0.015/0.025/0.035/0.09/0.13` 仅是 dark card fallback 的微弱层级色，合并到现有 canonical overlay stop；复审后保留 default/elevated/subtle/hover/active 的相邻状态差异 |
| `#f3f3f5` vs `#f4f4f5` | preserve | light theme primary background；dark theme status text | RGB distance = 1.41，但跨 light/dark theme 且角色不同；不通过数值相近抹平主题个性或状态文本对比 |
| `#b8c6ff` -> `#b8c4ff` | merge | Slate theme purple alpha ramp；Slate purple solid stop | RGB distance = 2，同一 Slate purple ramp 内肉眼不可区分；alpha ramp 继续保留独立语义 key，实际 RGB channel 收敛到 solid 500 stop |
| `#fafafa` -> `#ffffff` | merge | Dark theme primary button hover/active text | RGB distance = 8.66，均为同一 dark primary button 交互前景，不承担相邻区域或状态类别区分；默认态 `#f4f4f5` 保留以维持按钮交互层级 |
| `#e2e6eb` -> `#e2e8f0` | merge | Slate window control standard hover text；Slate accent soft ramp | RGB distance = 5，同一 Slate neutral chrome 体系内的 hover 前景，非 close/error 状态，合并到已有 soft accent stop |
| `#f0f2f5` -> `#eef0f3` | merge | Slate primary button default text；Slate text primary | RGB distance = 3.46，同一 Slate foreground ramp 内肉眼难区分，且不与 hover/active 的 white foreground 相邻表达不同业务状态 |
| `rgba(255,255,255,0.22)` -> `rgba(255,255,255,0.24)` | merge | static token scrollbar hover、info border、ChatInput border、dark fallback scrollbar、MiniApp fallback、scrollbar mixin defaults | 同一 white overlay hover/border stop 内 0.02 alpha 差异；不用于相邻信息层级，合并到已有 `0.24`/`border-medium` 后保留语义 key |
| `rgba(0,0,0,0.28)` -> `rgba(0,0,0,0.30)` | merge | light runtime scrollbar fallback、MiniApp light scrollbar fallback | 同一 light scrollbar hover fallback 内 0.02 alpha 差异；仅在主题未提供 scrollbar 值时兜底，合并到现有 black30 stop |
| dark neutral element alpha ramp | merge to canonical overlay stops | `tokens.scss`、`createDarkNeutralElement()`、theme prompt snapshot | 非标准 neutral alpha 已收敛到 canonical overlay ramp，保留 subtle/soft/base/medium/strong/elevated key；只压缩肉眼弱差异，不删除层级角色 |
| `--color-overlay-white-05/06` -> `--color-overlay-white-08` | retire key | `tokens.scss`、`ThemeService.ts`、generated widget payload、内部 CSS var 读取 | 旧 key 只表达极弱 white overlay 微差，继续保留会扩大主题扩展 surface；root/runtime/payload 不再导出，历史 generated widget 读取通过 iframe alias fallback 映射 |
| dark theme shadow alpha ramp | preserve | `src/web-ui/src/infrastructure/theme/presets/*-theme.ts` | 经独立复审，Tokyo、Slate、China Night、Cyber 等 shadow alpha 同时表达主题个性和 elevation 强度，且影响菜单、下拉、toolbar 等相邻浮层；不作为本轮压缩对象，后续只有截图证据充分时才按主题内语义合并 |
| low-end overlay payload stops | retire keys | `tokens.scss`、`ThemeService.ts`、generated widget payload、内部 CSS var 读取 | `--color-overlay-white-02` 合并到 `--color-overlay-white-04`，`--color-overlay-black-06` 合并到 `--color-overlay-black-08`；二者只表达极弱 surface/shadow 透明度，不承载 status/semantic；root/runtime/payload 不再导出，历史 generated widget 读取通过 iframe alias fallback 映射 |
| light slate overlay micro-stops | merge to coarse surface ramp | `tokens.scss`、Markdown/table、ChatInput、ModelSelector、ContextMenu、TiptapEditor 等浅色 surface 读取 | `--color-overlay-slate-03/06/10/14` 分别收敛到 `04/08/12/12`，保留 `04/08/12/22` 四个 stop；用途集中在浅色主题非语义 shadow、border、stripe、hover，避免为 0.01/0.02 alpha 微差扩大主题扩展 key 面 |
| accent blue alpha micro-stop | merge to existing accent stop | `tokens.scss` | `rgba(59, 130, 246, 0.12)` 只用于同一 blue accent 的 glow/gradient 微层级，合并到既有 `0.15` stop；不影响 semantic/error/warning 或相邻状态区分 |
| inspector active overlay alpha | merge to existing accent stop | `uiExceptionAccents.ts` | inspector active background 与 blue accent `0.15` 是同一强调背景概念，合并后提升可见性并减少一个全局唯一色；border 和 selected overlay 仍保留独立状态色 |
| dark preset neutral surface literals | merge within theme preset namespace | `bitfun-midnight`、`bitfun-cyber`、`bitfun-tokyo-night` | Midnight workbench 复用 panel；Cyber secondary/tertiary/quaternary 复用既有 dark neutral；Tokyo tertiary/elevated 复用 secondary/quaternary；只处理中性背景 surface，不合并 accent、semantic、Monaco syntax 或状态色 |
| boundary fallback white overlay | merge to coarse fallback stops | `themeBoundaryFallbacks.ts` | iframe/截图兜底只在 root token 不可用时使用，`borderBase`、`elementBgBase`、`elementBgMedium` 收敛到 `0.12`，保留 key 以维持边界语义 |
| Mermaid fallback near surfaces | merge within Mermaid namespace | `_tokens.scss`、`mermaidThemeFallbacks.ts` | light section/active/cluster 和 dark edge-label/note/activation fallback 使用既有 Mermaid surface stop；只在 Mermaid 专用域内合并，不映射到普通 app status token |
| Monaco light highlight and dark unchanged diff | merge within editor namespace | `MonacoThemeSync.ts`、`bitfun-dark.theme.ts` | light inactive selection/word highlight 收敛到相同弱高亮 stop；dark unchanged diff 使用 editor base background，降低不承担状态含义的深色微差 |
| markdown light code/table neutral | merge to existing text/surface token | `flowchat-markdown-code-vars.scss`、`flowchat-markdown-table-vars.scss` | inline code 前景复用 light text token，table header 复用 code block light surface；避免跨 Markdown 子表面的近似浅灰重复 |
| accent/glass low alpha values | merge to existing accent stops | `tokens.scss` | blue/purple/green glass 和 card accent 的 `0.06/0.10/0.18` 低 alpha 值收敛到相邻既有 stop，保留 hover/base key，避免扩张独立透明度阶梯 |
| mobile dark scene `#16161a` -> `#18181a` | merge | `src/mobile-web/src/theme/presets/dark.ts` | RGB distance = 2.83，scene/flowchat 与 secondary background 不再保留肉眼弱差异；仍保留 `BG_SCENE` 语义别名以便后续主题需要重新分化 |
| mobile light near grays | merge | `src/mobile-web/src/theme/presets/light.ts` | `#1c1c1e`、`#e5e5ea`、`#aeaeb2` 分别收敛到既有 dark/mobile gray；跨 light/dark theme 不在同一视图相邻表达层级，contrast 仍满足原角色预期 |
| installer dark runtime/static panel `#18181a` -> `#1a1c1e` | merge | `BitFun-Installer/src/theme/installerThemesData.ts`、`BitFun-Installer/src/styles/variables.css` | 复用安装器已有 Slate panel 值，运行时 preset 与首屏静态变量一致；背景层级 key 保留，实际深色 neutral 数量减少 |
| installer dark/slate element alpha ramp | merge | `BitFun-Installer/src/theme/installerThemesData.ts`、`BitFun-Installer/src/styles/variables.css` | `0.10/0.21` 收敛到既有 `0.12/0.24` overlay stop；soft/base 与 strong/elevated 分别保留语义 key，安装器非高密度相邻 surface 不依赖这些微小差异表达状态 |
| installer info `#E1AB80` -> `#e1ab80` | normalize | `BitFun-Installer/src/theme/installerThemesData.ts`、`BitFun-Installer/src/styles/variables.css` | 仅大小写归一，无视觉变化；避免跨 root 审计和人工复核时把同一 hex 误读成不同色值 |
| web/installer/CLI cyber deep blacks | merge | `src/web-ui/src/infrastructure/theme/presets/cyber-theme.ts`、`BitFun-Installer/src/theme/installerThemesData.ts`、`src/apps/cli/themes/presets/bitfun-cyber.json` | `#101010`、`#0d0d0d` 和旧 tooltip `rgba(16, 16, 16, 0.95)` 收敛到既有 dark primary `#0e0e10` 派生值；RGB distance <= 3.32，主题识别仍由 neon accent、surface 和 typography 表达；安装器和 CLI 同名主题同步避免跨 surface 漂移 |
| web dark/slate/tokyo tooltip literals | derive from preset background | `src/web-ui/src/infrastructure/theme/presets/*-theme.ts` | tooltip 仍输出相同 rgba 值，但改为由对应 `BACKGROUND_SECONDARY` 常量派生，降低源字面量数量并避免相同 surface 重复手写 |
| CLI Rust fallback near surfaces | merge within fallback palette | `src/apps/cli/src/ui/theme.rs` | fallback dark hover 复用 panel，light panel/block 复用同一浅灰，light hover 复用 element；fallback near pair 降为 0，preset 主路径和语义 key 不变 |
| CLI color audit baseline | guardrail | `scripts/audit-cli-theme-colors.mjs`、`scripts/theme-color-governance-baseline.cli.json` | CLI/TUI preset 与 Rust fallback contract 单独计数，避免被 web-ui CSS var 审计误归类；后续只能在债务减少时下调预算 |
| CLI preset non-adjacent near surfaces | merge within preset namespace | `src/apps/cli/themes/presets/bitfun-dark.json`、`bitfun-midnight.json`、`bitfun-tokyo-night.json` | Midnight 深色 background/panel/context、input、subtle border 和 diff gutter、dark removed diff、Tokyo added diff 复用已有终端 surface stop；preset 唯一色数 134 -> 125，总唯一色数 164 -> 155，preset near pair 20 -> 7；Cyber 相邻 element/border、diff body/gutter 和跨主题 text/background 仍保留，不按数值相近强合并 |
| CLI dark syntax variable `#80d4ff` -> `#7dcfff` | merge | `src/apps/cli/themes/presets/bitfun-dark.json` | 同一 dark CLI syntax palette 内 function/variable 均为 cyan 高亮，RGB distance 5.83，终端语法类别仍可通过 token role 保留；合并后不影响 diff、warning/error 或背景边界 |
| web theme preset neutral compression | merge within preset namespace | `src/web-ui/src/infrastructure/theme/presets/{china-night,china-style,cyber,dark,light,midnight,slate,tokyo-night}-theme.ts` | 跨主题深色 neutral、弱文本、非状态 scene/workbench 和低透明 accent seed 收敛到已有 stops；复审后保留 Ink Night secondary/lineHighlight、Tokyo Night primary、Slate success、light surface ramp、China paper elevated ramp、Monaco lineHighlight 和 error/warning/status 色。Theme preset 唯一色数 147 -> 121，theme preset near pair 57 -> 11 |
| CLI weak foreground and input preset compression | merge within CLI preset namespace | `src/apps/cli/themes/presets/bitfun-dark.json`、`bitfun-midnight.json` | Dark syntax punctuation 复用正文前景，Midnight markdown strong 复用正文前景，Midnight input background 复用 element surface；均不承担 diff body/gutter 或 border/element 相邻区分。CLI preset 唯一色数 125 -> 123，总唯一色数 155 -> 153，preset near pair 7 -> 5 |
| remaining web theme near pairs | preserve | web theme preset | 剩余 web near 主要是 light/paper 相邻 surface、主题身份背景与 Monaco light lineHighlight；CLI preset/runtime near pair 已清零。后续继续合并需要截图或语义证据，不以 RGB distance 自动处理 |
| web/installer dark/slate/midnight neutral text | merge | `src/web-ui/src/infrastructure/theme/presets/slate-theme.ts`、`midnight-theme.ts`、`BitFun-Installer/src/theme/installerThemesData.ts` | `#9da0a8`、`#9ea4ab` 收敛到 `#a1a1aa`，Slate branch 复用 `SLATE_ACCENT`；均为跨主题 neutral text/branch 语义，不合并 error/warning/status 色，同名安装器预览同步 |
| installer preset seed compression | derive within installer namespace | `BitFun-Installer/src/theme/installerThemesData.ts`、`BitFun-Installer/src/styles/variables.css` | 安装器是简单首启/安装 surface，不应维护完整 app 级主题色板；8 套 installer 主题压缩为少量 background/accent/status seed 加 dark/light tone，theme preset 色值 160 -> 63、preset 唯一色 121 -> 60，静态变量同步用 alias 派生而不是复制色值；保留每个主题的 success/warning/error seed，避免安装进度、完成和错误反馈与最终主题状态色脱节 |
| generated widget payload shell compression | move host-internal keys to iframe shell | `src/web-ui/src/tools/generative-widget/themePayload.ts`、`GenerativeWidgetFrame.tsx` | host payload allowlist 153 -> 98；只保留外部 widget 真正需要随主题变化的 canonical 子集和 `components.button` 投影，tool-card/layout/旧 overlay 与 legacy alias 由 iframe fallback/static shell 派生，避免把宿主内部 key 变成插件或主题扩展 API；accent/status/border/radius/spacing/button 等主题敏感 key 必须继续由宿主 payload 覆盖，不能固化在 iframe shell |
| slate success and CLI added diff near merge | merge non-adjacent equivalent concepts | `src/web-ui/src/infrastructure/theme/presets/slate-theme.ts`、`src/apps/cli/themes/presets/bitfun-ink-night.json`、`bitfun-tokyo-night.json` | Slate success 复用已有 success green；CLI ink/tokyo added diff body 复用同一深绿非相邻 surface。保留 Cyber element/border、diff body/gutter 等相邻区分，web theme preset near 11 -> 10，CLI preset near 5 -> 4 |
| installer named dark background seed | preserve visible primary identity | `BitFun-Installer/src/theme/installerThemesData.ts` | 安装器界面简单，但 ThemeSetup 预览直接展示 primary/secondary background；Dark 复用 canonical seed，Ink Night/Cyber/Tokyo Night 保留各自 primary，secondary 复用 common dark surface。Installer theme preset 唯一色 57 -> 53，near pair 11 -> 3；剩余 near pair 是用户可见主题识别，而非内部冗余 |
| web overlay alpha ladder | preserve adjacent state stops | `tokens.scss`、`ThemeService.ts`、`themePayload.ts` | 复审后保留 `0.04/0.08/0.12/0.15/0.20` 等黑白 overlay stop；card、markdown、shadow 等相邻状态仍通过语义 key 表达，不再保留 `0.06/0.10` 微弱中间 stop。生成式 widget host payload 仍不转发静态 overlay，由 iframe fallback/static shell 派生 |
| generated widget static overlay payload | move to iframe shell | `src/web-ui/src/tools/generative-widget/themePayload.ts`、`themePayload.test.ts` | 静态黑白 overlay 不再由 host payload 转发，iframe fallback/static shell 直接派生；payload allowlist 98 -> 94，历史低阶 overlay alias 继续通过 iframe fallback 映射，不影响已生成 widget |
| canonical overlay micro-stop retirement | retire keys | `tokens.scss`、`ThemeService.ts`、`themePayload.ts`、内部 CSS var 读取 | `--color-overlay-white-06` 合并到 `--color-overlay-white-08`，`--color-overlay-white-10` 合并到 `--color-overlay-white-12`，`--color-overlay-black-10` 合并到 `--color-overlay-black-12`；只处理非语义弱 overlay，历史 generated widget 读取通过 iframe alias fallback 保留 |
| Mermaid light semantic status tokens | derive fill/stroke from app semantic status, preserve readable note text | `_tokens.scss`、`mermaidThemeFallbacks.ts` | light done/crit 颜色改读 success/error semantic token，note fill/stroke 由 warning 派生；note text 保留专用深色值以满足说明文字可读性。critical/error fallback fill 共享同一弱 error 背景，Mermaid unique 73 -> 65 且 near pair 保持 0 |
| Monaco diff background strength | preserve per-layer strength | `bitfun-dark.theme.ts` | 复审后保留 inserted/removed/modified 的 text、line、gutter 背景强度阶梯；代码审查场景需要同时识别整行变更、行内片段和 gutter 定位锚点，不能只按同 change type 合并 |
| generated widget derived payload keys | move derived keys to iframe fallback/static shell | `themePayload.ts`、`themePayloadCompatibility.ts`、`themePayload.test.ts` | accent 细分 stop、status border、strong/prominent border、strong element 和 `--size-radius-md` 不再从 host payload 读取；iframe fallback/static shell 继续解析这些派生 key，payload allowlist 94 -> 80 |
| CLI preset/runtime near cleanup | merge non-semantic close surfaces | `src/apps/cli/themes/presets/*.json` | Dark/Ink/Midnight/Cyber 的非状态 surface、input、border 和极近前景值复用已有 preset stops；CLI preset unique 122 -> 118、total unique 152 -> 148，preset/runtime near pair 均降为 0。OpenCode compatibility 字段单独计数，不冒充 runtime 收益 |
| generated widget button payload | preserve after adversarial review | `src/web-ui/src/tools/generative-widget/themePayload.ts`、`themePayload.test.ts` | 曾评估将 button component token 移入 iframe shell 派生，但 Cyber/Tokyo/light 等主题的 button bg、border、shadow、transform 和 hover/active 不是稳定可派生值；因此 payload 保持 80，button payload 保持 18，避免牺牲 generated widget 的主题识别和交互反馈来换取表面 key 数下降 |
| CLI truecolor fallback palette | derive from built-in preset JSON | `src/apps/cli/src/ui/theme.rs`、`scripts/theme-color-governance-baseline.cli.json` | `Theme::dark()` / `Theme::light()` 不再手写第二套 `Color::Rgb` palette，而是解析内置 OpenCode-compatible preset；不完整外部 theme 仍回退到 base。用户可见 truecolor 默认值会跟随内置 preset：dark primary/background/panel 从 `#3b82f6/#111827/#1e2637` 到 `#60a5fa/#0e0e10/#1c1c1f`，light 从 `#2563eb/#f9fafb/#f0f2f5` 到 `#475569/#f3f3f5/#ffffff`；ANSI16/monochrome 不变。复审后补充 role 级对比 guard，并将 command prompt 改为跟随 primary，以覆盖 command card block/hover 背景；light diff added/removed、light diff line number、light warning 以及 dark diff line number 收敛到更可读的同主题既有/同义色。Rust fallback RGB 44 -> 0，fallback unique 32 -> 0，CLI total unique 148 -> 114，runtime unique 107 -> 75 |
| mobile static overlay foreground | tokenized static color | `src/mobile-web/src/theme/presets/shared.ts`、`chat-input.scss`、`light.ts` | 图片缩略图关闭按钮必须在黑色 overlay 上保持白色前景；新增 mobile `--color-static-white` 后组件 raw `#fff` 归零，light secondary 改读该静态 token，mobile 总颜色出现次数不增加且唯一色 31 -> 30 |
| Rust GUI theme config fallback | retire legacy schema | `src/crates/assembly/core/src/service/config/{types,manager,providers,global,service}.rs` | 删除旧顶层 GUI `theme` struct/provider/default 导出，只保留 `theme.id` -> `themes.current` 的加载、导入和旧调用路径 fallback；导入路径保留 raw config JSON 到 manager 归一化后再反序列化，避免旧 `theme.id` 被提前丢弃。Rust 不再拥有完整 UI theme schema，终端 `terminal.theme` 仍作为 ANSI palette 历史字段保留 |
| web theme preset near cleanup | merge non-critical preset surfaces | `src/web-ui/src/infrastructure/theme/presets/{china-style,light,slate,tokyo-night}-theme.ts` | Light quaternary 复用浅灰背景、Slate button text 复用 primary text、China Style elevated 复用 tertiary、Tokyo border 复用既有深 slate；产品复审后保留 Ink Night secondary，因为它也承担 Monaco lineHighlight 和相邻 ink surface 层级。Theme preset unique 119 -> 116，near pair 9 -> 5 |
| installer dark preview background cleanup | preserve adjacent theme-card identity | `BitFun-Installer/src/theme/installerThemesData.ts` | 产品复审确认 ThemeSetup 会相邻展示主题卡并直接渲染 primary/secondary background，因此 Ink Night/Cyber/Tokyo Night primary 属于用户可见主题识别，不再合并。Installer theme preset unique 保持 53，near pair 保持 3；后续若要继续压缩，需要先新增非背景 identity swatch 或截图证明不会削弱选择识别。 |
| CLI truecolor readable role cleanup | merge consumed visible roles | `src/apps/cli/themes/presets/bitfun-{dark,light}.json`、`src/apps/cli/src/ui/theme.rs` | 只处理 CLI renderer 当前消费的 foreground role：light diff added/removed 复用同主题 highlight 色，light diff line number 复用 textMuted，light warning/markdownEmph/syntaxNumber 共用更深 amber seed，dark diff line number 复用 textMuted；不处理未消费的 `diff*LineNumberBg` 兼容字段。preset unique 118 -> 114，runtime preset unique 77 -> 75，并用 role contrast 单测覆盖 diff、warning、muted、command 和 line-number 表面 |
| remaining near pairs | none in ordinary components | 无 | 审计口径下普通组件 near pair 已清零；后续只在专用 palette 自身重设计时处理 Monaco/terminal/Mermaid/syntax 内部近似色 |
| Monaco theme palette | classify as exception | `tools/editor/themes/bitfun-dark.theme.ts` | 该文件是 Monaco theme 完整色板，不是普通 app UI；归入 editor/exception 后不再被误计为 component raw color |
| Flow Chat capture fallback | boundary fallback | `ExportImageButton.tsx`、`captureElementToDownloadsPng.tsx` -> `themeBoundaryFallbacks.ts` | `#121214` 只在 root theme 变量不可用时兜底截图背景，集中 owner 后避免截图工具重复携带 raw fallback |
| git runtime alias surface | retire unused keys | `tokens.scss`、`ThemeService.ts` | `--git-color-pull*`、`--git-color-push*`、`--git-color-branch-border`、`--git-color-added-border`、`--git-color-changes-border`、`--git-color-changes-bg-hover` 和 `--git-color-deleted-border` 没有产品读取，且分别只是 branch/staged/status 派生同义 key；删除后实际渲染仍读取 canonical branch、staged、added、changes、deleted key，web-ui 定义 key 956 -> 945 |
| installer minimal runtime projection | retire unused installer keys | `BitFun-Installer/src/theme/installerThemesData.ts`、`installerThemeRuntime.ts`、`variables.css`、`ThemeSetup.tsx` | 安装器是简单首启/安装 surface，不消费完整 app 主题 schema；删除未读取的 background tertiary/quaternary/elevated/workbench/flowchat/tooltip、text disabled、purple family、info/highlight、border strong/prominent、element base/elevated 和额外 radius key。ThemeSetup 预览仍保留 primary/secondary background、element soft、muted text 和 accent；安装器定义 key 54 -> 30，唯一色 72 -> 66 |

Phase 6 防回退约束：

| 约束 | 当前值 | baseline | 作用 |
| --- | ---: | ---: | --- |
| `nearPairs.indistinguishableTotal` | 0 | 0 | 阻止新增普通组件肉眼不可区分 pair 未被合并或记录 |
| `nearPairs.nearTotal` | 0 | 0 | 阻止新增普通组件 near color 债务；新增必须合并、归类或记录理由 |
| `colorDomainNearPairs.indistinguishableTotal` | 0 | 0 | 控制专用域肉眼不可区分 pair 不继续增长，后续只能逐步降低或补充证据 |
| `colorDomainNearPairs.nearTotal` | 9 | 9 | 控制 theme preset/runtime/token/editor/Mermaid 等专用域 near 队列规模 |
| `colorScopes.appUi.uniqueColors` | 0 | 0 | 阻止普通组件 raw color 唯一色回涨 |
| `colorScopes.appUi.occurrences` | 0 | 0 | 阻止普通组件 raw color 出现次数回涨 |
| `colorScopes.exception.uniqueColors` | 182 | 182 | 控制专用域/例外域总体规模；UI exception、syntax 和 language identity 已收敛，但 Mermaid/editor/terminal 仍按各自 owner 单独治理 |
| `colorDomainScopes.syntax.occurrences` | 16 | 16 | 阻止 Prism syntax palette 回到一 token class 一色的不可扩展模式，同时保留相邻 token 可读性边界 |
| `colorDomainScopes.languageIdentity.uniqueColors` | 8 | 8 | 阻止 language/file identity 回到一语言一色或一扩展一色的不可扩展模式 |
| `colorDomainScopes.boundaryFallback.occurrences` | 18 | 18 | 防止 iframe/mini app/截图兜底色重新散写；导出 key 可保留语义，实际字面值必须回到 boundary fallback palette |
| `tokenAliasLiterals.occurrences` | 0 | 0 | 阻止重新出现可映射到 token 的 app literal |
| `colorDomainScopes.appUi.occurrences` | 0 | 0 | 阻止未归类 app UI 色值回涨 |
| CSS var governance errors | 0 | 0 | 保持 unresolved、fallback-only、non-contract 和 dynamic family 错误为零 |
| `compatibilityAliases.usedUnique` | 0 | 0 | 阻止产品代码重新通过旧 alias key 读取主题变量 |
| `compatibilityAliases.occurrences` | 0 | 0 | 阻止历史 alias 调用点回涨 |
| `compatibilityAliases.familyUsedUnique` | 0 | 0 | 阻止 `--radius-*`、`--spacing-*` 旧 family 重新成为内部读取面 |
| `compatibilityAliases.familyOccurrences` | 0 | 0 | 阻止旧 family 读取次数回涨 |
| `compatibilityAliases.staleRegisteredUnique` | 0 | 0 | 防止兼容 alias registry 保留没有定义或 canonical 目标缺失的 key |
| `compatibilityAliases.staleRegisteredFamilyUnique` | 0 | 0 | 防止 `--radius-*`、`--spacing-*` family contract 指向缺失的 canonical family |
| `compatibilityAliases.missingCanonicalUnique` | 0 | 0 | 防止 family alias 具体 key 缺失对应 canonical key |
| `surfaceTokenRenames.activeUnique` | 0 | 0 | 防止已迁移的 surface-local 旧 key 重新出现 |
| `surfaceTokenRenames.activeOccurrences` | 0 | 0 | 防止旧 key 在定义和读取两侧回流 |
| `surfaceTokenRenames.missingCanonicalUnique` | 0 | 0 | 防止 surface rename contract 指向不存在的 canonical key |
| `generatedWidgetPayload.varUnique` | 80 | 80 | 控制 widget 对外主题 payload allowlist 不继续膨胀；该预算只覆盖主题敏感 canonical 子集和 button component 投影，宿主内部、静态 overlay、派生 key 和历史兼容 key 不计入 payload API |
| generated widget button payload | 18 | 18 | Cyber/Tokyo/light 的 button bg、border、shadow、transform 和交互态不是低风险静态 key；除非 resolved button 输出等价，否则不从 host payload 移入 iframe shell |
| `generatedWidgetPayload.compatibilityAliasUnique` | 0 | 0 | 防止 payload 重新直接导出显式 legacy alias |
| `generatedWidgetPayload.compatibilityAliasFamilyUnique` | 0 | 0 | 防止 payload 重新直接导出 legacy size family 具体 key |
| `generatedWidgetPayload.externalOnlyCompatibilityUnique` | 0 | 0 | 防止 payload 重新保留仅因外部兼容存在的 legacy key |
| `generatedWidgetPayload.undefinedUnique` | 0 | 0 | 防止 payload 导出未定义主题 key |
| `generatedWidgetPayload.missingCompatibilityCanonicalUnique` | 0 | 0 | 防止 payload 兼容 alias 缺失 canonical 目标 |
| `generatedWidgetPayload.unexportedCompatibilityCanonicalUnique` | 0 | 0 | 防止 payload 兼容 alias 有 canonical 定义但未导出到 iframe |
| `mobile-web` app UI raw color | 0 | 0 | mobile-web 组件不再允许 raw app color；图片缩略图黑色 overlay 上的关闭按钮前景读取 `--color-static-white` |
| `mobile-web` dynamic families | 3 | 3 | `--color-accent-*`、`--color-purple-*`、`--color-pink-*` 由 mobile preset 拥有 |
| `installer` app UI raw color | 0 | 0 | 安装器组件不得携带 raw app color |
| `installer` dynamic families | 1 | 1 | 安装器只导出实际消费的 accent family；secondary purple family 不再作为安装器扩展面 |
| `cli` total unique colors | 114 | 114 | 控制 CLI/TUI preset 不继续膨胀；truecolor fallback 已由内置 preset JSON 派生，不再维护独立 Rust RGB palette |
| `cli` runtime unique colors | 75 | 75 | 只统计 BitFun CLI renderer 当前消费的 preset key；OpenCode 兼容声明和 legacy fallback 不冒充实际运行时预算 |
| `cli` runtime preset unique colors | 75 | 75 | 控制 CLI 当前消费的 preset key，不包含 markdown/syntax/diff line-number background 等未消费兼容字段 |
| `cli` compatibility preset unique colors | 87 | 87 | 单独约束 OpenCode schema 兼容声明；该口径不能冒充 BitFun CLI 运行时收益 |
| `cli` preset near pairs | 0 | 0 | 阻止 CLI preset 重新引入未解释的近似色；OpenCode compatibility 字段仍单独审计 |
| `cli` runtime preset near pairs | 0 | 0 | 阻止当前 renderer 消费路径重新引入近似色；后续压缩需先确认终端相邻状态和 diff 语义不会被削弱 |
| `cli` Rust fallback near pairs | 0 | 0 | fallback palette 不再允许新增近似重复色 |
| `fallbackContracts.uncontractedUnique` | 0 | 0 | 防止新增未说明边界的 `var(--token, fallback)` |
| `fallbackContracts.staleRegisteredUnique` | 0 | 0 | 防止已删除 fallback 继续留在 registry 中 |
| `colorDomainContracts.activeUncontractedUnique` | 0 | 0 | 防止新增专用颜色域但没有 owner 和 merge policy |

`nearPairs.*` 只基于非 token、非 exception 的普通组件颜色计算。Theme preset、
editor、syntax、terminal、language identity、boundary fallback 等专用域通过各自
`colorDomainScopes.*` 和 `colorDomainNearPairs.*` 预算约束。专用域 near 队列用于
安排截图和语义复核，不直接判定是否可合并。

视觉证据契约新增在 `scripts/theme-visual-governance-contract.json`，并由
`pnpm run theme:visual-contract` 校验。它不是截图替代品，而是后续 PR 的覆盖面
清单：任何影响主题或 UI 色值的变更，都应确认是否触达以下 surface，并按 contract
补充 focused visual review、contrast review、boundary render review 或 mobile build review。

| surface | 覆盖形态 | 重点风险 |
| --- | --- | --- |
| app-shell | desktop webview、web、desktop、narrow、dark/light/system | shell 邻近组件必须只读 canonical token，system theme 不能假设桌面专有行为 |
| flow-chat | desktop webview、web、desktop、narrow、streaming/error/empty | virtualized 和历史 turn 可能隐藏 token 回归 |
| tool-cards-review | tool card、review panel、expanded/collapsed/status | destructive/error 状态统一读取 `--color-error*`；仅 generated widget iframe fallback 保留历史 `--color-danger*` 映射 |
| code-editor-diff | Monaco、diff、selection、added/deleted/conflict | editor/diff 色表达相邻状态，不能按数值相似直接合并 |
| terminal | ANSI normal/bright、selection、error | ANSI 语义独立于 app semantic color |
| markdown-mermaid | Markdown、Prism、Mermaid、diagram/error | Markdown accent 通过 `--markdown-primary-color` 表达；tool-card 历史 `--primary-color` 已退场，Mermaid 角色不等于 app status |
| generated-widget | iframe fallback、host payload、loading/error | payload 只导出 canonical key；旧 alias 兼容集中在 iframe fallback |
| theme-settings | theme switcher、system/custom theme preview | custom theme preview 可能比普通组件更早暴露 runtime canonical token 缺失 |
| mobile-web-shell | mobile-web、mobile/narrow、loading/error/navigation | mobile web 是独立构建目标，不能只依赖 desktop WebView 验证 |
| installer-shell | installer、theme setup、language/options、loading/error | installer 是独立 React/Tauri surface；主题变量由 installer TS root 维护，Rust 不得复制 palette |

## 现有架构地图

当前主题相关定义分布在多个层次：

- `src/web-ui/src/component-library/styles/tokens.scss` 定义 SCSS 变量和
  `:root` CSS 变量。
- `src/web-ui/src/infrastructure/theme/core/ThemeService.ts` 根据当前主题在运行时注入 CSS 变量，
  同时补充了一批 app 级别别名和覆盖值。
- `src/web-ui/src/infrastructure/theme/presets/*.ts` 定义完整主题预设色板。
- `src/apps/desktop/src/theme.rs` 只维护 WebView 首屏最小 bootstrap 投影，不能扩展为完整主题 schema。
- `src/crates/assembly/core/src/service/config/manager.rs` 只保留旧 `theme.id` 到
  `themes.current` 的兼容归一化；`GlobalConfig` 不再包含顶层 GUI `theme`。
- `src/web-ui/src/tools/generative-widget/themePayload.ts` 向 generative widget payload
  暴露部分主题变量。
- `src/mobile-web/src/theme/presets` 定义 mobile-web 的独立运行时 token；移动端组件只读这些
  canonical key，不从 web-ui 或 Rust 复制 fallback palette。
- `BitFun-Installer/src/theme` 和 `BitFun-Installer/src/styles/variables.css` 定义安装器主题数据、
  运行时注入色阶和首屏静态变量；安装器 Rust 不拥有 UI palette。
- 普通组件 SCSS/CSS/TSX 的 raw app color 与局部 fallback 已由 baseline 约束为 0；
  新增必须先进入 token/exception 决策。

主要架构问题：

- 静态 token、运行时 token、payload allowlist、mobile preset 和 installer preset 已由审计
  contract 串联，但还不是单一代码生成源；新增 root 或 surface 时必须同步 contract owner。
- 有些 token 仍由运行时动态注入，必须通过 dynamic family contract 说明 owner，不能在组件内
  用 fallback literal 暗中兜底。
- 同一个语义角色的旧命名已退役为 baseline 约束；后续风险是旧 key 通过新 root、inline style
  或生成式 widget 边界回流。
- 当前主题验证链路仍不能替代可访问性证据，contrast 计算和 focused visual review 仍是大规模
  近似色合并前的必要条件。

## 问题分类

### 1. 组件内硬编码颜色

多个组件直接写入产品语义色，例如 `#60a5fa`、`#ef4444`、`#22c55e`
和大量白色半透明叠层。这会让主题调整变成跨文件替换，也会让同一语义角色
在不同组件中逐渐漂移。

改进方向：

- app 级语义颜色改为 CSS 变量。
- 组件独有角色使用组件 token。
- Monaco、terminal、语法高亮等特殊色板不直接映射到普通 app token，
  先建立专用命名空间。

### 2. 重复 fallback 色值

`var(--token, literal)` 对兼容有价值，但当它在大量组件中重复时，就会让
组件层携带 palette 副本。

改进方向：

- fallback 只保留在明确的兼容边界。
- 根主题层不再新增显式 legacy alias；确有外部边界时优先放在 generated widget iframe fallback 或 family contract。
- 组件确认 canonical token 一定存在后，移除局部 fallback 字面量。

### 3. 未定义或历史命名 token

高频可疑名称包括：

- `--color-text-tertiary`
- `--accent-primary`
- `--color-bg-hover`
- `--text-secondary`
- `--color-danger`
- `--color-border-subtle`
- `--element-bg-hover`
- `--border-primary`

其中部分可能来自动态注入，但也有明显历史别名或命名分叉。它们需要显式
进入兼容映射，而不是依赖组件 fallback 暗中兜底。

改进方向：

- 在主题层增加兼容 alias map。
- 文档中标记 deprecated 名称。
- 调用点逐步迁移到 canonical 名称。

### 4. 精确重复 token 值

精确重复不一定是错误。很多重复其实是不同语义角色当前恰好使用同一色值。
问题在于当前定义没有清晰表达 alias 方向。

例子：

- `#0e0e10` 同时用于 `$color-bg-primary`、`$color-bg-tertiary`、
  `$color-bg-workbench`、`$color-bg-flowchat`。
- `#1c1c1f` 同时用于 `$color-bg-secondary`、`$color-bg-elevated`、
  `$color-bg-scene`。
- git 相关颜色与 app intent 色如 warning、error、info 有重复。
- `$panel-border`、`$card-border`、`$input-border`、`$nav-border`
  都指向 `$border-base`。

改进方向：

- 不因为值相同就删除语义 token。
- 以“primitive value -> semantic token -> component token”的方向表达别名。
- 标记哪些 alias 是稳定语义 alias，哪些只是迁移期 alias。

### 5. 近似色冗余

近似色是风险最高的一类。相似颜色可能是历史漂移，也可能是在保护区域边界、
状态差异或主题个性。

典型族群：

- 蓝色强调族：`#60a5fa`、`#58a6ff`、`#3b82f6`。
- 暗色表面族：`#0e0e10`、`#111114`、`#121214`、`#141414`、
  `#16161a`、`#18181a`、`#1a1a1a`、`#1c1c1f`、`#1e1e22`。
- 灰色文本和边框族：`#a0a0a0`、`#9ca3af`、`#6b7280`、
  `#64748b`、`#e8e8e8`、`#e5e5e5`。
- 白色 overlay alpha：从 `0.03` 到 `0.18` 都有出现。

改进方向：

- 不能只按色差或 RGB 距离合并。
- 先判断语义角色、相邻关系、交互状态、主题预设和可访问性，再决定是否替换。
- 对于白/黑透明叠层，先建立精确等价的 overlay ramp，例如
  `--color-overlay-white-08` 和 `--color-overlay-white-12`。这一步只消除散落
  硬编码，不合并不同 alpha，因为 alpha 差异经常用于表达层级和状态。
- 如果数值型 overlay key 已经实际别名到另一个 alpha stop，内部读取应迁到真实
  canonical stop，并删除误导性的旧导出。

## 目标 Token 模型

建议采用分层 token 模型，每一层只承担一个职责。

### Primitive palette

primitive token 是原始色阶，不建议在普通组件样式中直接使用，只用于定义
语义 token。

示例：

- `--palette-blue-500`
- `--palette-red-500`
- `--palette-green-500`
- `--palette-amber-500`
- `--palette-neutral-900`
- `--palette-white`

### App semantic token

semantic token 描述产品级语义，应作为共享 UI 的默认使用层。

建议族群：

- 背景：`--color-bg-primary`、`--color-bg-secondary`、
  `--color-bg-tertiary`、`--color-bg-elevated`、`--color-bg-workbench`、
  `--color-bg-scene`、`--color-bg-flowchat`。
- 文本：`--color-text-primary`、`--color-text-secondary`、
  `--color-text-muted`、`--color-text-disabled`；如果设计系统确实需要第三层
  文本强度，再将 `--color-text-tertiary` 转正。
- 边框：`--border-base`、`--border-subtle`、`--border-emphasis`、
  `--border-focus`。
- 元素状态：`--element-bg-default`、`--element-bg-subtle`、
  `--element-bg-hover`、`--element-bg-active`、`--element-bg-selected`。
- 意图色：`--color-success`、`--color-warning`、`--color-error`、
  `--color-info`。
- 意图色背景：`--color-success-bg`、`--color-warning-bg`、
  `--color-error-bg`、`--color-info-bg`。

### Component token

当共享 semantic token 过于泛化，或者会隐藏组件自身契约时，使用组件 token。

示例：

- `--flowchat-input-bg`
- `--flowchat-input-border`
- `--flowchat-drop-zone-bg`
- `--toolbar-mode-bg`
- `--toolbar-mode-active-bg`
- `--tool-card-bg`
- `--tool-card-hover-bg`
- `--diff-added-bg`
- `--diff-deleted-bg`
- `--editor-token-keyword`
- `--terminal-ansi-green`

组件 token 默认可以映射到 semantic token，但当用户含义依赖差异时，需要保留
专用色值或专用映射。

### 兼容别名

显式 root/runtime 兼容别名已完成退役，避免旧 key 长期占用主题契约预算；历史 generated widget 内容通过 iframe fallback 兼容。

| 历史或漂移 token | 建议 canonical 目标 | 说明 |
| --- | --- | --- |
| `--accent-primary` | `--color-accent-500` | root/runtime alias 已退役；历史 generated widget 内容仅通过 iframe fallback 映射到 accent midpoint。 |
| `--text-primary` | `--color-text-primary` | root/runtime alias 已退役；历史 generated widget 内容仅通过 iframe fallback 映射。 |
| `--text-secondary` | `--color-text-secondary` | root/runtime alias 已退役；历史 generated widget 内容仅通过 iframe fallback 映射。 |
| `--text-muted` | `--color-text-muted` | root/runtime alias 已退役；历史 generated widget 内容仅通过 iframe fallback 映射。 |
| `--bg-primary` | `--color-bg-primary` | root/runtime alias 已退役；历史 generated widget 内容仅通过 iframe fallback 映射。 |
| `--bg-secondary` | `--color-bg-secondary` | root/runtime alias 已退役；历史 generated widget 内容仅通过 iframe fallback 映射。 |
| `--bg-tertiary` | `--color-bg-tertiary` | root/runtime alias 已退役；历史 generated widget 内容仅通过 iframe fallback 映射。 |
| `--border-primary` | `--border-base` | root/runtime alias 已退役；历史 generated widget 内容仅通过 iframe fallback 映射。 |
| `--color-border-subtle` | `--border-subtle` | root/runtime alias 已退役；历史 generated widget 内容仅通过 iframe fallback 映射。 |
| `--color-danger` | `--color-error` | root/runtime alias 已退役；历史 generated widget 内容仅通过 iframe fallback 映射到 error palette。 |
| `--color-bg-hover` | `--element-bg-hover` | root/runtime alias 已退役；泛化 hover 收敛到 element interaction layer。 |
| `--radius-*` | `--size-radius-*` | canonical family 为 `--size-radius-*`；旧 family 已从 root/runtime 退役，仅作为 generated widget iframe fallback 兼容和内部回读拦截规则。 |
| `--spacing-*` | `--size-gap-*` | canonical family 为 `--size-gap-*`；旧 family 已从 root/runtime 退役，仅作为 generated widget iframe fallback 兼容和内部回读拦截规则。 |

## 近似色合并规则

近似色清理必须先做安全分类。不能批量把相近颜色替换成同一个值。

默认目标是收敛，而不是保守保留。判断顺序应为：

1. 先证明能不能复用已有 token。
2. 如果色差极小且肉眼基本不可区分，可以直接合并。
3. 如果色差可见，必须给出合并依据：相同语义、非相邻显示、非状态区分、
   非数据含义、contrast 安全。
4. 如果依据不足，先标记为 `defer`，并补截图或调用点证据。
5. 只有存在明确用户理解风险时，才标记为 `do not merge`。

### 可以安全合并

同时满足以下条件时，可以合并：

- 色值代表同一个语义角色。
- 正常工作流中不会相邻显示。
- 不用于区分状态、严重程度、来源、所有权或数据含义。
- 替换后 contrast 不低于验收阈值。
- 截图对比没有造成层级或交互 affordance 丢失。

常见安全场景：

- 精确重复的语义 alias。
- 组件 fallback 复制了已经保证存在的根 token。
- 历史 alias 在运行时已经稳定指向 canonical token。
- 同一个暗色主题状态下重复出现的白色 overlay 值。

极高相似度直接合并建议门槛：

- 同一色彩空间和同一 alpha 下，RGB 通道差异肉眼不可辨。
- 不涉及 status、diff、syntax、terminal、theme personality。
- 不在相邻区域中承担边界分隔。
- audit report 中标记为 `indistinguishable`，review 时只需抽样确认。

### 必须视觉复核后才能合并

出现以下任一情况，合并前必须做视觉复核：

- 两个颜色会出现在同一视口或相邻区域。
- 颜色用于区分嵌套 panel、card、canvas、tool surface。
- 一个颜色表示 hover、active、selected、disabled、drag-over 或 focus。
- 颜色出现在 Flow Chat、tool card、review panel、git/diff UI、generated widget
  frame 等高密度区域。
- 颜色属于某个主题预设的个性表达。

合并前检查：

- 桌面和窄屏布局都要有 before/after 截图。
- 检查 normal、hover、active、selected、disabled、loading、error 等状态。
- 检查同一视口中的相邻区域。
- 检查文字和图标在替换后背景上的 contrast。
- 明确回答用户是否会失去以下判断能力：
  - 我现在在哪个区域。
  - 哪些元素可交互。
  - 当前状态是什么。
  - 哪些内容发生了变化。

这类合并不应被视为禁止合并。它们是主要的色值压缩空间，但必须带证据：

- 调用点列表。
- 旧值和目标 token 的语义说明。
- 相邻区域判断。
- before/after 截图或等价视觉证据。
- 如果合并会产生可见变化，需要在 PR 描述中说明预期影响。

### 默认不合并

以下场景默认不按近似色合并：

- success、warning、error、info、destructive action。
- git added、modified、deleted、renamed、branch、conflict。
- diff added/deleted 背景和行内高亮。
- Monaco syntax、terminal ANSI、code review token colors。
- cyber、tokyo、midnight、China 等主题个性颜色。
- 导航、scene、panel、canvas、input、floating overlay 等相邻布局区域的边界色。
- 任意可能改变 foreground/background contrast 的可访问性组合。

## 相邻关系审查模型

每个近似色合并候选都需要先回答这些问题：

| 问题 | 原因 |
| --- | --- |
| 它是否会和替换目标出现在同一视口？ | 相邻颜色可能承担区域分隔作用。 |
| 它是否区分父子表面？ | 合并可能让 card、panel、input 混在一起。 |
| 它是否区分交互状态？ | 合并可能削弱 hover、focus、active、selected。 |
| 它是否区分严重程度或数据含义？ | 状态色必须保持可读。 |
| 它是否同时影响亮色和暗色主题？ | 暗色下安全的合并，亮色下可能失败。 |
| 它是否出现在 generated widget 或 embedded frame？ | 嵌入表面不一定继承全部 root token。 |
| 它是否是主题个性的一部分？ | 主题预设可能需要保留接近但不同的 accent。 |

第一轮实施应优先建立以下高风险表面的 review inventory：

- 主框架：导航、scene viewport、content canvas、side panel。
- Flow Chat：transcript、input、collapsed input、tool card、toolbar mode、
  review team surface。
- Git 和 diff：状态 badge、文件状态、行高亮、branch indicator。
- Component library：select、code editor、stream text、button、input。
- Generated widget：widget frame、widget content、payload-exposed variables。

## 分阶段实施方案

### Phase 0：基线与工具

先建立可重复审计工具，再做批量修改。

交付物：

- 按文件和按色值聚合的颜色字面量清单。
- CSS 变量使用清单。
- 未定义或历史 token 报告。
- 精确重复 token 组。
- 按 hue/value/alpha 聚类的近似色报告。
- 高风险表面清单。

验收标准：

- 脚本可以在 `src/web-ui/src`、`src/mobile-web/src` 和 `BitFun-Installer/src` 上无副作用运行。
- 报告可以对比 baseline 与当前分支。
- 报告能区分普通 app color 与已知 exception namespace。

### Phase 1：canonical token 契约

明确 canonical token 家族和兼容别名。

交付物：

- canonical token map。
- 历史名称兼容 alias。
- `tokens.scss`、`ThemeService.ts`、`themePayload.ts` 的静态与运行时变量
  对齐。
- deprecated token 名单。

验收标准：

- 现有 UI 不应出现可见变化。
- 组件可以直接使用 canonical 名称，不需要本地 fallback literal。
- generated widget 在合理范围内获得与 app surface 一致的尺寸和颜色变量。

### Phase 2：精确重复合并

只合并 alias 安全的精确重复。

交付物：

- token 定义通过 alias 表达方向，而不是重复字面量。
- intent 与 git/diff alias 分开记录。
- border alias 指向 canonical border token。
- 高频白/黑 overlay 字面量迁移到精确 alpha token；相近 alpha 只记录为候选，
  不在没有视觉证据时合并。

验收标准：

- 预期无截图可见变化。
- `git diff --check`、web lint、type check 和相关测试通过。

### Phase 3：legacy fallback 迁移

迁移高频 fallback 调用点。

建议顺序：

1. component-library 中的 select、input、button、stream text。
2. Flow Chat 的 toolbar 和 input。
3. tool card 与 review panel。
4. workspace、git 和 diff surface。
5. generated widget frame 和 payload consumer。

边界处理规则：

- 普通 app 组件只有在 `tokens.scss`、`ThemeService.ts` 或组件根变量已经保证
  token 存在后，才移除局部 `var(--token, literal)`。
- embedded frame、generated widget、第三方内容宿主可以保留边界默认值，但默认值
  应集中在 frame/root contract 上，不应在每个 selector 中重复一套 fallback palette。
- `--member-accent`、`--group-color`、`--tag-color` 等由 TS inline style、数据驱动或
  动态 key 设置的变量，不能仅凭静态未定义报告删除；需要先确认运行时设置路径，
  再决定是建立组件根默认值，还是保留明确的边界 fallback。

验收标准：

- 组件文件不再携带根 token 的 fallback palette。
- 兼容 alias 仍保留给旧调用方或外部边界。
- 剩余 fallback 必须能解释其边界，例如 embedded widget 或第三方内容。

### Phase 4：组件 token 抽取

为不适合泛化的角色建立组件 token。

交付物：

- Flow Chat token set。
- Tool card token set。
- Diff/git token set。
- Editor/terminal exception token set。
- Widget frame token set。

验收标准：

- 组件 token 默认映射到 semantic token。
- 有意保留的例外被记录，并完成视觉复核。
- 组件不再直接用 raw color 表达产品语义。

### Phase 5：近似色合并

只有在 Phase 0-4 完成后，才进入近似色合并。

交付物：

- 候选合并表：包含角色、调用点、相邻风险和决策。
- 每个 conditional merge 都有 before/after 截图。
- rejected merge list，记录有意保留的近似色。

验收标准：

- 被合并的颜色拥有相同语义角色。
- 相邻 UI 层级仍清晰。
- 状态和数据含义仍可区分。
- 主题个性没有被抹平。

### Phase 6：防回退约束

增加轻量约束，避免新增同类债务。

交付物：

- 对组件中新 app raw color 的 lint 或 audit 检查。
- 已知 exception file、namespace contract 与 owner。
- compatibility alias、color domain 的机器可校验 owner/reason contract；fallback registry 维持为空并由 baseline 防回退。
- 覆盖 app-shell、Flow Chat、tool card/review、editor/diff、terminal、Mermaid/Markdown、
  generated widget、theme settings 和 mobile web 的视觉证据契约。
- CI 在迁移期只阻止新增问题，不因历史 baseline 直接失败。

验收标准：

- 新增组件级 raw color 必须有明确原因。
- 历史迁移可以按目录增量推进。
- exception 可见、可审查。
- 兼容 alias 可见、可审查；组件 fallback 保持 0，且 stale contract 为 0。
- CI 至少运行 `theme:color-audit:test`、`theme:color-audit:all` 和
  `theme:visual-contract`。

## 风险清单

| 风险 | 影响 | 缓解措施 |
| --- | --- | --- |
| 相邻表面的近似色被合并 | 用户可能无法区分 panel、card、输入区或工作区边界。 | 近似色合并前必须做相邻关系审查和截图对比。 |
| hover/active/selected 被合并到静态背景 | 交互 affordance 变弱。 | 状态 token 与 base surface token 分开建模。 |
| intent 色被过度归一 | warning、error、success、info 或 destructive 语义混淆。 | intent token 即使色值接近，也保留独立语义。 |
| git/diff 色被当作普通 success/error | added/deleted/changed/conflict 扫描效率下降。 | 使用专用 git/diff token，只有复核后才 alias 到 app intent。 |
| 主题个性被抹平 | 用户选择主题的价值下降。 | theme preset 保留自己的 primitive/accent 映射。 |
| fallback 先删、边界兜底后补 | embedded 或 early render surface 样式丢失。 | 先确认 canonical export 或边界 fallback，再删除普通组件 fallback。 |
| 兼容 alias 读点清零时误删边界兼容 | 旧主题、生成式 widget iframe fallback 或外部自定义内容读取旧 key 时样式丢失。 | 只迁移内部 `var()` 读取；root/runtime 显式 alias 保持退役，widget payload 只导出 canonical key，legacy key 仅由 iframe fallback 映射。 |
| 静态 token 与运行时 token 不一致 | widget、SCSS、runtime theme 注入结果不一致。 | `tokens.scss`、preset helper、`ThemeService.ts`、`themePayload.ts` 同阶段对齐。 |
| 动态 CSS 变量 key 被误判为未定义 | inline style 或数据驱动变量失去兜底，导致特定卡片、标签或分组颜色缺失。 | 对动态 key 建立运行时设置清单；删除 fallback 前补组件根默认值或保留边界 fallback。 |
| contrast 验证不可信 | 可访问性回归可能漏掉。 | 先实现真实 contrast 检查，再声称可访问性改善。 |
| 迁移 PR 过大 | review 疲劳导致视觉回归漏审。 | 按可验证的大块 contract/surface 组织 PR；每个 PR 附指标，避免拆成难以形成完整治理收益的零碎提交。 |
| editor/terminal 颜色被强行泛化 | 代码语法和 terminal 语义下降。 | 建立 exception namespace，而不是直接套普通 app token。 |

## 候选决策

### 精确重复

建议先合并定义方式，不删除语义角色。

- `--color-bg-workbench`、`--color-bg-flowchat`、`--color-bg-primary`
  即使当前解析到同一个值，也应保留为不同语义契约。
- panel/card/input/nav border 可以 alias 到 `--border-base` 或
  `--border-subtle`，但需要根据真实 contrast 和相邻关系确认。
- git/diff token 即使映射到 app intent 色，也应在组件使用层保持独立名称。

### 暗色表面近似色

不建议一次性合并所有暗色背景。

原因：

- BitFun 的主界面是高密度相邻 surface。极小的暗色差异可能用于区分 scene、
  panel、card、editor、input、floating overlay。
- 应先建立层级表：
  base -> workbench -> scene -> panel -> card -> elevated -> overlay ->
  hover/selected。

### 白色 overlay alpha

只按状态角色合并，不按“都是 white alpha”合并。

建议 token：

- `--overlay-white-subtle`
- `--overlay-white-hover`
- `--overlay-white-active`
- `--overlay-white-selected`
- `--overlay-white-focus`

alpha 差异经常承担 elevation 和交互状态，不应全部压成一个值。

### 蓝色强调色

保留 theme-specific 和 state-specific 蓝色，直到调用点完成分类。

可能角色：

- `--color-primary`
- `--color-accent-500`
- `--color-info`
- `--border-focus`
- `--link-color`
- `--selection-bg`

在确认调用点究竟表示 accent、info、link、focus、selected 或主题个性之前，
不要合并 `#60a5fa`、`#3b82f6`、`#58a6ff`。

### Editor 和 terminal 色

使用专用命名空间，不直接使用普通 app token。

建议方向：

- `--editor-syntax-keyword`
- `--editor-syntax-string`
- `--editor-selection-bg`
- `--terminal-ansi-red`
- `--terminal-ansi-green`
- `--terminal-selection-bg`

只有在语法和 terminal 含义仍然清晰时，才考虑把它们映射到 app palette。

## 验证方案

文档变更：

- `git diff --check`

实现类 PR：

- `pnpm run theme:color-audit:test`
- `pnpm run theme:color-audit:all`
- `pnpm run theme:visual-contract`
- `pnpm run lint:web`
- `pnpm run type-check:web`
- `pnpm --dir src/web-ui run test:run`
- 被修改 surface 的 focused screenshot review。
- changed foreground/background pair 的 contrast 检查。

大型 theme/runtime 变更还需要：

- 验证静态 CSS 变量和运行时注入变量都存在。
- 验证 generated widget payload 变量。
- 验证 dark 和 light theme。
- 至少覆盖以下 surface：
  - main shell
  - Flow Chat input 和 transcript
  - toolbar mode
  - tool card
  - review team panel
  - git/diff view
  - code editor
  - generated widget frame

建议每个实现 PR 都附 before/after 指标：

| 指标 | 目标 |
| --- | --- |
| 组件文件 raw color literal | 每个迁移 PR 递减。 |
| 组件级 fallback literal | 明确边界 contract 后递减。 |
| 未定义或历史 token 使用 | 内部 compatibility alias `var()` 读取保持 0；新增旧 key 读取必须先说明兼容边界。 |
| token 文件中的精确重复 literal | 改为 alias 表达。 |
| 近似色合并候选 | 每个都有 `merge`、`defer` 或 `do not merge` 决策。 |
| 视觉回归 | 已复核 surface 无回归。 |

长期预算目标：

| 指标 | 目标 |
| --- | --- |
| app 级 raw color literal | 普通组件中趋近于 0。 |
| unique app color literal | 进入 token 层后受预算约束，不再随组件增长。 |
| undocumented component color | 0。 |
| exception namespace color | 有 contract 和 owner。 |

## Review Checklist

颜色合并 PR 合入前必须检查：

- 每个被替换的字面量是否有明确语义角色。
- 旧色和新色是否可能在同一视口相邻出现。
- 旧差异是否用于区分父子 surface。
- 旧差异是否用于区分 hover、active、selected、focus、disabled、
  loading、drag-over 或 error。
- 旧差异是否用于区分状态、严重程度、数据来源或文件变更类型。
- 变更是否同时影响 light 和 dark theme。
- 变更是否影响 generated widget、code editor、terminal、Mermaid 或第三方内容。
- 删除 fallback 前，兼容 alias 是否已经存在。
- 新增或保留的 compatibility alias、color domain 是否进入对应 contract；新增 fallback 是否确有边界理由。
- 变更影响的 surface 是否已对照 `theme-visual-governance-contract.json` 确认覆盖形态。
- 高风险 surface 是否有截图或 focused visual check。
- PR 描述是否说明了任何用户可见视觉变化。

## 持续收敛约束

以下内容不是日期或进度记录，而是后续每轮主题变更都必须保持的约束：

1. 中心 token 与 mixin 层压缩：普通消费侧应保持不直接读取颜色类 Sass token；压缩空间集中在
   `tokens.scss` 的静态定义、legacy mixin、未消费 root export、badge/glass/shadow 派生和局部变量别名。
   只有被 `var()`、runtime 注入、payload 或明确边界消费的 key 才应进入 root/runtime contract。
2. 插件/主题扩展投影治理：插件侧只消费 `pluginThemeProjection.ts` 暴露的 7 个 OpenCode-compatible 语义色 key；
   新增插件 UI 需求必须先证明无法复用这 7 个 key，再新增小型 surface projection。不得把 generated widget payload
   或完整 runtime CSS var 表作为插件主题 schema。
3. 专用域近似色复核：mobile-web theme preset near pair 已清零；installer 保留 3 个相邻主题卡可见的 named dark primary preview pair；web-ui 已先处理不可感知的跨主题 neutral、tooltip 派生、runtime/static panel 值、极近 overlay fallback，以及 theme preset 的非状态 neutral/弱文本/scene/workbench 值。
   后续继续处理 theme preset 时必须避开按钮 hover/active、Tokyo Night 主题识别主背景、Ink Night secondary/lineHighlight、Monaco lineHighlight、Midnight lineHighlight/elevation、Mermaid、terminal、syntax 等相邻状态色，除非有截图证据证明不会损害层级。
4. CLI/TUI palette 压缩：CLI/TUI 已纳入 `theme:color-audit:cli`，truecolor fallback 由内置 preset JSON 派生，Rust 不再手写独立 RGB palette；Rust fallback、preset 和 runtime-consumed preset near pair 均为 0。后续只处理 preset 内能证明同一终端 surface 语义且已被 CLI renderer 消费的近似色，
   不能让 Rust/CLI 复制 web-ui palette。若要跨 surface 共享，只能先定义共享语义投影或生成链路。
5. 自定义主题扩展后续体验优化：custom theme 校验、加载、注册、导出和 preview 输入已绑定到 TS schema；
   如需继续改善首屏体验，只允许由 TS schema 生成最小 bootstrap cache，不允许 Rust 直接拥有 custom theme schema。
6. generated widget 兼容面维护：payload 已停止导出 `background/bg/text/radius/spacing` legacy key、静态黑白 overlay 转发、派生 accent/status/border/radius key 和已归并的
   `--color-overlay-white-02`、`--color-overlay-white-05`、`--color-overlay-white-06`、`--color-overlay-white-10`、`--color-overlay-black-06`、`--color-overlay-black-10`、`--color-overlay-black-25`；resolved button component token 因承载主题身份和交互反馈继续保留。历史内容兼容通过 iframe alias fallback 保留。后续新增 widget token 必须先导出 canonical，
   再评估是否需要 iframe-only alias。

每个 PR 应包含范围、影响 surface、before/after 指标、命中的 visual governance surface、
明确保留的近似色列表，以及验证命令和结果。

## 已审定兼容策略

以下 key 不再视为未登记游离 key。当前已进入
`TOKEN_COMPATIBILITY_ALIAS_CONTRACTS` 或 `TOKEN_COMPATIBILITY_ALIAS_FAMILY_CONTRACTS`，
内部调用方必须使用 canonical token；删除旧 key 前必须先完成 widget iframe fallback 兼容检查、
外部内容影响评估和视觉复核。

- `--color-text-tertiary` 当前不是一等 text ramp，root/runtime alias 已退役；
  历史 generated widget 内容仅通过 iframe fallback 映射到 `--color-text-muted`。
- `--color-primary*`、`--accent-primary*`、`--color-accent` 和旧 semantic numeric key
  已从 root token、runtime 注入和全局 compatibility alias registry 退役；新代码应使用
  accent scale、semantic role 或组件 action token，历史 generated widget 内容仅通过 iframe alias fallback 映射。
- `--color-danger*` 已从 root token、runtime 注入和全局 compatibility alias registry 退役；
  内部 destructive/error 状态统一读取 `--color-error*`，历史 generated widget 内容仅通过 iframe alias fallback 映射。
- generated widget payload 已停止导出低风险的 `--color-accent*` legacy、`--color-primary*`、
  `--accent-primary*`、`--color-danger*`、旧 surface/bg/text 细分别名、旧 semantic scale、
  旧 border 细分别名、核心 `background/bg/text` 兼容名、`--radius-*`、`--spacing-*`
  以及部分旧 font/motion 拼写和 tool-card 颜色转发；根 token 与 runtime 注入不再保留显式 legacy alias，generated widget
  iframe 提供 alias fallback，避免影响已生成 widget 内容。
- `--color-overlay-white-03` 已从静态 root、runtime 注入和 generated widget payload 退役；
  generated widget iframe alias fallback 将其映射到 `--color-overlay-white-04`，避免历史内容丢失样式。
- `--color-overlay-white-02` 已从 root/runtime、内部读取和 generated widget payload 退役；
  generated widget iframe alias fallback 将历史读取映射到 `--color-overlay-white-04`。
- `--color-overlay-white-05` 和 `--color-overlay-white-06` 已从 root/runtime、内部读取和 generated widget payload 退役；
  generated widget iframe alias fallback 将历史读取映射到 `--color-overlay-white-08`。
- `--color-overlay-white-10` 已从 root/runtime、内部读取和 generated widget payload 退役；
  generated widget iframe alias fallback 将历史读取映射到 `--color-overlay-white-12`。
- `--color-overlay-black-06` 已从 root/runtime、内部读取和 generated widget payload 退役；
  generated widget iframe alias fallback 将历史读取映射到 `--color-overlay-black-08`。
- `--color-overlay-black-10` 已从 root/runtime、内部读取和 generated widget payload 退役；
  generated widget iframe alias fallback 将历史读取映射到 `--color-overlay-black-12`。
- `--color-overlay-black-25` 已从 root/runtime、内部读取和 generated widget payload 退役；
  generated widget iframe alias fallback 将历史读取映射到 `--color-overlay-black-30`。
- generated widget payload 不导出宿主 FlowChat、navigation、z-index 和 tool-card 内部 key；iframe 内容保留独立布局和
  stacking context，只通过核心 canonical color、spacing、shape、font、motion 和 `components.button` 投影获取必要主题信息，tool-card 所需派生值由 iframe static shell 提供。
- 根 CSS var export 只保留被 `var()`、运行时主题注入或外部兼容边界实际消费的 key；组件内部仍可继续使用
  SCSS token/mixin，避免把未消费的 badge、legacy effect、z-index、git/status 和局部布局 key 扩散为运行时 contract。
- `colors.purple` 是次级强调色契约，不再等同完整 `AccentColors`。运行时只导出
  `50/100/200/400/500/600/800`，`300/700` 因无内部读取和 payload 消费已退役。
- ThemeService 的 runtime dynamic family 采用显式白名单导出，包括 accent、purple、shadow、
  blur、radius、spacing、motion、easing、font weight、font size 和 line-height；custom theme
  中的额外字段不得自动外溢为 root key。FontPreference 也不再生成 `5xl`。
- `effects.shadow` 运行时主题 scale 到 `xl` 为止；`$shadow-2xl` 已从共享 SCSS token 和 runtime
  theme contract 退役，旧调用点应读取 canonical `--shadow-xl` 或定义更窄的组件局部阴影。
- 尺寸 canonical family 是 `--size-radius-*` / `--size-gap-*`；`--radius-*` /
  `--spacing-*` 已从 root/runtime 和内部 source 退役，只作为 generated widget iframe alias fallback 保留。
- 迁移期 CI 采用严格 baseline：普通 app raw color、内部 compatibility alias 读取、fallback、
  未定义 CSS var、payload 未定义 key、payload 缺失 canonical 和 payload 未导出 canonical 均为 0；
  payload 兼容 alias 数量必须保持为 0，不能无依据增长。

## 完成标准

整体优化完成时应满足：

- 共享 app color 由 canonical semantic token 表达。
- 组件专属角色由文档化 component token 表达。
- 历史 token 名称已迁移或明确 alias。
- 普通组件文件不再出现 app 级 raw color。
- 近似色都有 merge、defer 或 reject 决策记录。
- 相邻 surface、交互状态、状态语义和主题个性仍能被用户清楚识别。
- 静态 token、运行时 token、widget payload token 对齐。
- 新增 raw color 必须经过可见 review 决策。
- 主题治理 CI 覆盖颜色审计、契约测试和视觉证据契约校验。
