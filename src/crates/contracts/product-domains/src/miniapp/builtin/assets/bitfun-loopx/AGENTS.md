# AGENTS.md — bitfun-loopx 内置 MiniApp 开发协定

本文件仅适用于本目录（bitfun-loopx 内置 MiniApp 的**唯一权威源码**），比仓库根
`AGENTS.md`、`src/crates/contracts/AGENTS.md` 更具体，冲突时以本文件为准。开发
流程细节见本目录 `README.md`「高效修改流程」，这里只钉住**用户明确要求的迭代原则**。

## 最高优先级：快速反馈迭代（用户要求，不可违背）

1. **不要主动编译**：只有用户明确要求 agent 编译时，才进行构建/编译。被要求编译时，
   构建/编译一律放后台并行执行（background job），不要在等待期间空转；优先完成不依赖
   编译结果的独立工作。
2. **修改代码之后不运行测试或预检查**：默认不跑 `cargo test`、`pnpm test`、
   thin-client / 契约测试、`cargo check`、`pnpm run type-check:web` 等。只有用户
   **明确要求检查或测试**时才运行；用户只说“编译”代表直接产出可运行 exe，不包含预检查。
   - 允许的秒级自检：`node --check ui.js` / `node --check worker.js`、
     肉眼确认 JSON 合法、`git diff --check`。这些不是测试。
3. **改完尽快交付可见结果**：每次修改后，以最快路径让用户看到效果并等待反馈；
   但只有用户明确要求编译时，才执行以下编译与重启步骤：
   - 纯 UI（`index.html` / `style.css` / `ui.js`）：批量做完一轮修改 →
     `node --check` → 用户要求编译后再单次重新编译 Desktop 二进制 → 重启应用；
   - Rust 宿主行为：完成一批修改 → 用户要求编译后直接单次
     `cargo build -p bitfun-desktop --bin bitfun-desktop`（统一配方，见 README
     「统一构建配方」，勿混用不同 profile 环境变量）→ 重启应用。最终 build 本身就是
     Rust 编译验证，不要在它前面重复跑 `cargo check`。
4. **先收集反馈，再继续下一轮**：交付可见结果后停下，等用户反馈；不要自行
   连锁扩展改动范围（"快速迭代"≠"一次改很多"）。

## 分层最小动作（速查）

| 修改文件 | 用户要求编译前可做的最小动作 | 用户明确要求编译后（何时才编译 Desktop） |
|---|---|---|
| `index.html` / `style.css` / `ui.js` | 连续批量编辑；`node --check ui.js` | 单次重新编译 Desktop 二进制，然后重启应用 |
| `worker.js` | `node --check worker.js` | 重新编译并重启 Worker |
| `meta.json` / `esm_dependencies.json` | 检查 JSON 与权限差异 | 编译并 reseed |
| `src/crates/contracts/product-domains/src/miniapp/loopx/**` | 完成一轮修改，等待用户要求；不跑 Cargo 预检查 | 单次构建 Desktop binary，然后重启应用 |
| `src/crates/services/services-integrations/src/miniapp/loopx_*.rs` | 完成一轮修改，等待用户要求；不跑 Cargo 预检查 | 单次构建 Desktop binary，然后重启应用 |
| `src/crates/assembly/core/src/miniapp/loopx/**` | 完成一轮修改，等待用户要求；不跑 Cargo 预检查 | 单次构建 Desktop binary，然后重启应用 |
| `scripts/build-loopx.mjs` / LoopX pin | 不做动作，等待用户要求 | `pnpm run build:loopx`（只在用户要求且 sidecar/pin 变化时） |

> 编译总原则：上表只是「用户明确要求编译时的最小动作」，不代表 agent 可以自行触发编译；
> 默认只有用户明确要求 agent 编译时才编译。

注：宿主目录见 `src/crates/contracts/product-domains/src/miniapp/builtin/assets/bitfun-loopx` 的
上一级（`../../../../../..` 之外的 Rust 目录），完整说明在 `README.md`。

## 快速循环约定

- 通常保持 Web UI Vite 常驻（`pnpm --dir src/web-ui dev`，端口 1422），Frontend
  改动走 HMR；MiniApp 资源因 `include_str!` 内嵌，仍需编译才能进二进制。仅在下方
  Windows 低内存规则触发时临时暂停，构建结束后必须恢复。
