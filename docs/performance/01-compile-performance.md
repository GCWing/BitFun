# BitFun 编译/构建性能审阅报告

- 审阅日期:2026-07-26
- 审阅范围:Rust workspace(36 个 crate + 5 个 app,约 57.8 万行 Rust)、前端 pnpm workspace(web-ui 1676 个 TS/TSX 文件、约 36.7 万行)、构建脚本(scripts/)、CI(.github/workflows)
- 审阅方式:只读静态分析 + 轻量命令(Cargo.lock 解析、LOC 统计、dist 产物检查),未运行完整编译
- 环境:Windows 11,rustc/cargo 1.97.0,无 `.cargo/config.toml`、无 `rust-toolchain.toml`,PATH 中未发现 sccache/lld-link

---

## 一、发现总览表(按预期收益排序)

| # | 发现 | 维度 | 预期收益 | 主要证据 |
|---|------|------|---------|---------|
| F1 | Monaco 被"双份打包":ESM 全量打进主 JS chunk,同时又以 AMD 形式复制到 public 运行时加载 | 前端 | 高 | `src/web-ui/src/tools/editor/core/MonacoEditorCore.tsx:11` 等 10+ 处 value import;`dist/assets/index-DwMxiWtW.js`(5.4MB)含 monaco 核心签名 |
| F2 | `build:web` 串行 type-check(366k 行、无 incremental),打包 CI 还重复执行一次 | 前端/CI | 高 | `package.json:49`、`src/web-ui/tsconfig.json`(无 incremental)、`.github/workflows/desktop-package.yml:225` |
| F3 | `bitfun-core` 巨石 crate(202k 行 / 469 文件),任何改动触发整 crate + 下游全量重编 | Rust | 高 | `src/crates/assembly/core`(LOC 统计) |
| F4 | release profile 全量 LTO + codegen-units=1 用于所有平台打包,仅 Linux aarch64 例外用了 thin | Rust/CI | 高 | `Cargo.toml:282-286`、`.github/workflows/desktop-package.yml:106` |
| F5 | 本地 dev 构建:全量 debuginfo + MSVC 默认链接器,`tauri dev` 路径未套用 dev.cjs 的加速环境变量 | Rust | 高 | `Cargo.toml:279-280`、`scripts/dev.cjs:346-351` vs `scripts/dev.cjs:749-767`、无 `.cargo/config.toml` |
| F6 | Cargo.lock 被 gitignore,CI 每次 `cargo generate-lockfile` → 依赖漂移 + rust-cache 频繁失效 | Rust/CI | 中-高 | `.gitignore:26`、`.github/workflows/ci.yml:47-48,136-137` |
| F7 | CI:`rust-build-check` 串行等待 `frontend-build`(含大量 lint/audit/test)完成才开始 | CI | 中 | `.github/workflows/ci.yml:72` |
| F8 | 每次 `desktop:dev` 无条件重建 mobile-web(先删 target 内产物 + pnpm install + vite build) | 脚本 | 中 | `scripts/dev.cjs:704-714`、`scripts/mobile-web-build.cjs:75-133` |
| F9 | 依赖树重复严重:1177 个包中 112 个名字存在多版本(image、thiserror、rand×3、getrandom×4、windows-sys×6、phf×6 等) | Rust | 中 | Cargo.lock 解析;`src/apps/desktop/Cargo.toml:85`(image 0.24 vs workspace 0.25) |
| F10 | reqwest TLS 单栈治理（已完成） | Rust | 已兑现 | workspace transport-only + 客户端 owner 显式 Rustls；见 F10 治理结果 |
| F11 | Vite dev watcher 强制 usePolling + 100ms 轮询,Windows 上 CPU 高、拖慢 HMR | 前端 | 中 | `src/web-ui/vite.config.ts:68-74` |
| F12 | prompts/公告 md 内嵌进 bitfun-core 的 build.rs 生成代码,改一个提示词 = 重编 202k 行 core + 下游 | Rust | 中 | `src/crates/assembly/core/build.rs:110-183,343-401` |
| F13 | beforeBuildCommand 内 web 构建与 mobile-web 构建纯串行;dev.cjs 准备步骤也全串行 | 脚本 | 中 | `src/apps/desktop/tauri.conf.json`(build 块)、`scripts/dev.cjs:668-737` |
| F14 | tokio 全 workspace 开 `full` feature;tauri 开 `unstable`;个别重依赖(oxc、rquickjs、git2 vendored、sherpa-onnx)集中于少数 feature | Rust | 低-中 | `Cargo.toml:72,149,178-180,183,248` |
| F15 | tsconfig(web-ui/mobile-web)未启用 incremental;type-check 每次冷启动 | 前端 | 低-中 | `src/web-ui/tsconfig.json`(全文无 incremental) |
| F16 | copy-monaco 在 postinstall / prebuild / dev.cjs 三处重复执行(14MB / 103 文件,单次成本小) | 脚本 | 低 | `package.json:10,16,45`、`scripts/dev.cjs:669` |
| F17 | pnpm 结构小问题:tests/e2e 已在 workspace 内仍单独 install;installer 独立 Rust workspace 导致 Tauri 栈本地编译两份 | 结构 | 低 | `pnpm-workspace.yaml`、`package.json:92`、`Cargo.toml:40-42` |
| F18 | build.rs 生成代码遍历 HashMap,输出字节序不确定 → 不可复现构建,削弱 sccache/远端缓存效果 | Rust | 低 | `src/crates/assembly/core/build.rs:303,429`、`src/apps/cli/build.rs` |
| F19 | `[profile.dev] incremental = true` 为默认值,冗余;dev profile 无任何针对性调优 | Rust | 低 | `Cargo.toml:279-280` |

