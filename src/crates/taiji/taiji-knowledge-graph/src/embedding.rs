//! 语义嵌入索引 —— RAG 语义搜索升级。
//!
//! 将知识图谱节点文本嵌入为稠密向量，
//! 通过余弦相似度进行近似最近邻（ANN）搜索。
//!
//! # 三层搜索架构
//!
//! ```text
//! L1: Grep（精确关键词匹配）—— 已有（KnowledgeGraph::search）
//! L2: Semantic（语义嵌入 → cosine ANN）—— 本模块
//! L3: Hybrid（Grep + Semantic 混合重排序）—— 本模块
//! ```

use std::collections::HashMap;

use crate::types::ConceptNode;

// ── 余弦相似度 ────────────────────────────────────────────────────────────

/// 计算两个向量的余弦相似度。
///
/// cos(a, b) = dot(a, b) / (||a|| * ||b||)
///
/// 返回值 ∈ [-1.0, 1.0]，1.0 表示完全相同方向。
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "vector dimensions must match");

    let (dot, norm_a, norm_b) = a
        .iter()
        .zip(b.iter())
        .fold((0.0_f32, 0.0_f32, 0.0_f32), |(d, na, nb), (&x, &y)| {
            (d + x * y, na + x * x, nb + y * y)
        });

    let denom = (norm_a * norm_b).sqrt();
    if denom < f32::EPSILON {
        0.0
    } else {
        (dot / denom).clamp(-1.0, 1.0)
    }
}

// ── SemanticIndex ─────────────────────────────────────────────────────────

/// 语义嵌入索引。
///
/// 预计算所有知识图谱节点的文本嵌入向量，
/// 查询时通过余弦相似度进行 ANN 搜索。
#[derive(Default)]
pub struct SemanticIndex {
    /// node_id → 嵌入向量
    embeddings: HashMap<String, Vec<f32>>,
}

impl SemanticIndex {
    /// 创建空索引。
    pub fn new() -> Self {
        Self {
            embeddings: HashMap::new(),
        }
    }

    /// 索引中的向量数量。
    pub fn len(&self) -> usize {
        self.embeddings.len()
    }

    /// 索引是否为空。
    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }

    /// 添加一个节点的嵌入向量。
    pub fn insert(&mut self, node_id: String, embedding: Vec<f32>) {
        self.embeddings.insert(node_id, embedding);
    }

    /// 批量索引节点。
    ///
    /// 对每个节点，将其 name + description 拼接后调用 `embed_fn` 生成向量。
    pub async fn index_nodes<F, Fut>(
        &mut self,
        nodes: &[ConceptNode],
        embed_fn: F,
    ) -> Result<(), anyhow::Error>
    where
        F: Fn(String) -> Fut,
        Fut: std::future::Future<Output = Result<Vec<f32>, anyhow::Error>>,
    {
        for node in nodes {
            let text = format!("{}: {}", node.name, node.description);
            let embedding = embed_fn(text).await?;
            self.embeddings.insert(node.id.clone(), embedding);
        }
        Ok(())
    }

    /// 语义搜索：对查询向量与所有索引向量计算余弦相似度，返回 top_k。
    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Vec<SearchHit> {
        let mut scores: Vec<(String, f32)> = self
            .embeddings
            .iter()
            .map(|(node_id, emb)| {
                let sim = cosine_similarity(query_embedding, emb);
                (node_id.clone(), sim)
            })
            .collect();

        // 按相似度降序排序
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scores
            .into_iter()
            .take(top_k)
            .map(|(node_id, similarity)| SearchHit {
                node_id,
                similarity,
            })
            .collect()
    }

    /// 获取某个节点的嵌入向量。
    pub fn get(&self, node_id: &str) -> Option<&Vec<f32>> {
        self.embeddings.get(node_id)
    }
}

// ── SearchHit ─────────────────────────────────────────────────────────────

/// 语义搜索结果。
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    /// 匹配节点 ID
    pub node_id: String,
    /// 余弦相似度 ∈ [-1.0, 1.0]
    pub similarity: f32,
}

// ── HybridSearch ──────────────────────────────────────────────────────────

/// 混合搜索模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// L1: 精确关键词匹配
    Grep,
    /// L2: 语义嵌入 → cosine ANN
    Semantic,
    /// L3: Grep + Semantic 混合重排序
    Hybrid,
}

/// 混合搜索重排序结果。
#[derive(Debug, Clone)]
pub struct HybridResult {
    /// 最终排序后的节点 ID
    pub node_ids: Vec<String>,
    /// 每个节点的综合得分
    pub scores: Vec<f32>,
    /// 使用的搜索模式
    pub mode: SearchMode,
}

