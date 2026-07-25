//! Task DAG type definitions.
//!
//! 该模块定义了任务编排的类型系统，用于声明式 DAG 配置。
//! 调度引擎（`task_dag_exec`）在运行时消费这些类型，
//! CronService 通过 `DagConfig` 中的 `cron_trigger` 字段触发执行。
//!
//! 职责边界（参考 C5e 多方论证）：
//! - CronService: WHEN（定时触发、cron 表达式解析、任务入队）
//! - TaskDag:     WHAT + ORDER（任务编排、拓扑排序、按层并行执行、重试）

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// TaskType — 任务类型枚举
// ---------------------------------------------------------------------------

/// 任务类型。
///
/// 已知类型直接枚举，未知类型通过 `Custom(String)` 捕获，
/// 保证 JSON 反序列化不会因新增任务类型而失败。
#[derive(Debug, Clone, PartialEq)]
pub enum TaskType {
    Render,
    Tts,
    Compose,
    Publish,
    WebsiteBuild,
    SocialPost,
    Email,
    Custom(String),
}

impl Serialize for TaskType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            TaskType::Render => serializer.serialize_str("render"),
            TaskType::Tts => serializer.serialize_str("tts"),
            TaskType::Compose => serializer.serialize_str("compose"),
            TaskType::Publish => serializer.serialize_str("publish"),
            TaskType::WebsiteBuild => serializer.serialize_str("website_build"),
            TaskType::SocialPost => serializer.serialize_str("social_post"),
            TaskType::Email => serializer.serialize_str("email"),
            TaskType::Custom(s) => serializer.serialize_str(s),
        }
    }
}

impl<'de> Deserialize<'de> for TaskType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "render" => Ok(TaskType::Render),
            "tts" => Ok(TaskType::Tts),
            "compose" => Ok(TaskType::Compose),
            "publish" => Ok(TaskType::Publish),
            "website_build" => Ok(TaskType::WebsiteBuild),
            "social_post" => Ok(TaskType::SocialPost),
            "email" => Ok(TaskType::Email),
            other => Ok(TaskType::Custom(other.to_string())),
        }
    }
}

// ---------------------------------------------------------------------------
// BackoffStrategy + RetryPolicy
// ---------------------------------------------------------------------------

/// 退避策略。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackoffStrategy {
    /// 不重试（退避无效）。
    #[default]
    None,
    /// 固定延迟（秒）。
    Fixed { delay_secs: u64 },
    /// 指数退避：延迟 = min(base_secs * 2^(retry-1), max_secs)。
    Exponential { base_secs: u64, max_secs: u64 },
}

impl BackoffStrategy {
    /// 计算第 `retry_attempt` 次重试前的等待秒数（0-indexed）。
    /// retry_attempt=0 → 第一次重试前的延迟。
    pub fn delay_secs(&self, retry_attempt: u32) -> u64 {
        match self {
            BackoffStrategy::None => 0,
            BackoffStrategy::Fixed { delay_secs } => *delay_secs,
            BackoffStrategy::Exponential {
                base_secs,
                max_secs,
            } => {
                let delay = base_secs.saturating_mul(2u64.saturating_pow(retry_attempt));
                delay.min(*max_secs)
            }
        }
    }
}

/// 重试策略。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetryPolicy {
    /// 最大重试次数。0 = 不重试。
    #[serde(default)]
    pub max_retries: u32,
    /// 重试间隔策略。
    #[serde(default)]
    pub backoff: BackoffStrategy,
    /// 触发重试的错误类型标签（如 `"timeout"`、`"exit_code"`、`"io_error"`）。
    /// 空 vec = 任何错误都重试。
    #[serde(default)]
    pub retry_on: Vec<String>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 0,
            backoff: BackoffStrategy::None,
            retry_on: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// TaskNode
// ---------------------------------------------------------------------------

/// 任务节点定义——DAG 中的一个顶点。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskNode {
    /// 任务唯一标识（DAG 节点 ID）。
    pub id: String,
    /// 任务类型。
    pub task_type: TaskType,
    /// 任务参数（JSON 可配置，具体 schema 由 task_type 决定）。
    #[serde(default)]
    pub config: Value,
    /// 超时秒数。0 = 不限。
    #[serde(default)]
    pub timeout_secs: u64,
    /// 重试策略。
    #[serde(default)]
    pub retry: RetryPolicy,
    /// 依赖的任务 ID 列表。
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// 是否启用。false = 调度时跳过此节点及其下游（除非下游有其他入边）。
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

// ---------------------------------------------------------------------------
// TaskStatus + TaskResult
// ---------------------------------------------------------------------------

/// 任务执行状态。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Success,
    Failed,
    Skipped,
    Timeout,
}