---

## 二、逐条详情

### F1(高)Monaco 双份:ESM 全量进主 chunk + AMD 副本运行时再加载

**问题描述**
项目的 Monaco 策略是:`pnpm run copy-monaco` 把 `monaco-editor/min/vs`(AMD 版,14MB/103 文件)复制到 `src/web-ui/public/monaco-editor`,运行时由 `@monaco-editor/react` 的 `loader.init()` 从该路径加载(`src/web-ui/src/tools/editor/services/MonacoInitManager.ts:11,71-75`)。但同时有 10+ 个源文件对 `monaco-editor` 做了 **value import**(非 `import type`):

- `src/web-ui/src/component-library/components/CodeEditor/CodeEditor.tsx:7`
- `src/web-ui/src/shared/helpers/MonacoHelper.ts:3`
- `src/web-ui/src/tools/editor/core/MonacoEditorCore.tsx:11`(`monaco.editor.create(...)` 实际调用,见 213 行)
- `src/web-ui/src/tools/editor/components/CodeEditor.tsx:10`(814 行 `monaco.editor.create`)
- `src/web-ui/src/tools/editor/services/MonacoModelManager.ts:12`(95 行 `monaco.editor.onWillDisposeModel`)等

vite.config.ts 没有为 `monaco-editor` 做 alias/external 处理,Rollup 会解析到 `monaco-editor` 的 ESM 入口(完整 editor),整包被打进**入口 chunk**。产物证据:`dist/assets/index-DwMxiWtW.js` 高达 **5.4MB**,其中含有 Monaco 内核独有的类名字符串 `monaco-mouse-cursor-text`(仅在 `index-DwMxiWtW.js` 与 monaco 的 CSS 中出现),且 `MonacoEnvironment` 也出现在该 chunk。即:**构建期要多打包压缩 ~3-4MB 的 Monaco ESM,运行期首屏要多下载解析这份代码,然后再从 /monaco-editor 加载第二份 AMD Monaco**。

**预期收益**:高。入口 chunk 缩小约 60-70%,vite build(压缩阶段)时间显著下降,应用启动时间同步受益。

**优化方案(二选一)**
- 方案 A(推荐,改动小):所有 `import * as monaco from 'monaco-editor'` 改为 `import type * as monaco from 'monaco-editor'`,运行时实例统一从 `MonacoInitManager.initialize()`/`loader.init()` 的返回值获取(现有单例已具备);同时把 `monaco-editor/min/vs/editor/editor.main.css` 的 import 保持不变(纯 CSS)。用 `scripts/report-web-bundle-size.cjs` 验证 index chunk 是否缩回。
- 方案 B(彻底,改动大):放弃 AMD 副本与 copy-monaco,全面走 Vite ESM + `?worker` 方式打包 Monaco 与其 worker,由 Vite 做 code-split(`monaco` 单独 chunk,懒加载)。

### F2(高)build:web 串行 type-check、无增量,CI 打包重复执行

**问题描述**
`package.json:49`:`"build:web": "pnpm run type-check:web && pnpm --dir src/web-ui build && pnpm run verify:monaco-assets"`。`type-check:web` 即 `tsc --noEmit`(`src/web-ui/package.json:16`),对 1676 个文件/36.7 万行做全量检查,且 `src/web-ui/tsconfig.json` 未开启 `incremental`(仅 `tsconfig.node.json:3` 有 composite)。Vite 构建本身不依赖 tsc(esbuild 转译),两者完全可并行。更严重的是打包 CI 里重复执行:`.github/workflows/desktop-package.yml:225` 先跑 `pnpm run type-check:web`,随后 228 行的 build_command → `desktop:build` → `tauri build` 的 `beforeBuildCommand`(`src/apps/desktop/tauri.conf.json`)= `pnpm run build:web && ...` → **再次 type-check**。5 个平台矩阵各多花一次全量 tsc。

**预期收益**:高。本地 `build:web` 墙钟时间约减 30-50%;打包 CI 每个平台省一次全量 tsc(约 1-3 分钟 × 5 平台)。

