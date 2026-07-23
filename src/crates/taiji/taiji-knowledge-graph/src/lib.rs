//! Taiji knowledge graph — petgraph-backed concept/strategy/case relation graph.
//!
//! Three layers:
//! - Concept: 理论概念（量价时空 + 派生概念）
//! - Strategy: 7 Agent 策略规则
//! - Case: 数据指标 + golden tick 案例

pub mod embedding;
pub mod types;

use petgraph::graph::DiGraph;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use serde::Deserialize;
use std::collections::HashMap;
use types::*;

/// 编译时生成的 JSON 格式节点/边
#[derive(Deserialize)]
struct GeneratedData {
    nodes: Vec<ConceptNode>,
    edges: Vec<RelationEdge>,
}

impl GeneratedData {
    fn load() -> Self {
        let json_str = include_str!(concat!(env!("OUT_DIR"), "/generated_graph_data.json"));
        serde_json::from_str(json_str).expect("Failed to parse generated graph data")
    }
}

pub struct KnowledgeGraph {
    graph: DiGraph<ConceptNode, RelationEdge>,
    /// id → NodeIndex 快速查找
    index: HashMap<String, NodeIndex>,
}

impl KnowledgeGraph {
    /// 从编译时生成的数据构建知识图谱
    pub fn build() -> Self {
        let data = GeneratedData::load();

        log::info!(
            "Building knowledge graph: {} nodes, {} edges",
            data.nodes.len(),
            data.edges.len()
        );

        let mut graph = DiGraph::<ConceptNode, RelationEdge>::new();
        let mut index = HashMap::new();

        for node in data.nodes {
            let idx = graph.add_node(node.clone());
            index.insert(node.id.clone(), idx);
        }

        for edge in data.edges {
            if let (Some(&from_idx), Some(&to_idx)) = (index.get(&edge.from), index.get(&edge.to)) {
                graph.add_edge(from_idx, to_idx, edge);
            }
        }

        log::info!("Knowledge graph built successfully");
        Self { graph, index }
    }

    /// 节点数
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// 边数
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// 查询以 concept_id 为根的 2-hop 子图
    pub fn query_subgraph(&self, concept_id: &str) -> Option<SubgraphResponse> {
        let root_idx = *self.index.get(concept_id)?;

        let mut node_ids = std::collections::HashSet::new();
        node_ids.insert(root_idx);

        // 1-hop: 直接邻居
        for neighbor in self
            .graph
            .neighbors_directed(root_idx, petgraph::Direction::Outgoing)
        {
            node_ids.insert(neighbor);
        }
        for neighbor in self
            .graph
            .neighbors_directed(root_idx, petgraph::Direction::Incoming)
        {
            node_ids.insert(neighbor);
        }

        // 2-hop: 邻居的邻居
        let hop1: Vec<_> = node_ids
            .iter()
            .copied()
            .filter(|&i| i != root_idx)
            .collect();
        for &n in &hop1 {
            for nn in self
                .graph
                .neighbors_directed(n, petgraph::Direction::Outgoing)
            {
                node_ids.insert(nn);
            }
        }

        let nodes: Vec<ConceptNode> = node_ids
            .iter()
            .map(|&idx| self.graph[idx].clone())
            .collect();

        let edges: Vec<RelationEdge> = self
            .graph
            .edge_references()
            .filter(|e| node_ids.contains(&e.source()) && node_ids.contains(&e.target()))
            .map(|e| e.weight().clone())
            .collect();

        Some(SubgraphResponse { nodes, edges })
    }

    /// 计算 breadthfirst 层次布局（按层分配 y 坐标）
    pub fn compute_layout(&self) -> LayoutResponse {
        let mut positions: Vec<LayoutPosition> = Vec::new();
        let mut visited = HashMap::<NodeIndex, u32>::new();

        // 从入度为 0 的节点（理论根节点）开始 BFS
        let mut queue: Vec<(NodeIndex, u32)> = self
            .graph
            .node_indices()
            .filter(|&n| {
                self.graph
                    .neighbors_directed(n, petgraph::Direction::Incoming)
                    .next()
                    .is_none()
            })
            .map(|n| (n, 0))
            .collect();

        if queue.is_empty() {
            // fallback: 从所有节点开始
            for n in self.graph.node_indices() {
                queue.push((n, 0));
            }
        }

        let mut layer_counts: HashMap<u32, usize> = HashMap::new();
        while let Some((node_idx, layer)) = queue.pop() {
            if visited.contains_key(&node_idx) {
                continue;
            }
            visited.insert(node_idx, layer);

            let count = layer_counts.entry(layer).or_insert(0);
            let x = *count as f64 * 180.0;
            *count += 1;
            let y = layer as f64 * 120.0;

            positions.push(LayoutPosition {
                id: self.graph[node_idx].id.clone(),
                x,
                y,
                layer,
            });

            // 子节点入队
            let mut children: Vec<_> = self
                .graph
                .neighbors_directed(node_idx, petgraph::Direction::Outgoing)
                .filter(|n| !visited.contains_key(n))
                .collect();
            for child in children.drain(..) {
                queue.push((child, layer + 1));
            }
        }

        LayoutResponse { positions }
    }