/// 任务执行结果——DAG 一次执行中单个节点的产出。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// 对应的任务节点 ID。
    pub task_id: String,
    /// 执行状态。
    pub status: TaskStatus,
    /// 输出上下文（供下游任务 `input_from` 注入）。
    #[serde(default)]
    pub output: Value,
    /// 错误信息。仅在 status 为 Failed/Timeout 时有值。
    #[serde(default)]
    pub error: Option<String>,
    /// 实际耗时（秒）。
    #[serde(default)]
    pub duration_secs: f64,
    /// 实际重试次数。
    #[serde(default)]
    pub retries_used: u32,
    /// 开始执行时间。
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    /// 完成时间。
    #[serde(default)]
    pub completed_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// DagConfig — 顶层 DAG 配置
// ---------------------------------------------------------------------------

/// DAG 配置——从 JSON/YAML 文件加载的完整任务编排定义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagConfig {
    /// DAG 名称。
    pub name: String,
    /// DAG 描述。
    #[serde(default)]
    pub description: String,
    /// 任务节点列表。
    pub nodes: Vec<TaskNode>,
    /// Cron 触发表达式（如 `"30 15 * * 1-5"`）。
    /// None 表示仅手动触发。
    #[serde(default)]
    pub cron_trigger: Option<String>,
    /// 时区（如 `"Asia/Shanghai"`）。None = 系统本地时区。
    #[serde(default)]
    pub timezone: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_type_serde_known() {
        let variants = vec![
            (TaskType::Render, r#""render""#),
            (TaskType::Tts, r#""tts""#),
            (TaskType::Compose, r#""compose""#),
            (TaskType::Publish, r#""publish""#),
            (TaskType::WebsiteBuild, r#""website_build""#),
            (TaskType::SocialPost, r#""social_post""#),
            (TaskType::Email, r#""email""#),
        ];
        for (variant, expected_json) in variants {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, expected_json);
            let roundtrip: TaskType = serde_json::from_str(expected_json).unwrap();
            assert_eq!(roundtrip, variant);
        }
    }

    #[test]
    fn test_task_type_custom() {
        let custom = TaskType::Custom("my_custom_task".into());
        let json = serde_json::to_string(&custom).unwrap();
        assert_eq!(json, r#""my_custom_task""#);

        let deserialized: TaskType = serde_json::from_str(r#""my_custom_task""#).unwrap();
        assert_eq!(deserialized, TaskType::Custom("my_custom_task".into()));

        let deserialized: TaskType = serde_json::from_str(r#""unknown_type""#).unwrap();
        assert_eq!(deserialized, TaskType::Custom("unknown_type".into()));
    }

    #[test]
    fn test_backoff_strategy_serde() {
        let none = BackoffStrategy::None;
        assert_eq!(serde_json::to_string(&none).unwrap(), r#""none""#);

        let fixed = BackoffStrategy::Fixed { delay_secs: 30 };
        let fixed_json = serde_json::to_string(&fixed).unwrap();
        let fixed_rt: BackoffStrategy = serde_json::from_str(&fixed_json).unwrap();
        assert_eq!(fixed_rt, BackoffStrategy::Fixed { delay_secs: 30 });

        let exp = BackoffStrategy::Exponential {
            base_secs: 5,
            max_secs: 300,
        };
        let exp_json = serde_json::to_string(&exp).unwrap();
        let exp_rt: BackoffStrategy = serde_json::from_str(&exp_json).unwrap();
        assert_eq!(
            exp_rt,
            BackoffStrategy::Exponential {
                base_secs: 5,
                max_secs: 300
            }
        );
    }

    #[test]
    fn test_retry_policy_defaults() {
        let json = r#"{}"#;
        let policy: RetryPolicy = serde_json::from_str(json).unwrap();
        assert_eq!(policy.max_retries, 0);
        assert_eq!(policy.backoff, BackoffStrategy::None);
        assert!(policy.retry_on.is_empty());
    }

    #[test]
    fn test_task_node_defaults() {
        let json = r#"{"id":"n1","task_type":"render"}"#;
        let node: TaskNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.id, "n1");
        assert_eq!(node.task_type, TaskType::Render);
        assert_eq!(node.config, Value::Null);
        assert_eq!(node.timeout_secs, 0);
        assert_eq!(node.retry.max_retries, 0);
        assert!(node.depends_on.is_empty());
        assert!(node.enabled);
    }

    #[test]
    fn test_task_status_serde() {
        assert_eq!(
            serde_json::to_string(&TaskStatus::Success).unwrap(),
            r#""success""#
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Failed).unwrap(),
            r#""failed""#
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Skipped).unwrap(),
            r#""skipped""#
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Timeout).unwrap(),
            r#""timeout""#
        );

        let s: TaskStatus = serde_json::from_str(r#""success""#).unwrap();
        assert_eq!(s, TaskStatus::Success);
        let f: TaskStatus = serde_json::from_str(r#""failed""#).unwrap();
        assert_eq!(f, TaskStatus::Failed);
    }

    /// 核心验证：DagConfig 可从 JSON 反序列化。
    #[test]
    fn test_dag_config_from_json() {
        let json = r#"{
            "name": "daily_video_pipeline",
            "description": "每日视频生产管线",
            "cron_trigger": "30 15 * * 1-5",
            "timezone": "Asia/Shanghai",
            "nodes": [
                {
                    "id": "export",
                    "task_type": "render",
                    "config": {"instrument": "ag2506", "freq": "5min"},
                    "timeout_secs": 120,
                    "enabled": true
                },
                {
                    "id": "tts",
                    "task_type": "tts",
                    "config": {"voice": "zh-CN-XiaoxiaoNeural"},
                    "depends_on": ["export"],
                    "retry": {
                        "max_retries": 2,
                        "backoff": {"fixed": {"delay_secs": 10}},
                        "retry_on": ["timeout"]
                    }
                },
                {
                    "id": "compose",
                    "task_type": "compose",
                    "depends_on": ["tts"],
                    "timeout_secs": 300
                },
                {
                    "id": "publish_bilibili",
                    "task_type": "publish",
                    "config": {"platform": "bilibili"},
                    "depends_on": ["compose"],
                    "enabled": false
                }
            ]
        }"#;

        let dag: DagConfig = serde_json::from_str(json).unwrap();

        assert_eq!(dag.name, "daily_video_pipeline");
        assert_eq!(dag.description, "每日视频生产管线");
        assert_eq!(dag.cron_trigger.as_deref(), Some("30 15 * * 1-5"));
        assert_eq!(dag.timezone.as_deref(), Some("Asia/Shanghai"));
        assert_eq!(dag.nodes.len(), 4);

        // export
        let export = &dag.nodes[0];
        assert_eq!(export.id, "export");
        assert_eq!(export.task_type, TaskType::Render);
        assert_eq!(export.config["instrument"], "ag2506");
        assert_eq!(export.timeout_secs, 120);
        assert!(export.depends_on.is_empty());
        assert!(export.enabled);

        // tts — has retry policy
        let tts = &dag.nodes[1];
        assert_eq!(tts.id, "tts");
        assert_eq!(tts.task_type, TaskType::Tts);
        assert_eq!(tts.depends_on, vec!["export"]);
        assert_eq!(tts.retry.max_retries, 2);
        assert_eq!(tts.retry.backoff, BackoffStrategy::Fixed { delay_secs: 10 });
        assert_eq!(tts.retry.retry_on, vec!["timeout"]);

        // compose
        let compose = &dag.nodes[2];
        assert_eq!(compose.id, "compose");
        assert_eq!(compose.task_type, TaskType::Compose);
        assert_eq!(compose.depends_on, vec!["tts"]);
        assert_eq!(compose.timeout_secs, 300);

        // publish — disabled
        let publish = &dag.nodes[3];
        assert_eq!(publish.id, "publish_bilibili");
        assert_eq!(publish.task_type, TaskType::Publish);
        assert_eq!(publish.depends_on, vec!["compose"]);
        assert!(!publish.enabled);
    }

    /// Tasks can use Custom type for future expansion without breaking deserialization.
    #[test]
    fn test_dag_config_with_custom_task_type() {
        let json = r#"{
            "name": "future_pipeline",
            "description": "",
            "nodes": [
                {
                    "id": "n1",
                    "task_type": "future_task_type",
                    "config": {"key": "value"}
                }
            ]
        }"#;

        let dag: DagConfig = serde_json::from_str(json).unwrap();
        assert_eq!(dag.nodes.len(), 1);
        assert_eq!(
            dag.nodes[0].task_type,
            TaskType::Custom("future_task_type".into())
        );
    }

    #[test]
    fn test_dag_config_roundtrip() {
        let dag = DagConfig {
            name: "test".into(),
            description: "roundtrip test".into(),
            nodes: vec![TaskNode {
                id: "n1".into(),
                task_type: TaskType::Render,
                config: serde_json::json!({"key": "value"}),
                timeout_secs: 60,
                retry: RetryPolicy {
                    max_retries: 1,
                    backoff: BackoffStrategy::Exponential {
                        base_secs: 5,
                        max_secs: 60,
                    },
                    retry_on: vec!["timeout".into()],
                },
                depends_on: vec![],
                enabled: true,
            }],
            cron_trigger: Some("0 8 * * 1-5".into()),
            timezone: Some("Asia/Shanghai".into()),
        };

        let json = serde_json::to_string(&dag).unwrap();
        let roundtrip: DagConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(roundtrip.name, "test");
        assert_eq!(roundtrip.nodes.len(), 1);
        assert_eq!(roundtrip.nodes[0].id, "n1");
        assert_eq!(roundtrip.nodes[0].retry.max_retries, 1);
        assert_eq!(roundtrip.cron_trigger.as_deref(), Some("0 8 * * 1-5"));
    }
}