**优化方案**
1. `build:web` 改为并行:`concurrently "pnpm run type-check:web" "pnpm --dir src/web-ui build"`(或用 `npm-run-all --parallel`),verify 放最后。
2. `src/web-ui/tsconfig.json` 增加 `"incremental": true, "tsBuildInfoFile": "node_modules/.cache/tsbuildinfo"`(noEmit + incremental 在 TS 5.x 合法)。
3. desktop-package.yml 中二选一:删除 225 行独立 type-check,或给 desktop:build 提供跳过 type-check 的入口(例如 `BITFUN_SKIP_TYPECHECK=1` 时 build:web 只跑 vite build)。

### F3(高)bitfun-core 巨石 crate 是 Rust 增量编译瓶颈

**问题描述**
LOC 统计(只算各 crate `src/`):`src/crates/assembly/core` **202,735 行 / 469 文件**,是第二名(services-integrations 72k)的近 3 倍;bitfun-desktop 依赖它(`src/apps/desktop/Cargo.toml:22`)。rustc 的编译单元是 crate:core 内任何一行改动都会重新编译整个 202k 行 crate(增量编译可缓解 codegen,但 MIR/借用检查/单体化与下游 `bitfun-desktop`(63k 行)的重编译+重链接不可避免)。相比之下 workspace 其他 crate 拆分粒度合理(contracts/adapters/execution 层多为 1-20k 行)。此外 core 的 `Cargo.toml` 显示它同时聚合了 sqlite(`rusqlite bundled`)、MCP 客户端、调试 HTTP 服务器、git2 等,大量子域仍在一个编译单元内。

**预期收益**:高(日常增量编译;长期工程)。

**优化方案(渐进式)**
1. 先用 `cargo build --timings` 定位 core 的编译耗时占比,确认瓶颈(低风险、只读)。
2. 按现有目录边界(`src/agentic`、`src/service/announcement`、sqlite 存储层、debug-log server 等)把相对独立、低耦合的子模块下沉为新 crate(workspace 已有清晰分层惯例)。优先拆"改动频繁"与"几乎不变"的两极模块。
3. 拆分时保持 re-export(`pub use`)以最小化调用方改动。

### F4(高)release 全量 LTO + codegen-units=1 拖慢所有打包构建

**问题描述**
`Cargo.toml:282-286`:`[profile.release] opt-level=3, lto=true(fat), codegen-units=1, strip=true`。fat LTO + CGU=1 使最终 crate 的 codegen/链接几乎完全单线程化,是 release 构建时长的最大放大器;desktop-package.yml 5 个平台打包全部使用该 profile,唯独 Linux aarch64 因慢而被显式覆盖为 `CARGO_PROFILE_RELEASE_LTO=thin`(`.github/workflows/desktop-package.yml:106`)——说明团队已验证 thin LTO 可行。thin LTO 通常仅比 fat LTO 损失 0-2% 运行性能,但构建时间可减 30-60%。

**预期收益**:高(打包 CI 与本地 release 构建)。

**优化方案**
1. `[profile.release]` 改为 `lto = "thin"`;codegen-units 可保守维持 1,或放宽到 16 换更多并行(先 A/B 对比二进制体积与 e2e perf 基线,仓库已有 `e2e:test:perf:release-fast` 类基础设施)。
2. 移除 desktop-package.yml:106 的特例环境变量(统一后不再需要)。
3. 若担心性能回退,可只在 nightly/desktop-package 的非发布分支先切 thin,观察一个周期。

### F5(高)本地 dev 循环:全量 debuginfo + 默认 MSVC 链接器

**问题描述**
`[profile.dev]` 只有 `incremental = true`(`Cargo.toml:279-280`,本身是默认值),debuginfo 为默认 full。Windows 上每次增量构建的大头是 link.exe 重链 bitfun-desktop(数百依赖 + 63k 行 app crate)并重写巨型 PDB。dev.cjs 已经意识到这一点——`rebuildDesktopDebugBinary()` 设置 `CARGO_PROFILE_DEV_DEBUG=0、CODEGEN_UNITS=256`(`scripts/dev.cjs:346-351`),**但只作用于 desktop-preview 路径**;最常用的 `desktop:dev`(`tauri dev`,`scripts/dev.cjs:749-767`)与 `desktop:dev:raw` 完全没有这些环境变量,仍是全量 debuginfo。仓库也没有 `.cargo/config.toml`,未启用任何链接器优化(rust-lld)或编译缓存(sccache)。

**预期收益**:高(日常 Rust 改动的"改一行到重启应用"时间,链接期通常可减 30-60%)。

