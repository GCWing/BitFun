# OpenBitFun 启动性能与产物体积审阅报告

- 审阅日期:2026-07-26
- 审阅范围:Tauri 冷启动链路(src/apps/desktop)、前端首包与分包(src/web-ui)、静态资源(public/dist)、Rust release profile
- 方法:只读源码分析 + 现成 dist 产物统计(未运行完整构建)

> 2026-09-02 更新:下文保留 2026-07-26 的体积基线；字体项已经按平台字体档案重新实现，当前状态见第 2 节。

## 产物体积基线(现成产物实测)

| 产物 | 大小 | 说明 |
|---|---|---|
| `dist/` 总计 | **65 MB** | Tauri `frontendDist`,会被嵌入 exe(brotli 压缩后进入安装包) |
| `dist/assets`(JS/CSS) | 19 MB(JS 13.6 MB / 731 个 chunk) | 入口 `index-DwMxiWtW.js` 单文件 **5.44 MB** |
| `dist/agent-companion-pets` | **18 MB** | 10 个桌宠精灵图全量内置 |
| `dist/monaco-editor` | **14 MB** | monaco `min/vs` 全量拷贝(含 9 种 NLS 语言、tsWorker 5.7 MB) |
| `dist/fonts` | **13 MB** | 审阅时的旧内置中文字体 3 个字重,各 ~4.2 MB |
| `dist/` 根目录图片 | ~2.7 MB | OpenBitFun-Logo.png 1.06 MB 等未压缩 PNG |
| `target/release/openbitfun-desktop.exe` | 217 MB(7-24 本地构建,profile 未知,可能为 release-fast) | 前端 dist 全部嵌入二进制 |
| NSIS 安装包 | 0.2.11 → 81 MB;0.2.13 → **90.5 MB**(两个小版本 +9.4 MB) | `target/release/bundle/nsis/` |

> 前置说明:前端在启动编排上已有明显投入(`src/web-ui/src/app/startup/startupPerformanceContract.test.ts` 契约测试、`index.html` 内联 splash、AppLayout lazy、i18n 命名空间懒加载、Rust 侧 startup_trace),本报告只列仍然成立的真实问题。

---

## 发现总览表(按收益排序)

| # | 问题 | 维度 | 预期收益 | 风险 |
|---|---|---|---|---|
| 1 | Rust 端全部后端服务**串行初始化完成后才创建窗口**,init 期间无任何窗口 | 冷启动 | **高** | 中 |
| 2 | 旧字体无条件进入所有平台安装包 | 体积/平台一致性 | **已解决**(2026-09-02) | — |
| 3 | 10 个桌宠精灵图 18 MB 全量内置 | 体积 | **高**(-10~16 MB,视方案) | 中(产品决策) |
| 4 | Monaco 全量 min 拷贝:未用 NLS 语言 1.4 MB + 未做裁剪 | 体积 | 中(-1.4 MB 起) | 低 |
| 5 | 首包 5.44 MB:三语种 bootstrap 文案 ~793 KB 全部 eager 打入入口 | 冷启动+体积 | 中(首包 -0.5 MB+,减 JS parse) | 中 |
| 6 | 未压缩大图:OpenBitFun-Logo.png 1.06 MB 等 ~2.7 MB PNG | 体积 | 中(-2 MB) | 低 |
| 7 | 四套语法高亮引擎并存(highlight.js×2 版本、prismjs×2 版本、refractor) | 体积 | 中 | 中 |
| 8 | `[profile.release]` 未设 `panic = "abort"` | 二进制体积 | 中(exe -5~10%) | 中低 |
| 9 | `main.tsx` 静态 import 桌宠组件 `AgentCompanionDesktopPet`,主窗口也要加载 | 冷启动 | 低 | 低 |
| 10 | vite 无 manualChunks(观察项,当前自动分包已可用) | 体积 | 低 | — |

---

## 逐条详情

### 1. 窗口创建被全部后端初始化串行阻塞(冷启动最大瓶颈)【高收益/中风险】