/// Grep + Semantic 混合重排序。
///
/// 将 Grep 匹配分数（0.0-1.0）与语义相似度（-1.0-1.0 → 归一化到 0.0-1.0）
/// 按 0.3:0.7 权重加权融合，取 top_k。
///
/// `grep_hits` — Grep 搜索结果，格式为 `[(node_id, grep_score), ...]`
/// `semantic_hits` — 语义搜索结果
/// `top_k` — 返回数量
pub fn hybrid_rerank(
    grep_hits: &[(String, f32)],
    semantic_hits: &[SearchHit],
    top_k: usize,
) -> HybridResult {
    let grep_weight: f32 = 0.3;
    let semantic_weight: f32 = 0.7;

    // 归一化语义相似度到 [0, 1]
    let sem_scores: HashMap<&str, f32> = semantic_hits
        .iter()
        .map(|h| (h.node_id.as_str(), (h.similarity + 1.0) / 2.0))
        .collect();

    let grep_scores: HashMap<&str, f32> =
        grep_hits.iter().map(|(id, s)| (id.as_str(), *s)).collect();

    // 收集所有候选节点
    let mut all_ids: Vec<&str> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (id, _) in grep_hits {
        if seen.insert(id.as_str()) {
            all_ids.push(id.as_str());
        }
    }
    for hit in semantic_hits {
        if seen.insert(hit.node_id.as_str()) {
            all_ids.push(hit.node_id.as_str());
        }
    }

    // 加权融合
    let mut merged: Vec<(String, f32)> = all_ids
        .into_iter()
        .map(|id| {
            let gs = grep_scores.get(id).copied().unwrap_or(0.0);
            let ss = sem_scores.get(id).copied().unwrap_or(0.0);
            let combined = grep_weight * gs + semantic_weight * ss;
            (id.to_string(), combined)
        })
        .collect();

    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    merged.truncate(top_k);

    let node_ids: Vec<String> = merged.iter().map(|(id, _)| id.clone()).collect();
    let scores: Vec<f32> = merged.iter().map(|(_, s)| *s).collect();

    HybridResult {
        node_ids,
        scores,
        mode: SearchMode::Hybrid,
    }
}