**优化方案**
1. 低风险:在 `Cargo.toml` `[profile.dev]` 显式设置 `debug = "line-tables-only"`(保留 panic 栈回溯行号,PDB 大幅缩小);需要完整调试时用 `CARGO_PROFILE_DEV_DEBUG=2` 临时覆盖。
2. 或者把 dev.cjs 的三个 CARGO_PROFILE_DEV_* 环境变量同样注入 `desktop:dev` 的 `tauri dev` 进程(与 preview 路径一致,改动只在脚本层)。
3. 中风险:新增 `.cargo/config.toml`,对 `x86_64-pc-windows-msvc` 设置 `linker = "rust-lld"`(随 rustup 分发,无需额外安装);先在本地验证 tauri/webview2 链接参数兼容后再提交。
4. 可选:安装 sccache 并设 `RUSTC_WRAPPER`(注意 sccache 与 incremental 互斥,更适合 CI/冷构建场景,见 F18 的可复现性前提)。

### F6(中-高)Cargo.lock 不入库,CI 每次重新解析依赖

**问题描述**
`.gitignore:26` 忽略了 `Cargo.lock`;CI 各 Rust job 先 `cargo generate-lockfile` 再 `--locked`(`.github/workflows/ci.yml:47-48,136-137`),`src/apps/desktop/Cargo.toml:48-53` 的注释也明说"CI 有意忽略根 Cargo.lock"。后果:(a) 任何上游依赖发新版本都会改变解析结果,swatinem/rust-cache 以 lockfile 哈希为 key,缓存频繁整段失效,依赖需从零重编;(b) 构建不可复现,还被迫用 `=x.y.z` 硬钉住问题依赖(`Cargo.toml:98-107` 的 time/brotli/bitflags 补丁群);(c) 本地与 CI 依赖树可能不一致。对应用型(非库)workspace,Cargo 官方建议提交 lockfile。

**预期收益**:中-高(CI 稳定性与缓存命中率;偶发的"上游发版导致全量重编/编译失败"归零)。

**优化方案**
1. 从 .gitignore 移除 Cargo.lock 并提交(根 workspace 与 BitFun-Installer/src-tauri 各一份);CI 删除 `cargo generate-lockfile` 步骤,直接 `--locked`。
2. 依赖更新改为显式动作(Renovate/Dependabot 或定期 `cargo update` PR),届时可逐步解除 `=` 钉版。
3. 风险:改变现行"自动吃最新补丁版本"策略,需团队确认;属流程变更而非代码变更。

### F7(中)CI 拓扑:Rust 检查串行排在完整前端流水线之后

**问题描述**
`.github/workflows/ci.yml:72` `rust-build-check.needs: frontend-build`。frontend-build 包含 hygiene/boundaries/i18n/theme 审计、eslint、vitest、build:web、mobile-web 构建等十几个串行步骤,全部完成后三平台 Rust job 才开始。Rust 侧对前端的真实依赖只有 `dist/` 存在(tauri-build 校验 frontendDist)与 `src/mobile-web/dist` 目录(CI 已用 `mkdir -p` 打桩,ci.yml:99-101)。

**预期收益**:中(PR 反馈总时长,估计缩短 5-15 分钟视 frontend job 时长)。

**优化方案**
- 将 `frontend-build` 拆成 `frontend-checks`(lint/audit/test)与 `frontend-dist`(仅 `pnpm install + build:web + build:mobile-web` 上传产物)两个 job:Rust job 只 `needs: frontend-dist`;或者更激进——Rust check/test 根本不需要真实 dist,给 `dist/` 也 mkdir 打桩即可完全并行(需验证 tauri-build 仅检查目录存在)。

### F8(中)desktop:dev 每次冷启动都全量重建 mobile-web

**问题描述**
`scripts/dev.cjs:704-714` 在每次 `desktop:dev` 启动时调用 `buildMobileWeb({install:true})`;`scripts/mobile-web-build.cjs` 无任何变更检测:75-94 行先删除所有 `target/*/mobile-web` 副本,109 行无条件 `pnpm install`,122 行无条件 `vite build`。mobile-web 源码不变时这是纯浪费(install + vite build 通常 20-60s),且删除 target 副本会迫使桌面端重新拷贝。`lint:rs:desktop`(package.json:41)同样每次先跑 `prepare:mobile-web`。

**预期收益**:中(每次 desktop dev 启动省 20-60s)。

**优化方案**
- 在 buildMobileWeb 中加入 mtime/hash 短路:比较 `src/mobile-web/{src,public,package.json,vite.config.*}` 最新 mtime 与 `src/mobile-web/dist` 构建标记(dev.cjs 已有同型实现 `getDesktopPreviewRebuildPlan`,`scripts/dev.cjs:420-456`,可复用);dist 有效时跳过 clean/install/build。提供 `--force` 逃生口。

### F9(中)重复依赖:112 个包存在多版本

