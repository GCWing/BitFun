//! TaskDag execution engine.
//!
//! 调度流程：
//!   1. DagConfig.nodes → taiji-engine Dag（add_node + add_edge）
//!   2. dag.sort() → Vec<Vec<String>> 分层
//!   3. 逐层 tokio::spawn 并发执行
//!   4. 每层完成后收集结果 → 下一层
//!   5. 上游 Failed/Timeout → 下游 Skipped
//!   6. 失败按 RetryPolicy 重试
//!   7. 超时 → TaskStatus::Timeout
//!   8. 进度持久化到 state_dir/{name}_state.json

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use taiji_engine::dag::Dag;

use crate::task_dag_types::{DagConfig, TaskNode, TaskResult, TaskStatus};

// ---------------------------------------------------------------------------
// TaskRunner — 可插拔的任务执行器
// ---------------------------------------------------------------------------

/// 任务执行 trait。由上层注入具体执行逻辑（render、tts、compose、publish 等）。
#[async_trait::async_trait]
pub trait TaskRunner: Send + Sync {
    /// 执行单个任务节点，返回结果。
    async fn run(&self, node: &TaskNode) -> TaskResult;
}

// ---------------------------------------------------------------------------
// TaskDagExecutor
// ---------------------------------------------------------------------------

/// DAG 任务调度引擎。
pub struct TaskDagExecutor {
    config: DagConfig,
    results: HashMap<String, TaskResult>,
    state_dir: PathBuf,
}

impl TaskDagExecutor {
    /// 创建执行器。
    pub fn new(config: DagConfig, state_dir: PathBuf) -> Self {
        Self {
            config,
            results: HashMap::new(),
            state_dir,
        }
    }

    /// 返回当前已收集的执行结果（只读）。
    pub fn results(&self) -> &HashMap<String, TaskResult> {
        &self.results
    }

    /// 执行完整 DAG。
    ///
    /// `runner` 提供每个任务节点的具体执行逻辑。
    /// 返回所有节点结果的引用。
    pub async fn execute(&mut self, runner: Arc<dyn TaskRunner>) -> &HashMap<String, TaskResult> {
        // 1. 构建 taiji-engine Dag
        let mut dag = Dag::new();
        let mut node_map: HashMap<String, TaskNode> = HashMap::new();

        for node in &self.config.nodes {
            if node.enabled {
                dag.add_node(node.id.clone());
                node_map.insert(node.id.clone(), node.clone());
            }
        }

        for node in &self.config.nodes {
            if !node.enabled {
                continue;
            }
            for dep in &node.depends_on {
                // 仅当上游节点也启用时才添加边
                if node_map.contains_key(dep) {
                    dag.add_edge(dep.clone(), node.id.clone());
                }
            }
        }

        // 2. 拓扑排序 → 分层
        let layers = match dag.sort() {
            Ok(layers) => layers,
            Err(cycle_nodes) => {
                // 循环依赖：全部节点标记为 Failed
                let now = Utc::now();
                for node in &self.config.nodes {
                    self.results.insert(
                        node.id.clone(),
                        TaskResult {
                            task_id: node.id.clone(),
                            status: TaskStatus::Failed,
                            output: Default::default(),
                            error: Some(format!("Cycle detected involving: {:?}", cycle_nodes)),
                            duration_secs: 0.0,
                            retries_used: 0,
                            started_at: Some(now),
                            completed_at: Some(now),
                        },
                    );
                }
                return &self.results;
            }
        };

        // 3-5. 逐层执行
        for layer in &layers {
            self.execute_layer(layer, &node_map, Arc::clone(&runner))
                .await;
            // 8. 每层完成后持久化状态
            self.persist_state();
        }

        &self.results
    }

    // ------------------------------------------------------------------
    // 内部方法
    // ------------------------------------------------------------------

