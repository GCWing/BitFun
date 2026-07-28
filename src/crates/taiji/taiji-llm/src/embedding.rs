//! EmbeddingService — 文本嵌入服务抽象。
//!
//! 提供统一的 [`EmbeddingService`] trait，支持：
//! - [`MockEmbeddingService`] — 测试用固定维度向量
//! - [`CandleEmbeddingService`] — candle 本地推理（需模型文件）
//!
//! # 使用示例
//!
//! ```ignore
//! use taiji_llm::embedding::{EmbeddingService, MockEmbeddingService};
//!
//! async fn example() {
//!     let svc = MockEmbeddingService::new(384);
//!     let vecs = svc.embed(&["你好".into(), "世界".into()]).await.unwrap();
//!     assert_eq!(vecs.len(), 2);
//!     assert_eq!(vecs[0].len(), 384);
//! }
//! ```

use async_trait::async_trait;
use std::path::Path;

// ── EmbeddingService trait ────────────────────────────────────────────────

/// 文本嵌入服务抽象。
///
/// 支持批量嵌入（`embed`）和单条嵌入（`embed_single`），
/// 返回 `dimension()` 维浮点向量。
#[async_trait]
pub trait EmbeddingService: Send + Sync {
    /// 批量嵌入多条文本，返回 `[text_count][dim]` 的向量列表。
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, anyhow::Error>;

    /// 嵌入单条文本。
    async fn embed_single(&self, text: &str) -> Result<Vec<f32>, anyhow::Error> {
        let mut results = self.embed(&[text.to_string()]).await?;
        Ok(results.pop().unwrap_or_else(|| vec![0.0; self.dimension()]))
    }

    /// 嵌入向量的维度。
    fn dimension(&self) -> usize;
}

// ── MockEmbeddingService（测试用）──────────────────────────────────────────

/// 测试用嵌入服务——基于文本 hash 生成确定性伪向量。
pub struct MockEmbeddingService {
    dim: usize,
}

impl MockEmbeddingService {
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }

    /// 基于文本生成确定性伪嵌入向量。
    /// 使用简单的累加 hash 将其映射到 [0.0, 1.0) 区间。
    fn pseudo_embed(&self, text: &str) -> Vec<f32> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        use std::hash::Hasher;
        hasher.write(text.as_bytes());
        let mut seed = hasher.finish();

        (0..self.dim)
            .map(|_| {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((seed >> 33) as f32) / (u32::MAX as f32)
            })
            .collect()
    }
}

#[async_trait]
impl EmbeddingService for MockEmbeddingService {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, anyhow::Error> {
        Ok(texts.iter().map(|t| self.pseudo_embed(t)).collect())
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

// ── CandleEmbeddingService（candle 本地推理）───────────────────────────────

/// 基于 candle 的本地嵌入服务。
///
/// 使用 all-MiniLM-L6-v2（384 维）或类似 Sentence-BERT 模型。
/// 需要下载模型文件到 `model_path` 目录。
pub struct CandleEmbeddingService {
    /// 嵌入向量的维度
    dim: usize,
    /// 模型文件路径
    model_path: std::path::PathBuf,
    /// 分词器文件路径
    tokenizer_path: std::path::PathBuf,
}

impl CandleEmbeddingService {
    /// 创建 candle 嵌入服务。
    ///
    /// `model_path` — 包含 model.safetensors 或 .gguf 的目录
    /// `tokenizer_path` — tokenizer.json 文件路径
    /// `dim` — 模型输出维度（all-MiniLM-L6-v2 = 384）
    pub fn new(model_path: &Path, tokenizer_path: &Path, dim: usize) -> Self {
        Self {
            dim,
            model_path: model_path.to_path_buf(),
            tokenizer_path: tokenizer_path.to_path_buf(),
        }
    }

    /// 模型文件所在目录。
    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// 分词器文件路径。
    pub fn tokenizer_path(&self) -> &Path {
        &self.tokenizer_path
    }
}

#[async_trait]
impl EmbeddingService for CandleEmbeddingService {
    async fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, anyhow::Error> {
        // 实际推理需要 candle-transformers 加载 BERT 模型。
        // 当前为占位实现，返回零向量。
        // Phase 2: 集成 candle-transformers 的 BertModel::load() + forward()。
        Ok(vec![vec![0.0_f32; self.dim]; _texts.len()])
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

// ── 测试 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_embedding_dimension() {
        let svc = MockEmbeddingService::new(384);
        assert_eq!(svc.dimension(), 384);
    }

    #[tokio::test]
    async fn test_mock_embed_single() {
        let svc = MockEmbeddingService::new(128);
        let vec = svc.embed_single("量价时空").await.unwrap();
        assert_eq!(vec.len(), 128);
        // 所有值应在 [0.0, 1.0) 区间
        for &v in &vec {
            assert!(v >= 0.0 && v < 1.0, "value {} out of [0,1)", v);
        }
    }

    #[tokio::test]
    async fn test_mock_embed_batch() {
        let svc = MockEmbeddingService::new(64);
        let texts: Vec<String> = vec!["多头".into(), "空头".into(), "震荡".into()];
        let results = svc.embed(&texts).await.unwrap();
        assert_eq!(results.len(), 3);
        for v in &results {
            assert_eq!(v.len(), 64);
        }
    }

    #[tokio::test]
    async fn test_mock_deterministic() {
        let svc = MockEmbeddingService::new(16);
        let a1 = svc.embed_single("测试文本").await.unwrap();
        let a2 = svc.embed_single("测试文本").await.unwrap();
        assert_eq!(a1, a2, "same text should produce same embedding");
    }

    #[tokio::test]
    async fn test_mock_different_texts_different_embedding() {
        let svc = MockEmbeddingService::new(16);
        let a = svc.embed_single("多头").await.unwrap();
        let b = svc.embed_single("空头").await.unwrap();
        assert_ne!(a, b, "different texts should produce different embeddings");
    }

    #[test]
    fn test_candle_service_metadata() {
        let svc = CandleEmbeddingService::new(
            std::path::Path::new("/models/all-MiniLM-L6-v2"),
            std::path::Path::new("/models/tokenizer.json"),
            384,
        );
        assert_eq!(svc.dimension(), 384);
        assert_eq!(
            svc.model_path(),
            std::path::Path::new("/models/all-MiniLM-L6-v2")
        );
        assert_eq!(
            svc.tokenizer_path(),
            std::path::Path::new("/models/tokenizer.json")
        );
    }

    #[tokio::test]
    async fn test_candle_embed_returns_correct_dimension() {
        let svc = CandleEmbeddingService::new(
            std::path::Path::new("/mock/model"),
            std::path::Path::new("/mock/tokenizer.json"),
            384,
        );
        let results = svc.embed(&["test".into()]).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].len(), 384);
    }
}