**问题描述**
Cargo.lock 共 1177 个包,其中 112 个名字存在 2 个以上版本(解析自本地 Cargo.lock)。重点:
- `image 0.24.9 + 0.25.10`:**根因在自家代码**——workspace 定义 image 0.25(`Cargo.toml:108`),但 `src/apps/desktop/Cargo.toml:85` 单独写死 `image = "0.24"`,两份 image(含 png/jpeg 解码器栈)都要编译。
- `rand 0.7/0.8/0.9`、`getrandom ×4`、`thiserror 1+2`、`syn 1+2`、`windows-sys ×6`、`windows-targets ×4`、`phf ×6`、`toml/toml_edit ×3`、`zbus 4+5`、`which 4+8`、`portable-pty 0.8+0.9`、`nix ×4` 等,多数由第三方传递引入,但每个多版本都是一份额外编译时间与 target 体积。

**预期收益**:中(冷构建时间与 target 体积;image 一项立收)。

**优化方案**
1. 立即:desktop 的 image 改回 `image = { workspace = true }`(0.25 同样支持 png/jpeg feature 子集),验证调用点 API 兼容。
2. 运行 `cargo tree -d -e normal --workspace`(建议加 `--target x86_64-pc-windows-msvc` 过滤无关平台)输出清单,针对 top 传递源头(如 pull 出 rand 0.7 的 crate)评估升级;`russh 0.45`、`screenshots 0.8`、`syntect-tui` 等旧版是主要嫌疑。
3. 将该清单纳入 nightly CI 做趋势看护(非阻断)。

### F10(已治理)reqwest TLS 单栈

**治理结果（2026-08-03）**
- workspace 级 `reqwest` 只保留 HTTP/序列化能力，URL-only 消费者不再被动加载 TLS；CLI、Desktop、AI Adapter、MiniApp Market 与 Services 的各 Reqwest owner feature 显式选择 Rustls。
- Reqwest 0.13 的 `rustls` feature 使用平台证书验证器，保留系统信任根行为；三处显式 Client Builder 统一使用 `tls_backend_rustls()`，不再混用默认后端或弃用 API。
- `Cargo.lock` 已移除 `native-tls`、`hyper-tls`、`tokio-native-tls`、`openssl` 与 `openssl-macros`；`openssl-sys` 只剩非 Windows 的 Git/libgit2 目标路径。Windows 产品图、本地开发、常规构建和三个既有 workflow 不再下载或配置预编译 OpenSSL。
- 边界检查约束 TLS 必须由真实客户端 owner 选择，避免未来把后端重新提升为 workspace 全局 feature；没有新增 CI job 或测试步骤。

**收益与风险**:Windows CI 每个受影响 job 省去约 6.6 秒 OpenSSL bootstrap（以 PR #1991 的 Windows job 步骤时间戳为基线），并减少一套 TLS 依赖闭包。显式代理配置仍由 Reqwest 处理；未受系统信任的自签证书仍会按安全默认值拒绝。仓库没有 Reqwest client identity 或自定义 native connector 用法，因此未保留第二后端兼容层。

### F11(中)Vite dev 强制 100ms 轮询 watch

**问题描述**
`src/web-ui/vite.config.ts:68-74`:`watch: { usePolling: true, interval: 100 }`,注释称为 Windows 稳定性。对 1676 个源文件 + node_modules 的轮询每 100ms 扫一轮,持续占用 CPU(常见 5-15% 单核),还与 Vite 官方建议相悖(Windows 原生 fs 事件在本地盘上是可靠的,轮询主要用于网络盘/WSL 挂载)。

**预期收益**:中(dev 机器 CPU/风扇/电池,HMR 延迟)。

**优化方案**
- 默认关闭 `usePolling`(删除该配置),仅当 `process.env.VITE_USE_POLLING` 显式设置时启用轮询作为逃生口;若必须保留轮询,interval 提到 ≥1000ms。

### F12(中)提示词/公告内容内嵌进 bitfun-core,内容改动触发核心重编

**问题描述**
`src/crates/assembly/core/build.rs` 把 `src/agentic/**/prompts` 与 announcement 的 md/txt 全文生成为巨型 `map.insert(r###"..."###, ...)` Rust 源码(110-183、343-401 行),并对目录逐文件 `rerun-if-changed`。改动任何一个提示词 md → build.rs 重跑 → 生成文件变化 → **202k 行的 bitfun-core 全量重编 + 下游 desktop 重编重链**。提示词是高频改动内容,与代码逻辑无关。

**预期收益**:中(提示词迭代场景的编译时间从"分钟级"降到"秒级")。

**优化方案**
1. 首选:改用 `include_dir!`(workspace 已有 `include_dir = "0.7"` 依赖,Cargo.toml:112)直接内嵌目录,删除 build.rs 的代码生成路径——虽然内容变化仍触发重编,但可把提示词嵌入下沉到一个**只包含内嵌资源的叶子 crate**,注意:Rust 中依赖变更仍会传递重编下游,所以真正的解法是第 2 条。
2. dev 构建从文件系统运行时读取(以 `CARGO_MANIFEST_DIR` 定位,debug_assertions 下启用),release 才走内嵌;这样日常提示词调优完全不触发 Rust 重编。风险:dev/release 行为差异,需保留 CI 中 release 路径的冒烟验证。

