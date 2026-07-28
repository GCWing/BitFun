# taiji-growth — Growth Engine

Phase 5 growth infrastructure: website publishing, report generation, email dispatch, TaskDag workflow orchestration. Builds on `taiji-engine::dag::Dag` for multi-step pipeline scheduling.

## Architecture Position

```
taiji-engine (Dag, StateStore)
  └── taiji-growth (TaskDag, EmailDispatcher, ReportMdGen, WebsitePublisher)
        └── taiji-blog-gen (CLI)
```

## Module Index

| Module | Description |
|--------|-------------|
| `types.rs` | `ContentAsset`, `ReportConfig`, `WebsiteConfig`, `ContentType` |
| `publisher_website.rs` | `WebsitePublisher` trait — build + deploy + status |
| `report_md_gen.rs` | Tera templates → Hugo-compatible Markdown with front matter |
| `email_dispatcher.rs` | lettre SMTP + double opt-in + unsubscribe |
| `task_dag_types.rs` | `TaskNode`, `TaskResult`, `RetryPolicy`, `DagConfig` |
| `task_dag_exec.rs` | DAG topo sort → layered `tokio::join!` concurrent execution |

## Quick Start

```rust
use taiji_growth::task_dag_types::{DagConfig, TaskNode, RetryPolicy};
use taiji_growth::task_dag_exec::TaskDagExecutor;

let config: DagConfig = serde_json::from_str(&dag_json)?;
let mut executor = TaskDagExecutor::new(config, state_dir);
let results = executor.execute().await?;
```

## License

SPDX-License-Identifier: Apache-2.0 OR MIT