// ── 测试 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── cosine_similarity 测试 ──────────────────────────────────────

    #[test]
    fn test_cosine_identical_vectors() {
        let v = vec![1.0_f32, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "identical vectors should have cos=1.0"
        );
    }

    #[test]
    fn test_cosine_orthogonal_vectors() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![0.0_f32, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6, "orthogonal vectors should have cos=0.0");
    }

    #[test]
    fn test_cosine_opposite_vectors() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![-1.0_f32, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(
            (sim + 1.0).abs() < 1e-6,
            "opposite vectors should have cos=-1.0"
        );
    }

    #[test]
    fn test_cosine_zero_vector() {
        let a = vec![0.0_f32, 0.0];
        let b = vec![1.0_f32, 2.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0, "zero vector should return cos=0.0");
    }

    #[test]
    fn test_cosine_similarity_range() {
        // 随机向量验证 cos 始终在 [-1, 1]
        let a = vec![0.5_f32, -0.3, 0.8, -0.1];
        let b = vec![-0.2_f32, 0.7, 0.1, 0.9];
        let sim = cosine_similarity(&a, &b);
        assert!(
            sim >= -1.0 && sim <= 1.0,
            "cosine should be in [-1, 1], got {}",
            sim
        );
    }

    // ── SemanticIndex 测试 ────────────────────────────────────────────

    fn make_test_embeddings() -> SemanticIndex {
        let mut idx = SemanticIndex::new();
        idx.insert("theory_structure".into(), vec![1.0_f32, 0.0, 0.0]);
        idx.insert("theory_trend".into(), vec![0.0_f32, 1.0, 0.0]);
        idx.insert("data_volume".into(), vec![0.9_f32, 0.1, 0.0]);
        idx.insert("data_price".into(), vec![0.0_f32, 0.0, 1.0]);
        idx
    }

    #[test]
    fn test_semantic_index_len() {
        let idx = make_test_embeddings();
        assert_eq!(idx.len(), 4);
        assert!(!idx.is_empty());
    }

    #[test]
    fn test_semantic_index_empty() {
        let idx = SemanticIndex::new();
        assert_eq!(idx.len(), 0);
        assert!(idx.is_empty());
    }

    #[test]
    fn test_semantic_search_top1() {
        let idx = make_test_embeddings();
        // 查询向量与 theory_structure 最相似
        let query = vec![1.0_f32, 0.0, 0.0];
        let hits = idx.search(&query, 1);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].node_id, "theory_structure");
        assert!((hits[0].similarity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_semantic_search_top3() {
        let idx = make_test_embeddings();
        // 查询与 theory_structure 完全一致的方向
        let query = vec![1.0_f32, 0.0, 0.0];
        let hits = idx.search(&query, 3);
        assert_eq!(hits.len(), 3);

        // 第一个应是最相似节点（theory_structure 或 data_volume）
        // 相似度应递减
        for i in 1..hits.len() {
            assert!(
                hits[i - 1].similarity >= hits[i].similarity,
                "results should be sorted by descending similarity"
            );
        }

        // theory_structure 的相似度应接近 1.0（查询完全相同）
        let ts_hit = hits
            .iter()
            .find(|h| h.node_id == "theory_structure")
            .unwrap();
        assert!((ts_hit.similarity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_semantic_search_recall_vs_grep() {
        // 语义搜索应能召回 Grep 无法匹配的语义相关节点。
        let mut idx = SemanticIndex::new();

        // 节点描述包含语义相近但关键词不同的内容
        idx.insert("node_a".into(), vec![0.7_f32, 0.3, 0.1]);
        idx.insert("node_b".into(), vec![0.68_f32, 0.32, 0.08]); // 与 node_a 高度相似
        idx.insert("node_c".into(), vec![-0.5_f32, 0.8, -0.2]); // 与 node_a 不相似

        // 查询 node_a 的嵌入
        let query = vec![0.7_f32, 0.3, 0.1];
        let hits = idx.search(&query, 2);

        // node_b 应与 node_a 语义相近（召回），即使关键词不匹配
        let hit_ids: Vec<&str> = hits.iter().map(|h| h.node_id.as_str()).collect();
        assert!(
            hit_ids.contains(&"node_a"),
            "semantic search should recall node_a itself"
        );
        assert!(
            hit_ids.contains(&"node_b"),
            "semantic search should recall semantically similar node_b"
        );

        // node_b 的语义相似度应 > 0.9（高度相似）
        let node_b_hit = hits.iter().find(|h| h.node_id == "node_b").unwrap();
        assert!(
            node_b_hit.similarity > 0.9,
            "node_b similarity should be > 0.9, got {}",
            node_b_hit.similarity
        );
    }

    #[test]
    fn test_semantic_index_get() {
        let idx = make_test_embeddings();
        let emb = idx.get("theory_structure").unwrap();
        assert_eq!(emb, &vec![1.0_f32, 0.0, 0.0]);
        assert!(idx.get("nonexistent").is_none());
    }

    // ── HybridSearch 测试 ────────────────────────────────────────────

    #[test]
    fn test_hybrid_rerank_combines_both_sources() {
        let grep_hits = vec![("node_x".into(), 1.0_f32), ("node_y".into(), 0.8)];
        let semantic_hits = vec![
            SearchHit {
                node_id: "node_z".into(),
                similarity: 0.9,
            },
            SearchHit {
                node_id: "node_x".into(),
                similarity: 0.6,
            },
        ];

        let result = hybrid_rerank(&grep_hits, &semantic_hits, 3);
        assert_eq!(result.mode, SearchMode::Hybrid);
        assert_eq!(result.node_ids.len(), 3);

        // node_x 应在 Grep 和 Semantic 中都被命中，综合得分最高
        assert!(
            result.node_ids.contains(&"node_x".into()),
            "hybrid should include node from both sources"
        );
        assert!(
            result.node_ids.contains(&"node_z".into()),
            "hybrid should include semantic-only hit"
        );
    }

    #[test]
    fn test_hybrid_rerank_truncates_to_top_k() {
        let grep_hits: Vec<(String, f32)> =
            (0..10).map(|i| (format!("grep_{}", i), 0.5_f32)).collect();
        let semantic_hits: Vec<SearchHit> = (0..10)
            .map(|i| SearchHit {
                node_id: format!("sem_{}", i),
                similarity: 0.7,
            })
            .collect();

        let result = hybrid_rerank(&grep_hits, &semantic_hits, 5);
        assert_eq!(result.node_ids.len(), 5);
        assert_eq!(result.scores.len(), 5);
    }

    #[test]
    fn test_hybrid_rerank_scores_descending() {
        let grep_hits = vec![("a".into(), 0.9), ("b".into(), 0.3)];
        let semantic_hits = vec![SearchHit {
            node_id: "c".into(),
            similarity: 0.8,
        }];

        let result = hybrid_rerank(&grep_hits, &semantic_hits, 5);
        for i in 1..result.scores.len() {
            assert!(
                result.scores[i - 1] >= result.scores[i],
                "hybrid scores should be descending"
            );
        }
    }
}
