# codgrep

本项目是一个本地 regex 预筛索引系统骨架，目标是把全库正则搜索拆成两段：

1. 先用字符级倒排索引召回候选文件。
2. 再只对候选文件执行真实 regex 验证。

当前实现先把最小链路跑通：

- 文件级索引
- `trigram` 和 `sparse-ngram` 两种 tokenizer 接口
- `docs.bin` / `doc_terms.bin` / `lookup.bin` / `postings.bin` 四文件落盘
- 保守型 regex 查询规划器
- 候选召回 + `regex` 精确验证
- 增量建索引的第一版：复用未变文件的文档级 token 集，只重读和重分词变更文件

## 模块布局

- `src/cli.rs`
  命令行入口；当前只保留服务进程、daemon 管理和 build/bench 运维子命令。
- `src/files.rs`
  仓库扫描、文本过滤、基础文件元数据采集。
- `src/tokenizer/`
  token 抽取策略，当前含 trigram 和 sparse n-gram。
- `src/planner.rs`
  regex 查询规划，先做保守提取，不覆盖完整语义。
- `src/index/`
  索引构建、二进制格式和查询时倒排读取。
- `src/search.rs`
  搜索结果类型定义。

## 二进制格式

- `docs.bin`
  保存 tokenizer 模式和文档元数据。
- `doc_terms.bin`
  保存每个文档去重后的 token hash 集，供增量建索引复用。
- `lookup.bin`
  保存定长 lookup entry，支持 `mmap + binary search`。
- `postings.bin`
  顺序存 posting list，使用 delta + varint 编码。

## 快速开始

```bash
cargo run -- serve
```

推荐把 `codgrep` 当成一个独立常驻进程来使用，形态更接近本地搜索 server，而不是 ripgrep 风格的一次性搜索 CLI。

如果你使用 `ManagedDaemonClient` 或 `codgrep::sdk::{ManagedClient, tokio::ManagedClient}`：

- daemon 默认按 `index_path/daemon-state.json` 发现和复用。
- 多个调用方会通过同一路径下的 `daemon-state.lock` 协调启动，避免重复拉起。
- 调用方通常不需要自己再包一层自定义 daemon 生命周期管理器。

常用运维/调试入口：

```bash
# 前台拉起 server
cargo run -- serve --bind 127.0.0.1:4597

# 注册或打开一个 repo
cargo run -- daemon open --repo /path/to/repo

# 对 repo 建索引
cargo run -- daemon build --repo-id /path/to/repo

# 通过 daemon 发搜索请求
cargo run -- daemon search --repo-id /path/to/repo 'foo.*bar'

# 通过 daemon 做路径 glob / scope 解析
cargo run -- daemon glob --repo-id /path/to/repo -g '*.rs' src

# 查询 repo 状态
cargo run -- daemon status --repo-id /path/to/repo

# 关闭 repo，或关闭整个 server
cargo run -- daemon close --repo-id /path/to/repo
cargo run -- daemon shutdown
```

如果只是离线准备索引或做 bench，也保留这两个入口：

```bash
# 通过 managed daemon 自动拉起/连接 server，并确保 repo 已可搜索
cargo run -- build --repo /path/to/repo

# benchmark 仍是本地运维工具入口
cargo run -- bench --suite-dir bench_data
```

当前 CLI 语义：

- 默认入口不再提供 ripgrep 风格的一次性搜索命令。
- 搜索相关命令通过 `daemon search` 暴露，面向服务进程调试和手工排障。
- 路径枚举/glob 相关能力通过 `daemon glob` 暴露。
- `serve` 用于拉起 TCP server，`daemon *` 用于向该 server 发协议请求。
- `build` 现在也会通过 managed daemon 走进程协议，不再直接在 CLI 进程里调用建索引库函数。

## External Interface

当前稳定的对外接口应视为 daemon 进程协议，以及建立在它之上的 Rust SDK：

- daemon protocol request/response/notification schema
- `DaemonClient` / `ManagedDaemonClient`
- `codgrep::sdk` Rust SDK facade
- `cg serve` 与 `cg daemon *` 这些围绕 daemon 的工具入口

其中：