**问题**:`openbitfun_desktop_lib::run()` 在进入 `tauri::Builder` 之前,串行 `await` 了 7 个初始化步骤;主窗口在 `setup()` 中更靠后的位置才创建。也就是说,从进程启动到用户看到第一帧(哪怕是 splash),要等完:

- `src/apps/desktop/src/lib.rs:325` `initialize_global_config().await`(磁盘 IO)
- `lib.rs:337-353` `initialize_global_i18n_service().await`
- `lib.rs:364` `AIClientFactory::initialize_global().await`
- `lib.rs:376-384` `init_agentic_system().await`(内含 PersistenceManager、TokenUsageService、CronService 等多次磁盘 IO,见 `lib.rs:1581-1737`)
- `lib.rs:388` `init_function_agents().await`
- `lib.rs:412` `AppState::new_async().await`(内含 `WorkspaceService::new().await` 等,`api/app_state.rs:95-125`)
- `lib.rs:423` `DesktopRuntimeContext::build`
- 进入 `setup()` 后还有:Windows 注册表同步(`lib.rs:545-594`)、flashgrep 探测(`lib.rs:603-653`)、mobile-web 资源探测(`lib.rs:659-701`)、以及 **`block_in_place` + `block_on` 同步等待工作区启动快照**(`lib.rs:704-728`),之后才 `theme::create_main_window`(`lib.rs:731`)。

窗口本身创建后是立即 `show()` 的(`theme.rs:549,561-589`),index.html 的内联 splash 也已把"白屏"处理得很好——**瓶颈不是白屏,而是"窗口出现之前"的纯黑等待期**,时长等于上述所有串行步骤之和(全部为磁盘 IO + 服务构建,冷盘/杀软扫描场景下会显著放大)。

**优化方案**(按侵入性递增):
1. 无依赖步骤并行化:`initialize_global_i18n_service`、`AIClientFactory::initialize_global`、`resolve_runtime_log_level` 之间无相互依赖,可 `tokio::join!`;`init_agentic_system` 内部的 `TokenUsageService::new`、`CronService::new` 亦可并行。
2. 把"窗口创建"前移:仅 `initialize_global_config`(主题/语言 bootstrap 需要)是窗口创建的真依赖;可在 config 就绪后立即创建并显示窗口(splash 已自带),其余服务初始化移入后台任务,前端已有 `initialize_workspace_startup_state` 命令兜底(`commands.rs:2024-2047` 注明了 fallback 路径)。需要给未就绪期间到达的 invoke 加"服务就绪门闩"(如全局 `OnceCell`/`Notify` gate)。
3. `prepare_workspace_startup_bootstrap_snapshot` 的 `block_in_place/block_on`(`lib.rs:708-716`)可设超时上限或改为异步注入(前端本就有命令回退路径),避免慢盘工作区把窗口创建拖住。

### 2. 平台字体档案【已解决】

2026-09-02 起，正式桌面构建按目标 triple 选择字体档案：

- Apple 使用系统 `-apple-system` / SF Pro 与系统等宽字体，不输出任何产品文本字体文件。
- Windows/Linux 使用 HarmonyOS Sans Base 与 SC 的 Regular、Medium、Bold 六个未修改 TTF；许可证不允许子集化或格式转换，因此这一档案优先保证授权合规和字形一致性，而不是最小体积。
- `zh-TW` 明确使用系统字体，不内置 TC 字体。
- 编辑器/终端字体与产品正文分离；Fira Code 只进入非 Apple 档案。

`scripts/web-font-profile.mjs` 固化源文件长度和 SHA-256，并在构建时审计字体与法律文件白名单，防止旧字体、TC 字体或非批准格式重新进入产物。

### 3. 桌宠资源 18 MB 全量内置【高收益/中风险(产品决策)】