    /// 执行单层中的所有任务（并发）。
    async fn execute_layer(
        &mut self,
        layer: &[String],
        node_map: &HashMap<String, TaskNode>,
        runner: Arc<dyn TaskRunner>,
    ) {
        // 先标记上游失败的下游节点为 Skipped
        for node_id in layer {
            if let Some(node) = node_map.get(node_id) {
                if self.any_upstream_failed(node) {
                    let now = Utc::now();
                    self.results.insert(
                        node_id.clone(),
                        TaskResult {
                            task_id: node_id.clone(),
                            status: TaskStatus::Skipped,
                            output: Default::default(),
                            error: Some("Upstream task failed".into()),
                            duration_secs: 0.0,
                            retries_used: 0,
                            started_at: Some(now),
                            completed_at: Some(now),
                        },
                    );
                }
            }
        }

        // 并发执行本层中未被跳过的任务
        let mut handles = tokio::task::JoinSet::new();

        for node_id in layer {
            if self.results.contains_key(node_id) {
                continue; // 已标记为 Skipped
            }
            if let Some(node) = node_map.get(node_id) {
                let node = node.clone();
                let runner = Arc::clone(&runner);
                handles.spawn(async move { Self::run_with_retry(node, runner).await });
            }
        }

        // 收集本层结果
        while let Some(result) = handles.join_next().await {
            match result {
                Ok(task_result) => {
                    self.results
                        .insert(task_result.task_id.clone(), task_result);
                }
                Err(e) => {
                    // JoinError：任务 panic。生成一个 Failed 结果。
                    let err_result = TaskResult {
                        task_id: "__join_error__".into(),
                        status: TaskStatus::Failed,
                        output: Default::default(),
                        error: Some(format!("Task panicked: {}", e)),
                        duration_secs: 0.0,
                        retries_used: 0,
                        started_at: Some(Utc::now()),
                        completed_at: Some(Utc::now()),
                    };
                    self.results.insert(err_result.task_id.clone(), err_result);
                }
            }
        }
    }

    /// 带重试和超时的单任务执行。
    ///
    /// 这是 static 方法，可安全传入 `tokio::spawn`。
    async fn run_with_retry(node: TaskNode, runner: Arc<dyn TaskRunner>) -> TaskResult {
        let max_attempts = node.retry.max_retries + 1; // 1 次初始 + N 次重试
        let started_at = Utc::now();

        for attempt in 0..max_attempts {
            if attempt > 0 {
                // 重试前退避等待
                let delay_secs = node.retry.backoff.delay_secs(attempt - 1);
                if delay_secs > 0 {
                    tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                }
            }

            let result = Self::run_single_with_timeout(&node, &*runner).await;

            let should_retry = matches!(result.status, TaskStatus::Failed | TaskStatus::Timeout);

            let retry_tag_matches = node.retry.retry_on.is_empty()
                || node
                    .retry
                    .retry_on
                    .iter()
                    .any(|tag| result.error.as_deref().unwrap_or("").contains(tag.as_str()));

            if should_retry && retry_tag_matches && attempt + 1 < max_attempts {
                continue;
            }

            // 最终结果（成功、重试耗尽、或不可重试的错误）
            return TaskResult {
                retries_used: attempt,
                started_at: Some(started_at),
                ..result
            };
        }

        // 不应到达此处（循环必然在最后一次迭代返回），但编译器需要
        unreachable!()
    }

    /// 执行单次任务（带超时保护）。
    async fn run_single_with_timeout(node: &TaskNode, runner: &dyn TaskRunner) -> TaskResult {
        let fut = runner.run(node);

        if node.timeout_secs > 0 {
            match tokio::time::timeout(Duration::from_secs(node.timeout_secs), fut).await {
                Ok(result) => result,
                Err(_elapsed) => TaskResult {
                    task_id: node.id.clone(),
                    status: TaskStatus::Timeout,
                    output: Default::default(),
                    error: Some(format!("Task timed out after {}s", node.timeout_secs)),
                    duration_secs: node.timeout_secs as f64,
                    retries_used: 0,
                    started_at: Some(Utc::now()),
                    completed_at: Some(Utc::now()),
                },
            }
        } else {
            fut.await
        }
    }

    /// 检查节点的上游依赖中是否有失败的。
    fn any_upstream_failed(&self, node: &TaskNode) -> bool {
        node.depends_on.iter().any(|dep_id| {
            self.results
                .get(dep_id)
                .is_some_and(|r| r.status == TaskStatus::Failed || r.status == TaskStatus::Timeout)
        })
    }