- `daemon::protocol` 是跨语言/跨进程集成时的规范基线。
- `DaemonClient` / `ManagedDaemonClient` 与 `codgrep::sdk` 都是这层 contract 的客户端封装。
- Rust 调用方应优先依赖 `codgrep::sdk`，而不是直接在产品代码里拼 protocol 请求。
- crate 根上仍然存在的历史库导出，仅用于仓库内部与迁移期代码复用，不应当作产品接入面。

入口选择（推荐）：

- 外部集成 / IDE / agent / 脚本 / 后续多语言 SDK：以 daemon 协议为准。
- Rust 调用方：优先使用 `codgrep::sdk`，而不是直接拼 `Request/Response`。
- 本仓库内部实现：可以继续复用 workspace / index / search 这些库层模块。

Rust SDK 当前会重导出一批常用 protocol DTO，例如 `OpenRepoParams`、`EnsureRepoParams`、`RepoStatus`、`TaskStatus`、`RefreshPolicyConfig`。这些类型在 Rust 集成里建议从 `codgrep::sdk` 路径引用，而不是从 `codgrep::daemon::protocol` 直接引用。

路径边界约定：

- `codgrep` 只把自己的产物目录 `.codgrep-index`、`.codgrep-bench`，以及调用方显式配置的 `index_path` 视为内部路径。
- `codgrep` 不会内建保留某个宿主产品目录名；像 `.bitfun`、`.idea`、`.vscode` 这类目录是否排除，应由接入方或 ignore 规则决定。

Rust SDK 最小示例：

```rust
use codgrep::sdk::{
    count_only_query, EnsureRepoParams, GlobRequest, ManagedClient, PathScope,
    RefreshPolicyConfig, RepoConfig, SearchRequest,
};

let client = ManagedClient::new();
let repo = client.ensure_repo(EnsureRepoParams {
    repo_path: "/path/to/repo".into(),
    index_path: None,
    config: RepoConfig::default(),
    refresh: RefreshPolicyConfig::default(),
})?;

let search = repo.search(SearchRequest::new(count_only_query("LLVMContext")))?;
let glob = repo.glob(
    GlobRequest::new().with_scope(PathScope {
        globs: vec!["*.rs".into()],
        ..PathScope::default()
    }),
)?;
```

如果你的产品流程是“先打开 repo，等用户手动点索引”，可以走 `open_repo + index_build + wait_task`：

```rust
use std::time::Duration;

use codgrep::sdk::{
    count_only_query, ManagedClient, OpenRepoParams, RefreshPolicyConfig, RepoConfig,
    SearchRequest,
};

let client = ManagedClient::new();
let repo = client.open_repo(OpenRepoParams {
    repo_path: "/path/to/repo".into(),
    index_path: None,
    config: RepoConfig::default(),
    refresh: RefreshPolicyConfig::default(),
})?;

let task = repo.index_build()?;
repo.wait_task(task.task_id, Duration::from_secs(30))?;

let search = repo.search(SearchRequest::new(count_only_query("LLVMContext")))?;
```

如果调用方需要消费 daemon 的 `$/progress`、`workspace/statusChanged`、`task/finished`，可以订阅 repo 事件：

```rust
use std::time::Duration;

use codgrep::sdk::{
    ManagedClient, OpenRepoParams, RefreshPolicyConfig, RepoEvent, RepoTaskFinished,
    RepoWorkspaceStatusChanged, RepoConfig,
};

let client = ManagedClient::new();
let repo = client.open_repo(OpenRepoParams {
    repo_path: "/path/to/repo".into(),
    index_path: None,
    config: RepoConfig::default(),
    refresh: RefreshPolicyConfig::default(),
})?;

let mut events = repo.subscribe_events()?;
let task = repo.index_build()?;
let task_id = task.task_id.clone();

while let Some(event) = events.recv_timeout(Duration::from_secs(1))? {
    match event {
        RepoEvent::Progress(progress) if progress.task_id == task_id => {
            eprintln!("build progress: {} / {:?}", progress.processed, progress.total);
        }
        RepoEvent::WorkspaceStatusChanged(RepoWorkspaceStatusChanged { status, .. }) => {
            eprintln!("repo phase: {:?}", status.phase);
        }
        RepoEvent::TaskFinished(RepoTaskFinished { task, .. }) if task.task_id == task_id => {
            break;
        }
        _ => {}
    }
}
```

如果调用方本身已经是 `tokio` 应用，可以打开 `tokio-sdk` feature：

```bash
cargo add codgrep --features tokio-sdk
```

最小异步示例：