- 2026-08-26 失败复盘：只重新编译并启动 `target/debug/bitfun-desktop.exe`，但没有确认
  Web UI Vite 已恢复，会让 Desktop WebView 打开 `localhost` 后显示
  `ERR_CONNECTION_REFUSED`。启动或重启 Desktop 前必须确认 1422 已监听，并做一次 HTTP
  探测；若未监听，先后台启动 `pnpm --dir src/web-ui dev --host 127.0.0.1`，确认
  `http://127.0.0.1:1422/` 返回 200 后再启动 Desktop。
- 每次重新编译前**先停掉正在运行的 `bitfun-desktop.exe`**，避免两个实例抢占
  同一个 AppData / reseed 目录。
- 启动直接用刚编译的 `target/debug/bitfun-desktop.exe`（Vite 保持运行），
  **不要**用 `pnpm run desktop:dev` 反复启停做 UI 微调。
- 编译、启动一律放后台 job；新二进制会按内容哈希自动 reseed `compiled.html`。
- `src/web-ui/**` 改动由常驻 Vite HMR 直接生效；快速反馈流程不要追加
  `pnpm run type-check:web`，也不要因此重新编译 Rust。只有内嵌 MiniApp source 或 Rust
  发生变化时才需要 Desktop binary build。

## 编译影响面约束（写代码前执行）

1. **先判断 Cargo 影响链再编辑**：Rust 的增量单位主要是 crate，不是单个文件。修改
   `product-domains`、`services-integrations` 或 `bitfun-core` 任一项都会让其下游重新编译；
   同一轮同时触及三者会形成 `contracts → services/core → desktop` 的大范围重编。编辑前
   必须列出准备触及的 crate，并确认每一层都是当前可见结果所必需。
2. **UI 问题不扩散到 Rust**：文案、布局、日志展示和交互只改内嵌 MiniApp source；
   `src/web-ui/**` 问题只改 Web UI 并走 Vite HMR。不要为了方便把纯展示逻辑放进 Rust，
   也不要因为 Web UI 改动重新编译 Desktop。
3. **LoopX 私有行为留在最窄 owner**：LoopX 专用投影、去抖、日志摘要和调度逻辑优先
   留在 `src/crates/assembly/core/src/miniapp/loopx/**`。不要顺手修改全局配置、共享事件、
   runtime、Cargo features 或 manifest；只有真实稳定合同属于下层 owner 时才向下修改，
   不得为编译速度破坏正确架构边界。
4. **主流程修复与非阻塞改进分轮**：当前问题能在一个 owner 内闭环时，不把 prompt
   润色、共享重构、通用清理或另一层的体验优化塞进同一次真机反馈 build。记录为下一轮，
   等用户看到主流程效果后再决定是否做。
5. **不制造无关源码变更**：不格式化未触及的 Rust 文件，不调整 Cargo.toml/features，
   不移动模块，不做与当前问题无关的重命名。Cargo 按内容哈希判断脏单元，任何共享源码
   变化都可能扩大下游重编。
6. **批量编辑，一次 binary build**：在不编译的状态下完成同一影响链的全部必要修改，
   秒级检查后只构建一次。不要为了逐文件确认而在中间启动 Cargo。
7. **构建前向用户说明预期影响面**：若不可避免地同时触及多个广泛 crate，编译前简短
   说明为什么无法保持局部，以及预计会重编哪些层。长期需要进一步提速时，应评审把
   LoopX Desktop wiring / embedded assets 从大 crate 拆到更窄的产品 owner；不要临时用
   错误依赖方向规避编译。

## Windows Desktop 编译防重跑规范（2026-08-26 失败复盘）

本机为 16 GB Windows。一次失败流程中，已有的不同 profile `cargo check` 长时间占锁，
随后另一条 `cargo test` 抢占同一 target；默认并发的正式 build 又多次在没有 Rust 诊断时
退出。实测单个 `bitfun-core` `rustc` 工作集接近 5 GB。为避免等待、冷重编和内存峰值，
用户明确要求编译时必须遵守：

1. **编译前双重验锁**：先查看所有 `cargo` / `rustc` 的 PID、命令行和开始时间；已有
   Cargo 时不得再启动 check、test 或 build，也不得终止不属于当前任务的进程。等待其
   结束，并在正式 build 前立即复查一次，确认 target 无竞争者。
