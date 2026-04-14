# Workspace Index Architecture

## Goal

`codgrep` 当前采用面向 agent 代码搜索的两层检索架构：

1. `base snapshot`
   对应某个 Git commit 或 repo fingerprint 的稳定只读索引。
2. `dirty worktree repair`
   对当前工作区里相对 base 已变更的少量文件做查询期修补。

设计目标不是持续维护一套持久化实时增量倒排，而是：

- 对当前 workspace view 不漏召回。
- 保持搜索热路径简单。
- 把复杂度留给真正有收益的场景。

## External Contract

对产品/集成方而言，当前推荐把 daemon 协议视为唯一稳定外部 contract。

- daemon request/response/notification schema 负责对外暴露能力。
- `WorkspaceIndex` / `WorkspaceSnapshot` / `SearchEngine` 这些库层 facade 主要服务于 daemon 内部实现与迁移期代码。

## Layering

### Base Snapshot

Base snapshot 继续复用当前工程已有的 generation 目录布局：

- `docs.bin`
- `doc_terms.bin`
- `lookup.bin`
- `postings.bin`

它是只读、可复用、按 `snapshot_key = HEAD commit + config fingerprint` 缓存的主索引层。
在非 Git 目录下，会退化为稳定的 repo-scoped fallback snapshot key。

### Dirty Worktree Repair

库层默认不维护持久化 overlay cache，也不把 dirty subset 落盘成另一份倒排。

查询时直接做两件事：

1. 用 base snapshot 对 clean files 做候选召回。
2. 对 dirty files 做 query-time repair。

语义：

- `Modified(path)` 等价于 `shadow base[path] + scan current[path]`
- `Deleted(path)` 等价于 `shadow base[path]`
- `Added(path)` 等价于 `scan current[path]`
- `Renamed(old, new)` 等价于 `delete old + add new`

## Effective View

逻辑主键使用规范化路径，而不是 `doc_id == docs slot`。

对任一路径 `path`：

1. 如果 `path` 在 dirty 集里，使用当前工作区内容。
2. 如果 `path` 在 dirty 集里且文件已删除，路径不存在。
3. 否则回退到 base snapshot。

查询时的候选集合：

```text
EffectiveCandidates =
    (BaseCandidates - ShadowedDirtyPaths)
    UNION DirtyExistingPaths
```

最终 verify 永远读取当前有效路径上的真实内容。

## Runtime Modes

### Library Mode

`WorkspaceIndex` / `WorkspaceSnapshot` 默认采用：

`Base Snapshot + Query-time Dirty Repair`

也就是说：

- base snapshot 落盘并复用。
- dirty path 集由 query 时或 snapshot 创建时解析。
- dirty 文档内容在 query 时从当前文件系统读取。

### Daemon Mode

daemon 在进程内会额外维护一层 optional in-memory dirty overlay：

- dirty path 集
- dirty 文档缓存
- dirty `SearchDocumentIndex`

这层 overlay：

- 只存在于 daemon 进程内存。
- 不会持久化到磁盘。
- daemon 重启或 repo 关闭后会丢失。
- 下次 `open_repo` 时会通过重新 diff 当前 worktree 来重建。

这意味着 daemon 的运行时路径实际上是：

`Base Snapshot + Optional In-Memory Dirty Overlay`

## Runtime Modules

当前实现建议关注下面几个模块：

```text
src/index/
  builder.rs
  format.rs
  searcher.rs

src/workspace.rs
src/workspace/runtime.rs
src/daemon/repo.rs
```

职责：

- `builder.rs`
  构建和复用 commit-addressed base snapshot。
- `searcher.rs`
  提供 base snapshot 的倒排查询与 stale/worktree diff 检测。
- `workspace/runtime.rs`
  合并 base 候选与 dirty 文件集合，并解析当前工作区文档。
- `workspace.rs`
  对外暴露稳定的 workspace facade 与 reusable snapshot 视图。
- `daemon/repo.rs`
  维护 watcher、repo runtime 状态，以及 daemon 进程内的 dirty overlay。

## Public Library API

```rust
pub struct WorkspaceIndexOptions {
    pub build_config: BuildConfig,
}

pub struct WorkspaceIndex;

pub struct WorkspaceSnapshot;

impl WorkspaceIndex {
    pub fn open(options: WorkspaceIndexOptions) -> Result<Self>;
    pub fn ensure_base_snapshot(&self) -> Result<BaseSnapshotInfo>;
    pub fn status(&self) -> Result<IndexStatus>;
    pub fn probe_freshness(&self) -> Result<WorkspaceFreshness>;
    pub fn probe_freshness_if_due(&self, ttl: Duration) -> Result<WorkspaceFreshness>;
    pub fn search(&self, query: &QueryConfig) -> Result<SearchResults>;
    pub fn snapshot(&self) -> Result<WorkspaceSnapshot>;
}

impl WorkspaceSnapshot {
    pub fn base_snapshot_key(&self) -> &str;
    pub fn dirty_diff(&self) -> &IndexWorktreeDiff;
    pub fn search(&self, query: &QueryConfig) -> Result<SearchResults>;
}
```

## Query Semantics

`WorkspaceIndex::search()` 默认返回的是：

`base snapshot + current dirty files repaired`

这意味着调用方不再需要显式 refresh 某个 overlay cache；只要 base snapshot 已存在，workspace 查询就会直接以当前工作区视图为准。

`WorkspaceSnapshot::search()` 返回的是：

`snapshot creation time captured dirty path set + current file contents on those paths`

也就是说 `WorkspaceSnapshot` 固定的是 dirty path 集，而不是 dirty 文件内容本身。

## Current Limits

- dirty files 当前按 `size + mtime` 检测，不是 content hash。
- library facade 里的 dirty files 当前统一走 query-time repair。
- daemon runtime 会维护一个内存 dirty overlay，用来减少重复 query 的 query-time repair 成本。
- `WorkspaceSnapshot` 会固定 dirty path 集合，但最终 verify 仍然读取真实文件路径。
- `lookup.bin` 已做 `mmap`，`postings.bin` 仍是按 offset 解码读取。

## Migration Note

旧的持久化 overlay cache 方案已经移除：

- 不再有 `overlay_key`
- 不再有 `refresh_overlay()`
- 不再有 `prune_overlay_cache()`
- 不再保留 overlay manifest / tombstones / postings 复用路径

当前架构默认采用：

`Base Snapshot + Query-time Dirty Repair`

daemon 长驻模式下则是：

`Base Snapshot + Optional In-Memory Dirty Overlay`