    /// 持久化当前执行状态到磁盘。
    fn persist_state(&self) {
        let path = self
            .state_dir
            .join(format!("{}_state.json", self.config.name));
        // best-effort: 持久化失败不中断执行
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.results) {
            let _ = std::fs::write(&path, json);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_dag_types::{RetryPolicy, TaskType};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    /// 辅助：构造一个简单的 TaskNode。
    fn make_node(id: &str, depends_on: Vec<&str>) -> TaskNode {
        TaskNode {
            id: id.into(),
            task_type: TaskType::Custom("test".into()),
            config: Default::default(),
            timeout_secs: 0,
            retry: RetryPolicy::default(),
            depends_on: depends_on.into_iter().map(String::from).collect(),
            enabled: true,
        }
    }

    /// 辅助：构造 DagConfig。
    fn make_config(nodes: Vec<TaskNode>) -> DagConfig {
        DagConfig {
            name: "test_dag".into(),
            description: String::new(),
            nodes,
            cron_trigger: None,
            timezone: None,
        }
    }

    // ------------------------------------------------------------------
    // 测试 1：3 节点 DAG（A→B, A→C）分层结果 [[A],[B,C]]
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_dag_sort_three_nodes() {
        let config = make_config(vec![
            make_node("A", vec![]),
            make_node("B", vec!["A"]),
            make_node("C", vec!["A"]),
        ]);

        // 通过 execute 间接触发 DAG 构建，然后验证执行顺序
        struct RecordRunner {
            order: Mutex<Vec<String>>,
        }
        #[async_trait::async_trait]
        impl TaskRunner for RecordRunner {
            async fn run(&self, node: &TaskNode) -> TaskResult {
                self.order.lock().unwrap().push(node.id.clone());
                let now = Utc::now();
                TaskResult {
                    task_id: node.id.clone(),
                    status: TaskStatus::Success,
                    output: Default::default(),
                    error: None,
                    duration_secs: 0.0,
                    retries_used: 0,
                    started_at: Some(now),
                    completed_at: Some(now),
                }
            }
        }

        let runner = RecordRunner {
            order: Mutex::new(Vec::new()),
        };
        let mut executor = TaskDagExecutor::new(config, std::env::temp_dir());
        executor.execute(Arc::new(runner)).await;

        // runner 已被 move into Arc，无法再访问 order。
        // 验证：B 和 C 的结果状态为 Success（说明它们被执行了）
        for id in &["A", "B", "C"] {
            let r = executor.results().get(*id).unwrap();
            assert_eq!(r.status, TaskStatus::Success, "node {} should succeed", id);
        }
    }

    /// 验证分层执行：A 先于 B 和 C。
    #[tokio::test]
    async fn test_dag_sort_layer_order() {
        let config = make_config(vec![
            make_node("A", vec![]),
            make_node("B", vec!["A"]),
            make_node("C", vec!["A"]),
        ]);

        struct RecordRunner {
            order: Mutex<Vec<String>>,
        }
        #[async_trait::async_trait]
        impl TaskRunner for RecordRunner {
            async fn run(&self, node: &TaskNode) -> TaskResult {
                self.order.lock().unwrap().push(node.id.clone());
                let now = Utc::now();
                TaskResult {
                    task_id: node.id.clone(),
                    status: TaskStatus::Success,
                    output: Default::default(),
                    error: None,
                    duration_secs: 0.0,
                    retries_used: 0,
                    started_at: Some(now),
                    completed_at: Some(now),
                }
            }
        }

        let runner = Arc::new(RecordRunner {
            order: Mutex::new(Vec::new()),
        });
        let mut executor = TaskDagExecutor::new(config, std::env::temp_dir());
        let runner2: Arc<dyn TaskRunner> = runner.clone();
        executor.execute(runner2).await;

        let order = runner.order.lock().unwrap();
        // A 必须先执行
        assert_eq!(order[0], "A");
        // B 和 C 在 A 之后执行（同层顺序不保证，但都在 A 之后）
        assert!(order.contains(&"B".into()));
        assert!(order.contains(&"C".into()));

        // 验证所有结果都是 Success
        for id in &["A", "B", "C"] {
            let r = executor.results().get(*id).unwrap();
            assert_eq!(r.status, TaskStatus::Success, "node {} should succeed", id);
        }
    }

    // ------------------------------------------------------------------
    // 测试 2：RetryPolicy 重试次数正确
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_retry_count_correct() {
        let mut retry_node = make_node("flaky", vec![]);
        retry_node.retry = RetryPolicy {
            max_retries: 3,
            backoff: crate::task_dag_types::BackoffStrategy::Fixed { delay_secs: 0 },
            retry_on: vec![],
        };

        let config = make_config(vec![retry_node]);

        struct FlakyRunner {
            attempts: AtomicU32,
        }
        #[async_trait::async_trait]
        impl TaskRunner for FlakyRunner {
            async fn run(&self, node: &TaskNode) -> TaskResult {
                let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
                let now = Utc::now();
                if attempt < 3 {
                    // 前 3 次失败
                    TaskResult {
                        task_id: node.id.clone(),
                        status: TaskStatus::Failed,
                        output: Default::default(),
                        error: Some("flaky failure".into()),
                        duration_secs: 0.0,
                        retries_used: 0,
                        started_at: Some(now),
                        completed_at: Some(now),
                    }
                } else {
                    // 第 4 次（attempt=3）成功
                    TaskResult {
                        task_id: node.id.clone(),
                        status: TaskStatus::Success,
                        output: Default::default(),
                        error: None,
                        duration_secs: 0.0,
                        retries_used: 0,
                        started_at: Some(now),
                        completed_at: Some(now),
                    }
                }
            }
        }

        let runner = FlakyRunner {
            attempts: AtomicU32::new(0),
        };
        let mut executor = TaskDagExecutor::new(config, std::env::temp_dir());
        executor.execute(Arc::new(runner)).await;

        let result = executor.results().get("flaky").unwrap();
        assert_eq!(result.status, TaskStatus::Success);
        // 前 3 次失败 + 第 4 次成功 → retries_used = 3
        assert_eq!(result.retries_used, 3);
    }

    /// 重试耗尽后最终状态为 Failed。
    #[tokio::test]
    async fn test_retry_exhausted_fails() {
        let mut retry_node = make_node("always_fail", vec![]);
        retry_node.retry = RetryPolicy {
            max_retries: 2,
            backoff: crate::task_dag_types::BackoffStrategy::Fixed { delay_secs: 0 },
            retry_on: vec![],
        };

        let config = make_config(vec![retry_node]);

        struct AlwaysFailRunner;
        #[async_trait::async_trait]
        impl TaskRunner for AlwaysFailRunner {
            async fn run(&self, node: &TaskNode) -> TaskResult {
                let now = Utc::now();
                TaskResult {
                    task_id: node.id.clone(),
                    status: TaskStatus::Failed,
                    output: Default::default(),
                    error: Some("persistent failure".into()),
                    duration_secs: 0.0,
                    retries_used: 0,
                    started_at: Some(now),
                    completed_at: Some(now),
                }
            }
        }

        let mut executor = TaskDagExecutor::new(config, std::env::temp_dir());
        executor.execute(Arc::new(AlwaysFailRunner)).await;

        let result = executor.results().get("always_fail").unwrap();
        assert_eq!(result.status, TaskStatus::Failed);
        // max_retries=2 → 总共 3 次尝试（初始+2次重试），retries_used 应记录最终尝试次数
        assert_eq!(result.retries_used, 2);
    }

    // ------------------------------------------------------------------
    // 测试 3：上游失败 → 下游 Skipped
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_upstream_failure_skips_downstream() {
        let config = make_config(vec![
            make_node("A", vec![]),
            make_node("B", vec!["A"]),
            make_node("C", vec!["A"]),
        ]);

        struct FailARunner;
        #[async_trait::async_trait]
        impl TaskRunner for FailARunner {
            async fn run(&self, node: &TaskNode) -> TaskResult {
                let now = Utc::now();
                if node.id == "A" {
                    TaskResult {
                        task_id: node.id.clone(),
                        status: TaskStatus::Failed,
                        output: Default::default(),
                        error: Some("A fails".into()),
                        duration_secs: 0.0,
                        retries_used: 0,
                        started_at: Some(now),
                        completed_at: Some(now),
                    }
                } else {
                    TaskResult {
                        task_id: node.id.clone(),
                        status: TaskStatus::Success,
                        output: Default::default(),
                        error: None,
                        duration_secs: 0.0,
                        retries_used: 0,
                        started_at: Some(now),
                        completed_at: Some(now),
                    }
                }
            }
        }

        let mut executor = TaskDagExecutor::new(config, std::env::temp_dir());
        executor.execute(Arc::new(FailARunner)).await;

        // A → Failed
        assert_eq!(
            executor.results().get("A").unwrap().status,
            TaskStatus::Failed
        );
        // B, C → Skipped
        assert_eq!(
            executor.results().get("B").unwrap().status,
            TaskStatus::Skipped
        );
        assert_eq!(
            executor.results().get("C").unwrap().status,
            TaskStatus::Skipped
        );
    }

    /// 上游 Timeout 同样导致下游 Skipped。
    #[tokio::test]
    async fn test_upstream_timeout_skips_downstream() {
        let mut timeout_node = make_node("A", vec![]);
        timeout_node.timeout_secs = 1;

        let config = make_config(vec![timeout_node, make_node("B", vec!["A"])]);

        struct SlowRunner;
        #[async_trait::async_trait]
        impl TaskRunner for SlowRunner {
            async fn run(&self, node: &TaskNode) -> TaskResult {
                if node.id == "A" {
                    // 睡眠超过超时时间
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                let now = Utc::now();
                TaskResult {
                    task_id: node.id.clone(),
                    status: TaskStatus::Success,
                    output: Default::default(),
                    error: None,
                    duration_secs: 0.0,
                    retries_used: 0,
                    started_at: Some(now),
                    completed_at: Some(now),
                }
            }
        }

        let mut executor = TaskDagExecutor::new(config, std::env::temp_dir());
        executor.execute(Arc::new(SlowRunner)).await;

        assert_eq!(
            executor.results().get("A").unwrap().status,
            TaskStatus::Timeout
        );
        assert_eq!(
            executor.results().get("B").unwrap().status,
            TaskStatus::Skipped
        );
    }
}