```rust
use codgrep::sdk::{
    count_only_query, EnsureRepoParams, RefreshPolicyConfig, RepoConfig, SearchRequest,
};

let client = codgrep::sdk::tokio::ManagedClient::new();
let repo = client
    .ensure_repo(EnsureRepoParams {
        repo_path: "/path/to/repo".into(),
        index_path: None,
        config: RepoConfig::default(),
        refresh: RefreshPolicyConfig::default(),
    })
    .await?;

let search = repo
    .search(SearchRequest::new(count_only_query("LLVMContext")))
    .await?;
```

异步 SDK 也提供同样的 repo 事件订阅接口：

```rust
let mut events = repo.subscribe_events().await?;
if let Some(event) = events.recv_timeout(std::time::Duration::from_secs(1)).await? {
    match event {
        codgrep::sdk::RepoEvent::Progress(progress) => {
            eprintln!("progress: {}", progress.message);
        }
        codgrep::sdk::RepoEvent::WorkspaceStatusChanged(_) => {}
        codgrep::sdk::RepoEvent::TaskFinished(done) => {
            eprintln!("task finished: {}", done.task.task_id);
        }
    }
}
```

核心职责边界（固定定义）：

- `daemon::protocol`：唯一稳定外部 contract。
- `DaemonClient` / `ManagedDaemonClient`：daemon protocol 的低层 transport/client 封装。
- `codgrep::sdk`：Rust 调用方的首选 facade，建立在 daemon protocol 之上。
- 其他 crate library API：内部实现、测试、benchmark、迁移期兼容，不作为稳定外部接口。

## Internal Library Notes

crate 根上仍然保留若干历史库导出，主要用于：

- daemon 内部实现
- benchmark / fixture / 测试代码
- 迁移期的仓库内辅助代码

这些库接口不是推荐的外部接入方式，后续会继续收缩；新的外部能力优先补到 daemon protocol / SDK，而不是继续扩展库层 facade。

## Daemon Overlay

daemon 运行时会在内存里维护一层 dirty overlay：

- `base snapshot` 仍然是磁盘上的主索引。
- dirty 路径集合、dirty 文档和 dirty `SearchDocumentIndex` 只存在于 daemon 进程内存。
- daemon 重启或 repo 被关闭后，内存 overlay 会丢失；下次 `open_repo` 时会根据“当前 base snapshot + 当前 worktree”重新计算并重建。

这意味着：

- 正确性依旧由当前 worktree 保证。
- 冷启动 / 重启后会有一段 overlay 恢复时间。
- `WorkspaceStrict` 在 repo 未 ready 前可能短暂不可用；`WorkspaceEventual` 可以先退到 `rg` / scan fallback。

## Benchmark

当前项目已集成一版兼容 ripgrep `benchsuite` 命名方式的 benchmark runner。

语料目录默认按下面布局准备：

- `bench_data/linux/`
- `bench_data/subtitles/en.sample.txt`
- `bench_data/subtitles/ru.txt`

运行示例：

```bash
cargo run -- bench --suite-dir /Users/user/workspace/codgrep/bench_data --filter linux_literal --rebuild
```

如果希望顺带对比 `rg`：

```bash
cargo run -- bench --suite-dir /Users/user/workspace/codgrep/bench_data --filter subtitles_en_literal --compare-rg --rebuild
```

如果希望额外测库层 facade 和 dirty worktree 路径：

```bash
cargo run -- bench \
  --suite-dir /Users/user/workspace/codgrep/bench_data \
  --filter linux_literal \
  --compare-worktree \
  --compare-workspace \
  --rebuild
```

如果希望 dirty worktree 更贴近“只改了几份文件”的 agent 场景，可以给 worktree fixture 加一个文件数上限。这个模式会从完整语料里抽一个保留真实目录结构的小型子仓库，并保证包含当前 benchmark 需要命中的文件：

```bash
cargo run -- bench \
  --suite-dir /Users/user/workspace/codgrep/bench_data \
  --filter linux_literal \
  --compare-worktree \
  --worktree-sample-files 32 \
  --rebuild
```

如果希望比较 cold query，可以切到 `cold` cache mode。这个模式会在每个 sample 前主动驱逐本次 benchmark 会访问到的 index / corpus 文件页缓存；如果你还想连系统级 page cache 一起清掉，可以额外提供一个 hook：