**证据**:`dist/agent-companion-pets/` 10 个宠物包,每个 1.5~2.4 MB 精灵图(9 个 webp + 1 个 png);`src/web-ui/public/agent-companion-pets` 同源。这是 dist 中最大的单一目录,且多数用户只用 0~1 个宠物。

**方案**:
- 低风险速赢:`panda-pix/spritesheet.png`(1.80 MB)是唯一 PNG,转 webp 预计 -0.9 MB。
- 结构方案:仅内置默认宠物(如 openbitfun),其余改为首次选择时按需下载(项目已有 `import_agent_companion_pet_package` 导入通道,`lib.rs:1040`),安装包 -14~16 MB。需要产品确认离线场景表现。

### 4. Monaco 全量拷贝未裁剪【中收益/低风险】

**证据**:`package.json` `copy-monaco` 脚本把 `monaco-editor/min/vs/**/*` 整体拷入 `public/monaco-editor`(14 MB)。其中:
- 9 个 NLS 语言包,应用仅支持 en-US/zh-CN/zh-TW(`index.html:39-45` 的 locale 解析),`nls.messages.{ru,ja,ko,fr,it,es,de}.js` 共 **1.4 MB 完全未用**(实测 `dist/monaco-editor/vs/`,ru 单个 538 KB)。
- `tsWorker.js` 5.7 MB **确认在用**(`MonacoInitManager.ts:29-30` worker 映射 + `:139-186` 开启了 TS/JS 语义诊断),不可删。
- `verify-monaco-assets.cjs` 只校验 6 个必需文件存在,不做多余文件裁剪。

**方案**:`copy-monaco` 后增加裁剪步骤(或换 copyfiles glob 排除),删除非 zh-cn/zh-tw 的 `nls.messages.*.js`;顺带评估 `basic-languages`(656 KB)中明显不会出现的语言。收益:dist -1.4 MB 起,exe/安装包同步减小。

### 5. 首包 5.44 MB,含三语种 eager 文案【中收益/中风险】

**证据**:
- 入口 chunk `dist/assets/index-DwMxiWtW.js` = 5.44 MB,是第二大 chunk(mermaid.core 491 KB)的 11 倍;桌面端无 HTTP 缓存收益,但 5.4 MB 的 parse/eval 直接落在冷启动关键路径上(WebView2 首次运行无字节码缓存)。
- `src/web-ui/src/infrastructure/i18n/core/I18nService.ts:44-57`:`bootstrapLocaleModules` 用 `import.meta.glob(..., { eager: true })` 把 **3 个语言 × 9 个命名空间**全部内联进入口;实测这批 JSON 源文件共 **793 KB**,而运行时只需要当前语言(~1/3)。
- 已排除其他嫌疑:入口内无 base64 大资源(data URI 合计 78 字节)、无 tiptap/prosemirror/mermaid/monaco/lucide 全量(签名探测均为 0 或个位数命中),lucide 无 `import *` 全量导入。

**方案**:bootstrap 文案按语言拆分——保持 eager 但只对 resolved locale 生效:把 eager glob 改为三个按语言的 `import.meta.glob`(每语言一个 chunk),启动时根据 `__OPENBITFUN_BOOTSTRAP_LOCALE__` 同步选择(Vite 会为每个 glob 生成独立 chunk,当前语言之外的不加载);或将同步 `t()` 依赖的模块初始化改为在 `I18nProvider` ready 后执行。注意与 `startupPerformanceContract.test.ts` 中 "i18n provider 不允许异步 waterfall" 的契约协调,属于中风险改动。

### 6. 未压缩大图 ~2.7 MB【中收益/低风险】

**证据**(`dist/` 根目录 = `public/` 根目录):`OpenBitFun-Logo.png` 1,088,684 B、`panda_full_2.png` 433 KB、`panda_full_1.png` 420 KB、`Logo-ICON.png` 391 KB、`panda_1/2.png` 各 ~185 KB。使用点仅 `NurseryGallery.tsx` 与 `WelcomePanel.tsx`(展示尺寸远小于原图)。`Logo-ICON.png` 由 `copy-icons` 从桌面 icons 拷来,384 KB 明显未过压缩。