### F13(中)构建/启动编排中的串行步骤

**问题描述**
- `src/apps/desktop/tauri.conf.json` build 块:`beforeBuildCommand: "pnpm run build:web && pnpm run prepare:mobile-web"` —— web-ui 构建、mobile-web 构建互相独立却串行;且它们整体又发生在 cargo 编译之前(tauri CLI 约束,前端与 Rust 无法并行)。
- `scripts/dev.cjs:668-737`:copy-monaco → generate-version → mobile-web → flashgrep 全串行,彼此无依赖。

**预期收益**:中(desktop:build 总时长减 1-3 分钟;dev 启动准备段减半)。

**优化方案**
1. beforeBuildCommand 改为并行封装脚本(Node 内 `Promise.all` 两个子进程,注意日志前缀区分)。
2. dev.cjs 的四个准备步骤用 `Promise.all` 并行(copy/generate/flashgrep 都是纯本地 IO)。
3. 进阶:绕开 tauri CLI 的串行约束——desktop-tauri-build.mjs 先并行启动"前端构建"与"cargo build(直接 cargo,不经 tauri)",最后再让 `tauri build` 复用增量结果;实现复杂度较高,建议放到后期。

### F14(低-中)feature 面偏大与重量级依赖

**问题描述**
- `tokio = { features = ["full"] }`(Cargo.toml:72)全 workspace 生效,包含 io-std/signal/process 等未必全用的模块(tokio 编译不算大头,收益有限)。
- `tauri` 开 `unstable` + `tray-icon` + `macos-private-api`(183 行)——`unstable` 为多 webview 所需,合理但注意跟踪。
- 重量级依赖:`oxc`(JS 编译器,178 行,canvas-runtime feature)、`rquickjs`(C 的 QuickJS,180 行)、`git2 vendored-libgit2`(149 行,冷构建编译整个 libgit2 C 库)、`sherpa-onnx`(248 行,speech feature,已用 prebuilt 缓解,target 下有 sherpa-onnx-prebuilt)。desktop 默认开 `canvas-runtime + speech`(`src/apps/desktop/Cargo.toml:27`)。

**预期收益**:低-中(主要影响冷构建)。

**优化方案**
- tokio 换成显式 feature 列表(rt-multi-thread、macros、fs、net、io-util、sync、time、process、signal 按需);一次性梳理,风险低但触碰面广,建议用 `cargo check --workspace` 验证。
- 为 `canvas-runtime`/`speech` 评估"开发期默认关闭"的 dev feature 预设(例如 desktop `default = []` 已是如此,可提供 `desktop:dev:lite` 脚本传 `--no-default-features` 组合),按需取舍。

### F15(低-中)tsc 无增量缓存

见 F2 方案 2;`src/mobile-web/tsconfig.json` 同样处理。单独列出是因为即使不动 build:web 编排,本地反复 `type-check:web` 也能从 incremental 获益(二次检查通常快 3-10 倍)。

### F16(低)copy-monaco 重复执行

**问题描述**:`package.json:10,16,45` + `scripts/dev.cjs:669`,postinstall、prebuild:web、dev 启动三处都会全量复制。实测量级仅 14MB/103 文件,单次秒级,优先级低。
**优化方案**:在 copy 脚本中比对 monaco-editor 版本号(package.json vs 目标目录内 marker 文件),相同则跳过。

### F17(低)pnpm/workspace 结构小问题

- `tests/e2e` 已列入 `pnpm-workspace.yaml`,根 `pnpm install` 已装依赖,但仍保留 `e2e:install`(package.json:92)单独 install 入口,易造成双份状态;website 不在 workspace(独立 install),属有意隔离可保留。
- `BitFun-Installer/src-tauri` 是独立 Rust workspace(根 Cargo.toml:40-42 exclude),与主 workspace 各自维护 target,Tauri 全家桶在本地要编两份。可评估共享 `CARGO_TARGET_DIR`(风险:两个 workspace 依赖版本不同会互相踩缓存,需先对齐版本)或接受现状。

### F18(低)build.rs 生成代码不可复现

**问题描述**:`assembly/core/build.rs:303,429` 与 `src/apps/cli/build.rs` 遍历 `HashMap` 生成 `map.insert(...)` 行,顺序随机。同样输入两次构建产出字节不同的生成文件,破坏可复现构建,也让 sccache/远端缓存对相关 crate 失效。
**优化方案**:改用 `BTreeMap` 或收集后 `sort`,一行级改动、零风险。

### F19(低)profile.dev 冗余配置

`Cargo.toml:279-280` 的 `incremental = true` 是 dev 默认值,可删;该段落是放置 F5 建议(`debug = "line-tables-only"`)的天然位置。

---

