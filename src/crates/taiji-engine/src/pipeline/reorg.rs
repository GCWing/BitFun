use crate::error::Result;

/// 重组动作
#[derive(Debug, Clone, PartialEq)]
pub enum ReorgAction {
    Skip,    // 跳过此节点，继续执行
    Retry,   // 重试此节点
    Degrade, // 降级：跳过此节点及其下游
    Halt,    // 停止 Pipeline
}

/// 军事重组策略：根据节点连续失败次数决定行动。
pub struct ReorgPolicy {
    #[allow(dead_code)]
    max_retries: usize,
    failure_counts: std::collections::HashMap<String, usize>,
}

impl ReorgPolicy {
    pub fn new(max_retries: usize) -> Self {
        Self {
            max_retries,
            failure_counts: std::collections::HashMap::new(),
        }
    }

    /// 记录节点失败并返回建议动作
    pub fn on_node_failure(&mut self, node_id: &str) -> ReorgAction {
        let count = self.failure_counts.entry(node_id.to_string()).or_insert(0);
        *count += 1;

        match *count {
            1 => ReorgAction::Retry,
            2 => ReorgAction::Skip,
            _ => ReorgAction::Degrade,
        }
    }

    /// 节点成功时重置计数
    pub fn on_node_success(&mut self, node_id: &str) {
        self.failure_counts.remove(node_id);
    }

    /// 检查是否应停止 Pipeline（所有节点都 Degrade）
    pub fn should_halt(&self, total_nodes: usize) -> bool {
        let degraded = self.failure_counts.values().filter(|&&c| c >= 3).count();
        degraded >= total_nodes
    }
}

/// 军事重组执行器：递归展开嵌套子链的重组逻辑。
pub fn reorg(policy: &mut ReorgPolicy, node_id: &str, total_nodes: usize) -> Result<ReorgAction> {
    if policy.should_halt(total_nodes) {
        return Ok(ReorgAction::Halt);
    }

    let action = policy.on_node_failure(node_id);
    Ok(action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_failure_retry() {
        let mut policy = ReorgPolicy::new(3);
        assert_eq!(policy.on_node_failure("n1"), ReorgAction::Retry);
    }

    #[test]
    fn test_second_failure_skip() {
        let mut policy = ReorgPolicy::new(3);
        policy.on_node_failure("n1");
        assert_eq!(policy.on_node_failure("n1"), ReorgAction::Skip);
    }

    #[test]
    fn test_third_failure_degrade() {
        let mut policy = ReorgPolicy::new(3);
        policy.on_node_failure("n1");
        policy.on_node_failure("n1");
        assert_eq!(policy.on_node_failure("n1"), ReorgAction::Degrade);
    }

    #[test]
    fn test_success_resets() {
        let mut policy = ReorgPolicy::new(3);
        policy.on_node_failure("n1");
        policy.on_node_success("n1");
        assert_eq!(policy.on_node_failure("n1"), ReorgAction::Retry);
    }
}
