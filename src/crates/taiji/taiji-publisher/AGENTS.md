# Taiji Publisher

Multi-platform video publishing crate for the Taiji multi-agent trading system.

## Adapter Pattern Mapping

`taiji-publisher` is a **domain-specific application** of the BitFun adapter pattern
defined in [`src/crates/adapters/AGENTS.md`](../../adapters/AGENTS.md).  Each
platform backend is an independently implemented adapter behind a shared trait
contract; the scheduler orchestrates them without owning platform-specific logic.

| Taiji Publisher Concept | BitFun Adapter Equivalent | Notes |
|---|---|---|
| `PlatformPublisher` trait | `TransportAdapter` / `PluginHostAdapter` | Stable interface contract: `platform_name()`, `check_auth()`, `upload()`, `status()`, `update()`, `unpublish()` |
| `BiliupPublisher` | `ai-adapters` provider impl (e.g. Anthropic adapter) | CLI-wrapper adapter: translates `VideoAsset` → biliup CLI args |
| `TwitterPublisher` | `ai-adapters` HTTP provider impl (e.g. OpenAI adapter) | HTTP API adapter: translates `VideoAsset` → Twitter API v2 JSON |
| `SocialPublisher` | `webdriver` adapter | Browser-automation CLI wrapper: Playwright-driven upload for Douyin / Xiaohongshu |
| `PublishScheduler` | `assembly` (adapter registration + orchestration) | Trait-object dispatch over `Vec<Arc<dyn PlatformPublisher>>`; concurrent execution via `JoinSet` + `Semaphore` |
| `process_util` module | `bitfun-services-core::process_manager` | Mirrors `create_command` / timeout patterns; marked for migration when taiji-publisher depends on services-core |

### Why This Is the Adapter Pattern

1. **Stable trait = contract.**  `PlatformPublisher` defines the shape every platform must
   fulfill.  Consumer code (`PublishScheduler`) only sees `dyn PlatformPublisher` —
   it never branches on platform identity.

2. **Each platform is independently implemented.**  `BiliupPublisher` knows nothing about
   Twitter OAuth; `TwitterPublisher` knows nothing about biliup CLI.  Adding a new
   platform requires only a new `impl PlatformPublisher` struct — no changes to the
   scheduler or shared types.

3. **Protocol translation, not product policy.**  Publishers translate `VideoAsset`
   into platform-specific commands or HTTP payloads.  They do **not** decide which
   videos to publish, when to schedule, or how to compose cross-platform summaries.

4. **Default trait methods = optional capability opt-in.**  `update()` and `unpublish()`
   default to `Err("not supported")`, following the same pattern as BitFun adapters
   that gate optional protocol features behind trait defaults.

### Relationship to BitFun Assembly

If integrated into the BitFun process, `PublishScheduler` would be registered in the
assembly layer (`src/crates/assembly/`) alongside other adapter registrations.
`PlatformPublisher` implementors would be instantiated via adapter factories, and the
scheduler would be wired as a product capability — exactly as `TransportAdapter`
implementors are registered in `src/apps/desktop/`.

## Modules

| File | Purpose |
|---|---|
| [`lib.rs`](src/lib.rs) | `VideoAsset` DTO, `PlatformPublisher` trait, `PublishResult`, `PublishStatus` |
| [`biliup.rs`](src/biliup.rs) | Bilibili adapter via biliup CLI |
| [`publisher_twitter.rs`](src/publisher_twitter.rs) | Twitter (X) adapter via Twitter API v2 |
| [`social_auto.rs`](src/social_auto.rs) | Douyin / Xiaohongshu adapter via social-auto-upload CLI |
| [`publish_scheduler.rs`](src/publish_scheduler.rs) | Concurrent multi-platform orchestrator with exponential backoff retry |
| [`process_util.rs`](src/process_util.rs) | CLI safety wrappers (`CREATE_NO_WINDOW`, timeout, sanitization) |
| [`publisher_wechat_mp.rs`](src/publisher_wechat_mp.rs) | WeChat Official Account adapter |

## Dependency Boundaries

- `taiji-publisher` depends on `taiji-content` (for `DateRange`).
- It does **not** depend on BitFun `adapters`, `assembly`, `services`, or `execution`
  crates — it is a standalone Taiji domain crate that **parallels** the BitFun adapter
  pattern rather than extending it.
- External CLI tools (`biliup`, `python` + `social-auto-upload`) and HTTP APIs
  (Twitter API v2) are boundary resources — only the corresponding publisher struct
  calls them.