## 三、实施建议清单(可直接派发给实施 agent)

按建议实施顺序排列;每条含验收标准与风险等级。

| 任务 | 内容 | 涉及文件 | 风险 |
|------|------|---------|------|
| T1 | Monaco 去重(F1 方案 A):将 `src/web-ui/src` 下所有 `import * as monaco from 'monaco-editor'` 改为 `import type`,运行时实例统一经 `MonacoInitManager.initialize()` 注入(必要处传参或增加 `getMonaco()` 访问器);保留 `editor.main.css` import 与 AMD loader 流程。验收:`pnpm run build:web` 后 `dist/assets/index-*.js` 体积从 ~5.4MB 降至 ~2MB 以下,`grep -L monaco-mouse-cursor-text dist/assets/index-*.js`,编辑器与 diff 功能手测正常。 | `src/web-ui/src/**`(约 10-14 个文件)、`vite.config.ts` | 中(触碰编辑器核心路径,需手测 CodeEditor/DiffEditor/主题/worker) |
| T2 | build:web 并行化 + tsc 增量(F2/F15):a) 根 package.json `build:web` 改为并行执行 type-check 与 vite build(推荐 `npm-run-all2 --parallel` 或自写 Node 封装,任一失败即整体失败);b) `src/web-ui/tsconfig.json`、`src/mobile-web/tsconfig.json` 加 `"incremental": true` 与 tsBuildInfoFile(放 node_modules/.cache),并确认 .gitignore 覆盖;c) `.github/workflows/desktop-package.yml` 删除独立的 `pnpm run type-check:web` 步骤(225 行)。验收:build:web 总时长下降;CI 打包日志中 tsc 只出现一次。 | `package.json:49`、`src/web-ui/tsconfig.json`、`src/mobile-web/tsconfig.json`、`.github/workflows/desktop-package.yml` | 低 |
| T3 | release 改 thin LTO(F4):`Cargo.toml` `[profile.release]` `lto = "thin"`;删除 desktop-package.yml:106 的 `CARGO_PROFILE_RELEASE_LTO=thin` 特例。验收:三平台打包成功,二进制体积增幅 <5%,`e2e:test:perf:release-fast` 基线无回退(注意 release-fast 继承 release 后 `lto=false` 覆盖不受影响)。 | `Cargo.toml:284`、`.github/workflows/desktop-package.yml:106` | 中(需一轮打包验证 + 性能基线对比) |
| T4 | dev 构建 debuginfo 裁剪(F5/F19):`Cargo.toml` `[profile.dev]` 增加 `debug = "line-tables-only"`,删除冗余 `incremental = true`;在 `scripts/dev.cjs` 的 `desktop:dev`(tauri dev)路径注入与 preview 相同的 `CARGO_PROFILE_DEV_CODEGEN_UNITS=256`(允许 env 覆盖)。文档注明"需要断点调试时 `set CARGO_PROFILE_DEV_DEBUG=2`"。验收:改动一个 desktop crate 文件后的增量重链时间下降;panic 栈仍含行号。 | `Cargo.toml:279-280`、`scripts/dev.cjs:749-767` | 低(调试体验有权衡,需在 README/AGENTS 说明) |
| T5 | mobile-web 构建短路(F8):在 `scripts/mobile-web-build.cjs` 增加输入 mtime 检测(参考 `dev.cjs:420-456` 的实现),dist 新于全部输入时跳过 clean/install/build;`--force`/env 逃生口;dev.cjs 与 lint:rs:desktop 路径自动受益。验收:连续两次 `desktop:dev` 第二次跳过 mobile-web 构建;修改 mobile-web 源码后正确重建。 | `scripts/mobile-web-build.cjs`、`scripts/dev.cjs` | 低 |
| T6 | 修复 image 双版本(F9):`src/apps/desktop/Cargo.toml:85` 改为 `image = { workspace = true }`,如 API 不兼容则升级调用点到 0.25。验收:Cargo.lock 中 image 仅剩 0.25.x;`cargo check -p bitfun-desktop` 通过。 | `src/apps/desktop/Cargo.toml:85` | 低 |
| T7 | CI 拓扑并行化(F7):拆分 ci.yml 的 frontend-build 为 `frontend-checks`(lint/audit/test,不阻塞他人)与 `frontend-dist`(install + build:web + build:mobile-web + upload);`rust-build-check` 改 `needs: frontend-dist`。进一步验证 `cargo check/test` 是否只需 dist 目录存在——若是,用 mkdir 打桩彻底解除 needs。验收:PR 上 Rust job 提前开始,总流水线时长下降。 | `.github/workflows/ci.yml` | 中(改 CI 拓扑,需观察 1-2 个 PR) |
| T8 | 提交 Cargo.lock(F6):从 `.gitignore:26` 移除并提交根与 installer 两份 lockfile;CI 删除 `cargo generate-lockfile` 步骤;建立定期依赖更新流程后逐步解除 `Cargo.toml:98-107` 的 `=` 钉版。验收:rust-cache 命中率上升,CI 不再因上游发版突然变慢/失败。 | `.gitignore`、`.github/workflows/*.yml`、`Cargo.toml` | 中(流程变更,需团队确认依赖更新策略) |
| T9 | Vite watch 去轮询(F11):删除 `src/web-ui/vite.config.ts:68-74` 的 `usePolling/interval`,保留 ignored 列表;以 `VITE_USE_POLLING=1` 环境变量作为网络盘用户逃生口。验收:dev server 空闲 CPU 占用明显下降,HMR 正常。 | `src/web-ui/vite.config.ts` | 低(个别特殊文件系统需逃生口) |
| T10 | build.rs 确定性输出(F18):`assembly/core/build.rs` 与 `src/apps/cli/build.rs` 生成代码前对 key 排序(HashMap→BTreeMap)。验收:连续两次 clean build 生成的 OUT_DIR 文件字节一致。 | `src/crates/assembly/core/build.rs`、`src/apps/cli/build.rs` | 低 |
| T11（已完成） | reqwest TLS 单栈(F10):workspace 保持 transport-only，由真实客户端 owner 显式选择 Rustls；删除 native-tls 与无消费者的 Windows OpenSSL bootstrap，保留平台证书验证。验收:Cargo.lock 无 native-tls；Windows 产品图无 openssl-sys，非 Windows 的 Git/libgit2 路径不变；相关最小 feature 与产品入口编译通过。 | 根与客户端 owner `Cargo.toml`、相关调用点、既有 workflow | 已完成；平台信任根行为保留，无第二后端兼容层 |
| T12 | beforeBuildCommand 并行(F13):新增 `scripts/frontend-build-all.mjs` 并行跑 build:web 与 prepare:mobile-web,tauri.conf.json / tauri.dev.conf.json 的 beforeBuildCommand 指向它;dev.cjs 准备步骤改 Promise.all。验收:desktop:build 前端阶段时长≈max(两者) 而非 sum。 | `src/apps/desktop/tauri.conf.json`、`tauri.dev.conf.json`、`scripts/dev.cjs`、新脚本 | 低 |
| T13 | bitfun-core 拆分启动(F3,长期):先跑 `cargo build --timings` 与 `cargo tree -d` 存档基线;选 1-2 个低耦合子域(如 announcement、debug-log server)试点拆出独立 crate 并保留 re-export;结合 F12 的"dev 运行时读取提示词"改造。验收:改动试点子域后 `cargo build -p bitfun-desktop` 的重编 crate 数与耗时下降。 | `src/crates/assembly/core/**`、根 `Cargo.toml` members | 中-高(架构改动,分多个 PR 渐进) |
| T14 | 可选工具链增强(F5):提交 `.cargo/config.toml` 模板(注释形式提供 rust-lld 与 sccache 配置,默认不启用),团队自选开启;CI 冷构建可评估 sccache-action。验收:提供文档,默认行为不变。 | 新增 `.cargo/config.toml`、文档 | 低(默认关闭) |

