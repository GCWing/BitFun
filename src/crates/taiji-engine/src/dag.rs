use std::collections::HashMap;

use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;

pub type NodeId = String;

pub struct Dag {
    graph: DiGraph<NodeId, ()>,
    node_map: HashMap<NodeId, NodeIndex>,
}

impl Default for Dag {
    fn default() -> Self {
        Self::new()
    }
}

impl Dag {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_map: HashMap::new(),
        }
    }

    /// 添加节点
    pub fn add_node(&mut self, id: NodeId) {
        if !self.node_map.contains_key(&id) {
            let idx = self.graph.add_node(id.clone());
            self.node_map.insert(id, idx);
        }
    }

    /// 添加有向边 from → to。自动注册节点如果不存在。重复边为幂等操作。
    pub fn add_edge(&mut self, from: NodeId, to: NodeId) {
        self.add_node(from.clone());
        self.add_node(to.clone());
        let from_idx = self.node_map[&from];
        let to_idx = self.node_map[&to];
        if self.graph.find_edge(from_idx, to_idx).is_some() {
            return; // 重复边，跳过
        }
        self.graph.add_edge(from_idx, to_idx, ());
    }

    /// 使用 petgraph 拓扑排序。返回按层分组的执行顺序。
    /// 如果有循环依赖，返回 Err 并列出环中节点。
    pub fn sort(&self) -> Result<Vec<Vec<NodeId>>, Vec<NodeId>> {
        match toposort(&self.graph, None) {
            Ok(order) => {
                // 在拓扑序上 DP 计算每层深度
                let mut depth: HashMap<NodeIndex, usize> = HashMap::new();
                for &node in &order {
                    let current = depth.entry(node).or_insert(0);
                    let current_depth = *current;
                    for succ in self.graph.neighbors_directed(node, Direction::Outgoing) {
                        let d = depth.entry(succ).or_insert(0);
                        *d = (*d).max(current_depth + 1);
                    }
                }

                let max_depth = depth.values().copied().max().unwrap_or(0);
                let mut layers: Vec<Vec<NodeId>> = vec![Vec::new(); max_depth + 1];
                for &node in &order {
                    let l = depth[&node];
                    layers[l].push(self.graph[node].clone());
                }
                // 防御性：不应有空层
                layers.retain(|l| !l.is_empty());

                let total: usize = layers.iter().map(|l| l.len()).sum();
                debug_assert_eq!(
                    total,
                    self.graph.node_count(),
                    "duplicate nodes in sort result: {} unique positions for {} nodes",
                    total,
                    self.graph.node_count()
                );
                Ok(layers)
            }
            Err(_cycle) => {
                // 用 Kahn 归约找出所有环中节点（入度仍 > 0 的节点）
                let mut in_deg: HashMap<NodeIndex, usize> = self
                    .graph
                    .node_indices()
                    .map(|n| {
                        let deg = self
                            .graph
                            .neighbors_directed(n, Direction::Incoming)
                            .count();
                        (n, deg)
                    })
                    .collect();

                let mut queue: Vec<NodeIndex> = in_deg
                    .iter()
                    .filter(|(_, &d)| d == 0)
                    .map(|(&n, _)| n)
                    .collect();

                while let Some(n) = queue.pop() {
                    for succ in self.graph.neighbors_directed(n, Direction::Outgoing) {
                        let d = in_deg.get_mut(&succ).unwrap();
                        *d = d.saturating_sub(1);
                        if *d == 0 {
                            queue.push(succ);
                        }
                    }
                }

                let cycle_nodes: Vec<NodeId> = in_deg
                    .iter()
                    .filter(|(_, &d)| d > 0)
                    .map(|(&n, _)| self.graph[n].clone())
                    .collect();
                Err(cycle_nodes)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_chain() {
        let mut dag = Dag::new();
        dag.add_edge("A".into(), "B".into());
        dag.add_edge("B".into(), "C".into());
        let layers = dag.sort().unwrap();
        assert_eq!(layers.len(), 3); // A → B → C
    }

    #[test]
    fn test_fork() {
        let mut dag = Dag::new();
        dag.add_edge("A".into(), "B".into());
        dag.add_edge("A".into(), "C".into());
        let layers = dag.sort().unwrap();
        assert_eq!(layers.len(), 2); // A → [B, C]
    }

    #[test]
    fn test_merge() {
        let mut dag = Dag::new();
        dag.add_edge("A".into(), "C".into());
        dag.add_edge("B".into(), "C".into());
        let layers = dag.sort().unwrap();
        assert_eq!(layers.len(), 2); // [A, B] → C
    }

    #[test]
    fn test_cycle_detected() {
        let mut dag = Dag::new();
        dag.add_edge("A".into(), "B".into());
        dag.add_edge("B".into(), "C".into());
        dag.add_edge("C".into(), "A".into());
        assert!(dag.sort().is_err());
    }

    #[test]
    fn test_add_duplicate_edge_is_idempotent() {
        // 无重复边的基准
        let mut dag1 = Dag::new();
        dag1.add_edge("A".into(), "B".into());
        dag1.add_edge("B".into(), "C".into());
        dag1.add_edge("A".into(), "C".into());
        let layers1 = dag1.sort().unwrap();

        // 相同 DAG，但 A→B 重复添加一次
        let mut dag2 = Dag::new();
        dag2.add_edge("A".into(), "B".into());
        dag2.add_edge("A".into(), "B".into()); // 重复边，应被去重
        dag2.add_edge("B".into(), "C".into());
        dag2.add_edge("A".into(), "C".into());
        let layers2 = dag2.sort().unwrap();

        // 重复边不应改变排序结果
        assert_eq!(layers1, layers2);
        assert_eq!(layers1.len(), 3); // A → B → C（C 同时依赖 A 或 B）
    }
}