```bash
cargo run -- bench \
  --suite-dir /Users/user/workspace/codgrep/bench_data \
  --filter linux_literal \
  --compare-rg \
  --cache-mode cold \
  --bench-iter 3 \
  --cold-hook 'sync && printf 3 | tee /proc/sys/vm/drop_caches >/dev/null'
```

说明：

- `codgrep` 的 benchmark 会单独统计索引构建时间和查询时间。
- `--compare-worktree` 会额外跑 dirty worktree 查询路径；当前实现直接走“base snapshot + query-time dirty repair”，不再依赖持久化 overlay cache。历史 runner 名称里的 `dirty_first_query` / `dirty_cached_query` 仅为兼容已有 benchmark 输出。
- `--worktree-sample-files <N>` 只影响 `--compare-worktree` 的 fixture 构造：不再复制完整目录，而是抽一个最多 `N` 个文本文件的小型真实子仓库。这个模式更适合测“少量 dirty 文件”的真实查询成本；不传时仍然保留完整仓库 worst-case。
- `--compare-workspace` 会额外跑 clean workspace facade 路径；每个 sample 都会重新走一次 `snapshot().search()`，用于把工作区读视图构造成本也计入结果，而不是只测固定 snapshot 上的纯查询。
- `rg` 对比项只统计直接搜索耗时，不包含任何建索引步骤。
- `--raw-output <path>` 会把每个 sample 以 CSV 形式落盘；除了 `runner` 原始 id 之外，还会额外写出 `runner_family` 和 `runner_mode`，方便脚本稳定地区分 `daemon_steady_state` / `workspace_snapshot` / `dirty_first_query` / `dirty_cached_query` 等路径。当前这两条 dirty runner 都走 query-time repair，只是为了兼容旧 benchmark 标签而保留了名称。
- bench 默认是 `warm` cache mode；`warmup_iter=1`、`bench_iter=3` 的默认值测出来的不是 cold query。
- bench 默认是 `trace` query mode：如果 ad-hoc benchmark 传了多个 `--pattern`，会把它们当成一条 query trace 顺序执行，而不是把同一个 query 热重复很多次；需要旧的 steady-state 语义时可显式传 `--query-mode same`。
- `cold` cache mode 默认使用 `posix_fadvise(..., DONTNEED)` 驱逐本次 benchmark 文件集的 page cache；额外的 `--cold-hook` 适合在 Linux 上配合 `drop_caches` 做更严格的系统级 cold run。
- 当前 Linux 语料只要求仓库已 clone，不要求本机必须能成功构建内核。

## 当前边界

- 只索引 UTF-8 文本文件。
- 查询规划器优先保证正确性，复杂表达式会降级到弱查询或全量候选。
- base snapshot 现在已经按 `snapshot_key` 缓存：Git 仓库使用 `HEAD commit + config fingerprint`，非 Git 目录使用稳定 fallback key。
- dirty Git worktree 在 base snapshot 缺失时会按 `HEAD` materialize 一份只读 base，不会把脏工作树直接写进 commit base。
- workspace 查询默认使用“base snapshot + 当前 dirty files 扫描修补”的两层视图，不再依赖持久化 overlay cache。
- dirty files 当前仍按 `size + mtime` 探测，不是 content hash。
- 当前没有引入阈值 sidecar；dirty files 始终走 query-time repair。
- `lookup.bin` 已做 `mmap`，`postings.bin` 仍是按 offset 解码读取。
- CLI 和库层都不会在后台偷偷重建 base snapshot；如果需要新的 commit base，需要显式调用 build/rebuild 接口。

## 内部库层状态

`WorkspaceIndex::ensure_base_snapshot()` / `WorkspaceIndex::status()` 暴露的 `BaseSnapshotInfo` 现在包含：

- `snapshot_key`
- `snapshot_kind`
- `head_commit`
- `config_fingerprint`

如果后续需要把 freshness 结果作为稳定外部 contract 暴露，优先补到 daemon protocol / SDK，而不是继续扩展库层 facade。

## 下一步建议

- 给查询规划器补充更完整的 AST/HIR 解析。
- 为 posting list 增加压缩和缓存。
- 把 overlay 从“按 dirty state 命中的完整小索引 + 受影响 token 级重写”继续演进到“更细粒度的字节级 patch 和跨 base 复用”。
- 加入 doc_freq 统计与更激进的 top-k token 选择器。