### 快速收益组合(建议第一批实施)
T2 + T4 + T5 + T6 + T9 + T10 + T12:全部低风险,合计可显著改善日常 dev 循环(启动省 20-60s、增量链接提速、dev CPU 下降)与 build:web 时长;随后再做 T1(最大单项前端收益)、T3(打包 CI)、T7/T8(CI 结构)。

---

## 附:数据快照

- Rust workspace:36 members + installer 独立 workspace;总计约 577,903 行 Rust。Top crate:assembly/core 202,735 行、services-integrations 72,251、desktop 63,887、cli 53,976。本地 `target/` 实测 **98GB**(cargo-target-gc 存在的原因;F5 的 debuginfo 裁剪与 F9 去重也能显著降低该体积)。
- `BitFun-Installer/src-tauri/Cargo.toml:61-65` 的 release profile 同样是 `lto=true + codegen-units=1`(opt-level="z"),T3 的 thin LTO 评估可一并覆盖。
- Cargo.lock:1177 个包,112 个存在多版本(windows-sys ×6、phf ×6、getrandom ×4、nix ×4、rand ×3、quick-xml ×4 等)。
- 前端:web-ui 1676 个 TS/TSX、366,580 行;dist 共 872 个 asset,JS 总量 14.3MB,入口 chunk 5.4MB(含 Monaco 内核);public/monaco-editor 14MB/103 文件。
- 现有良好实践(保持):release-fast profile(Cargo.toml:288-293)、cargo-target-gc 缓存清理、Reqwest Rustls 单栈与平台证书验证、sherpa-onnx prebuilt、CI CARGO_INCREMENTAL=0 + debug=0(ci.yml:74-77)、swatinem/rust-cache、pnpm store 缓存、workspace.dependencies 统一版本声明。