    /// 模糊搜索节点（名称 + 描述包含关键词）
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();

        self.graph
            .node_indices()
            .filter_map(|idx| {
                let node = &self.graph[idx];
                let name_lower = node.name.to_lowercase();
                let desc_lower = node.description.to_lowercase();

                let score = if name_lower == query_lower {
                    1.0
                } else if name_lower.starts_with(&query_lower) {
                    0.9
                } else if name_lower.contains(&query_lower) {
                    0.7
                } else if desc_lower.contains(&query_lower) {
                    0.4
                } else {
                    return None;
                };

                let related_ids: Vec<String> = self
                    .graph
                    .neighbors_directed(idx, petgraph::Direction::Outgoing)
                    .chain(
                        self.graph
                            .neighbors_directed(idx, petgraph::Direction::Incoming),
                    )
                    .take(10)
                    .map(|n| self.graph[n].id.clone())
                    .collect();

                Some(SearchResult {
                    node: node.clone(),
                    score,
                    related_ids,
                })
            })
            .collect()
    }

    /// 按类别过滤节点
    pub fn nodes_by_category(&self, category: &NodeCategory) -> Vec<&ConceptNode> {
        self.graph
            .node_indices()
            .filter(|&idx| &self.graph[idx].category == category)
            .map(|idx| &self.graph[idx])
            .collect()
    }

    /// 返回全量节点（不暴露 petgraph 内部类型）
    pub fn all_nodes(&self) -> Vec<&ConceptNode> {
        self.graph
            .node_indices()
            .map(|idx| &self.graph[idx])
            .collect()
    }

    /// 返回全量边（不暴露 petgraph 内部类型）
    pub fn all_edges(&self) -> Vec<(String, String, &RelationEdge)> {
        self.graph
            .edge_references()
            .map(|e| {
                (
                    self.graph[e.source()].id.clone(),
                    self.graph[e.target()].id.clone(),
                    e.weight(),
                )
            })
            .collect()
    }

    /// 查找两个节点之间的最短路径（概念→策略→案例）
    pub fn path_between(&self, from_id: &str, to_id: &str) -> Option<Vec<String>> {
        let from_idx = *self.index.get(from_id)?;
        let to_idx = *self.index.get(to_id)?;

        let result = petgraph::algo::astar(&self.graph, from_idx, |n| n == to_idx, |_| 1, |_| 0);

        result.map(|(_cost, path)| path.iter().map(|&idx| self.graph[idx].id.clone()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_graph() {
        let kg = KnowledgeGraph::build();
        assert!(kg.node_count() > 0);
        assert!(kg.edge_count() > 0);
    }

    #[test]
    fn test_query_subgraph() {
        let kg = KnowledgeGraph::build();
        let result = kg.query_subgraph("agent_decision");
        assert!(result.is_some());
        let sub = result.unwrap();
        assert!(!sub.nodes.is_empty());
    }

    #[test]
    fn test_search() {
        let kg = KnowledgeGraph::build();
        let results = kg.search("结构");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_layout() {
        let kg = KnowledgeGraph::build();
        let layout = kg.compute_layout();
        assert!(!layout.positions.is_empty());
    }

    #[test]
    fn test_nodes_by_category() {
        let kg = KnowledgeGraph::build();
        let concepts = kg.nodes_by_category(&NodeCategory::Concept);
        let strategies = kg.nodes_by_category(&NodeCategory::Strategy);
        assert!(!concepts.is_empty());
        assert!(!strategies.is_empty());
    }

    #[test]
    fn test_path_between() {
        let kg = KnowledgeGraph::build();
        let path = kg.path_between("theory_structure", "data_trend_direction");
        assert!(path.is_some());
        let p = path.unwrap();
        assert!(p.contains(&"theory_structure".to_string()));
        assert!(p.contains(&"data_trend_direction".to_string()));
    }
}