**方案**:oxipng/pngquant 无损+有损压缩,或转 webp 并按实际显示尺寸导出(Logo 预计 1.06 MB → <150 KB)。注意 `Logo-ICON-128.png` 有透明度契约测试(`startupPerformanceContract.test.ts:44-57`),压缩需保 alpha。

### 7. 四套语法高亮引擎并存【中收益/中风险】

**证据**(pnpm-lock.yaml 实测版本):`highlight.js@11.11.1`(直接依赖,`MarkdownEditor.tsx:39`、`PlanViewer.tsx:24`)+ `highlight.js@10.7.3`(react-syntax-highlighter → lowlight 传入)+ `prismjs@1.30.0`(直接依赖,`InlineDiffPreview.tsx:15` 静态 import)+ `prismjs@1.27.0`(react-syntax-highlighter → refractor@3.6.0)。react-syntax-highlighter 本身经 `syntaxHighlighterLoader.ts` 懒加载(好),但四套引擎/两对重复版本仍然都会进入产物(体现为 dist 中数百个语言小 chunk 及 MEditor 442 KB 等)。

**方案**:统一到一套(建议 highlight.js 11 或直接复用 Monaco colorize);至少将 `InlineDiffPreview` 的 `import Prism from 'prismjs'` 改为与 react-syntax-highlighter 同源(refractor)或懒加载,消除双 prismjs。预计 JS 总量 -0.5~1 MB。

### 8. release profile 缺 `panic = "abort"`【中收益/中低风险】

**证据**:根 `Cargo.toml:282-286` `[profile.release]` 已有 `opt-level=3, lto=true, codegen-units=1, strip=true`(体积姿势基本正确),但未设 `panic = "abort"`,unwinding 表与 landing pad 通常占 5~10% 二进制体积。另:本地 `target/release/openbitfun-desktop.exe` 实测 217 MB,若为 `release-fast` profile(strip=false、lto=false,`Cargo.toml:288-293`)构建则不代表发布体积;发布链路(NSIS 90.5 MB)以 `[profile.release]` 为准。

**方案**:`[profile.release]` 增加 `panic = "abort"`。风险点:代码中依赖 `catch_unwind` 的路径会变成直接终止(`lib.rs:316` 有 `setup_panic_hook`,panic hook 本身仍然有效);建议加上后跑一轮桌面回归。注意安装包体积从 0.2.11→0.2.13 两个小版本涨了 9.4 MB,建议在 CI 中加体积基线监控。

### 9. `main.tsx` 静态引入桌宠组件【低收益/低风险】

**证据**:`src/web-ui/src/main.tsx:3` `import AgentCompanionDesktopPet from "./app/components/AgentCompanionDesktopPet/..."`,该组件仅在 `openbitfunWindow === 'agent-companion'` 的独立小窗使用(`main.tsx:346-369`),却随入口 chunk 被主窗口一并加载(连带 `AgentCompanionPet`、其 SCSS 与配置服务)。

**方案**:改为 `lazy(() => import(...))` + Suspense(pet 窗口本就有预载动画兜底),主窗口首包相应减小;主 `App` 分支保持静态。

### 10. vite 未配置 manualChunks(观察项)

**证据**:`src/web-ui/vite.config.ts:95-104` build 段无 `rollupOptions.manualChunks`。实际产物 731 个 chunk,说明路由/面板级动态 import 已广泛使用(AppLayout、设置面板、mermaid、xterm、katex、cytoscape 均为独立懒加载 chunk),自动分包工作正常。桌面端无 CDN 缓存诉求,**不建议为拆而拆**;优先做第 5/9 条"缩小入口内容"即可。若后续做增量更新(差量补丁),再考虑稳定 vendor chunk 命名。

---

## 实施建议清单(可直接派发)