2. **极速反馈只运行最终 build**：用户要求“编译看效果”时，不运行 `cargo check`、
   `cargo test`、Web UI type-check 或任何前置构建。它们重复解析/编译依赖，却不产生用户
   要试用的 exe。若用户另外明确要求某个检查，该检查也必须继承 README 的三个
   `CARGO_PROFILE_DEV_*` 值，禁止运行裸 Cargo 命令污染增量指纹。
3. **本机强制单并发**：在统一 profile 之外设置 `CARGO_BUILD_JOBS=1`。该变量只限制
   同时运行的 rustc 数量，不改变 Cargo 指纹；不要用默认并发或 `-j 2` 反复碰内存
   上限。统一命令为：

   ```powershell
   $env:CARGO_PROFILE_DEV_DEBUG = "0"
   $env:CARGO_PROFILE_DEV_INCREMENTAL = "true"
   $env:CARGO_PROFILE_DEV_CODEGEN_UNITS = "256"
   $env:CARGO_BUILD_JOBS = "1"
   cargo build -p bitfun-desktop --bin bitfun-desktop
   ```

   指定 `--bin bitfun-desktop` 用于明确只请求 Desktop binary target，但当前 package 的
   binary 依赖同包 lib，而 `[lib]` 同时声明 `staticlib`、`cdylib`、`rlib`；Cargo 仍会
   在一次 lib rustc 中生成三种 crate-type，并链接约 30 MB 的
   `bitfun_desktop_lib.dll`。不要宣称 `--bin` 已省掉该阶段。真正移除它需要把移动端/FFI
   wrapper 与 Desktop 使用的 rlib 拆成不同 package/target，必须作为独立架构改动评审。

   构建前同时检查系统可用提交空间；低于 2 GB，或单个 rustc 运行期间降到 1 GB 左右时，
   可以临时停止**仅属于本仓库**的 Vite 进程链，构建结束后用原命令恢复。2026-08-26
   实测暂停 Vite 将可用提交空间从约 650 MB 恢复到约 1.8 GB，使最终链接完成。不得为
   编译关闭其他用户应用、Codex 进程或无关服务。

4. **只保留一个可追踪的后台 build**：记录后台 job/session、构建开始时间和输出日志，
   持续轮询同一个句柄直到退出；不得因为暂时没有输出而重复启动。正常构建不要用
   `-vv`，只有无诊断退出时才用它定位最后一个 rustc 命令。
5. **成功必须有四项证据**：Cargo 退出码为 0；输出含 `Finished`；
   `target/debug/bitfun-desktop.exe` 的 `LastWriteTime` 晚于本次构建开始；Cargo/rustc
   已全部退出。缺一项都视为构建失败，不得启动旧 exe 冒充新版本。
6. **启动前后都核对进程**：build 前停止全部 `bitfun-desktop.exe` 并确认已经退出，
   防止最终链接或 AppData reseed 冲突；仅在上述成功证据齐全后后台启动新 exe，再核对
   进程 `StartTime` 与路径。构建期间若 Desktop 被其他入口重新拉起，先停掉它再等待链接。
7. **失败先诊断，不盲目重跑**：先检查日志尾部、竞争 Cargo 命令、exe 时间戳和系统
   可用内存。无 `error:` / 无 `Finished` 且 exe 时间戳未变，说明没有产出新应用；不要
   宣称编译成功，也不要继续运行旧二进制。测试仍只在用户明确要求时运行。
8. **2026-08-26 极速反馈复盘**：一次主流程修复在最终 build 前运行了两个
   `cargo check`，之后又运行 Web UI type-check；它们分别额外消耗约 1 分 44 秒和
   1 分 50 秒，且没有让用户更早看到效果。后续把所有修改集中完成后只运行一次
   `cargo build -p bitfun-desktop --bin bitfun-desktop`。运行中新发现的纯 Web UI 问题
   走 Vite HMR 修复，不再触发第二次 Rust build。

## 禁止事项

- 不要主动编译：用户没有明确要求时，不执行 `cargo check`、`cargo build`、
  `pnpm run build:loopx` 等任何构建/编译动作。
- 用户只要求“编译看效果”时，不要自行追加 `cargo check`、`pnpm run type-check:web`
  或测试；最终 binary build 是唯一允许的编译动作。
- 不要在每次代码改动后主动跑测试套件（见上）。
- 不要直接修改 `%APPDATA%/bitfun/data/miniapps/builtin-bitfun-loopx/**` 当源码。
- 不要用 `git add .` 或把运行目录/生成物（`compiled.html`、`~/.bitfun/bitfun-loopx/**`）提交。