| 任务 | 具体内容 | 涉及文件 | 预期收益 | 风险 |
|---|---|---|---|---|
| T1 平台字体档案(已完成) | Apple 使用系统字体且不携带产品字体；Windows/Linux 内置 HarmonyOS Sans Base + SC 六个未修改 TTF；`zh-TW` 使用系统字体 | `scripts/web-font-profile.mjs`、`src/web-ui/src/font-profiles/`、`src/web-ui/src/assets/fonts/` | Apple 移除产品字体；非 Apple 接受合规字体体积 | 已完成 |
| T2 Monaco NLS 裁剪 | `copy-monaco` 后删除 `nls.messages.{ru,ja,ko,fr,it,es,de}.js`(保留 zh-cn/zh-tw);在 `verify-monaco-assets.cjs` 中加断言防回归 | 根 `package.json` scripts、`scripts/verify-monaco-assets.cjs` | dist -1.4 MB | 低 |
| T3 图片压缩 | oxipng/pngquant 压缩 `public/` 根目录 6 个 PNG(Logo、panda 系列);`panda-pix/spritesheet.png` 转 webp;保持 `Logo-ICON-128.png` 透明度契约测试通过 | `src/web-ui/public/*.png`、`public/agent-companion-pets/panda-pix/` | dist -2.5~3 MB | 低 |
| T4 桌宠按需分发 | 产品确认后:仅内置默认宠物,其余走现有 `import_agent_companion_pet_package` 通道按需获取 | `src/web-ui/public/agent-companion-pets/`、桌宠选择 UI | 安装包 -14 MB | 中(需产品确认) |
| T5 Rust 启动并行化(第一步,低风险) | `run()` 内用 `tokio::join!` 并行 i18n/AIClientFactory/log-level 三步;`init_agentic_system` 内 TokenUsage/Cron 并行;为 `prepare_workspace_startup_bootstrap_snapshot` 的 block_on 加超时回退 | `src/apps/desktop/src/lib.rs` | 冷启动前段缩短(IO 并行) | 中低 |
| T6 Rust 窗口前移(第二步,需设计) | config 就绪后立即创建并显示主窗口,agentic/AppState 等移到窗口创建后台;为 invoke 增加服务就绪 gate;用现有 startup_trace 对比前后指标 | `src/apps/desktop/src/lib.rs`、`theme.rs`、`api/commands.rs` | 首帧出现时间大幅提前(高) | 中 |
| T7 i18n 首包按语言拆分 | `bootstrapLocaleModules` 按语言拆为独立 eager glob/chunk,启动仅加载 `__OPENBITFUN_BOOTSTRAP_LOCALE__` 对应语言;同步更新 `startupPerformanceContract.test.ts` 契约 | `src/web-ui/src/infrastructure/i18n/core/I18nService.ts`、契约测试 | 首包 -0.5 MB、减 parse | 中 |
| T8 桌宠组件懒加载 | `main.tsx` 中 `AgentCompanionDesktopPet` 改 `React.lazy`,pet 窗口加 Suspense fallback(复用现有预载动画) | `src/web-ui/src/main.tsx` | 主窗口首包减小 | 低 |
| T9 panic=abort | `[profile.release]` 加 `panic = "abort"`,全量桌面冒烟(重点:插件、panic hook、崩溃诊断路径) | 根 `Cargo.toml` | exe -5~10% | 中低 |
| T10 高亮引擎统一 | 消除双 prismjs(InlineDiffPreview 改用 refractor 或懒加载),规划 highlight.js/react-syntax-highlighter 二选一 | `src/web-ui/src/flow_chat/components/InlineDiffPreview.tsx` 等 | JS -0.5~1 MB | 中 |
| T11 体积基线监控 | CI 增加 dist 分目录与安装包体积基线对比(0.2.11→0.2.13 已 +9.4 MB 无告警) | `scripts/ci/` | 防回归 | 低 |

建议后续实施顺序:T2/T3(纯资源,低风险速赢)→ T5 → T8 → T9 → T7 → T6(收益最大但需设计评审)→ T4/T10/T11。
