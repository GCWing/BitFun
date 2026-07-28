//! Fear & Greed Index — 五因子情绪温度计。
//!
//! 参考：CNN Fear & Greed Index 方法论，适配期货市场：
//! - HV20（25%）：历史波动率越高 → 恐惧 ↑
//! - 商品动量（25%）：价格涨幅越大 → 贪婪 ↑
//! - 持仓变化率（20%）：OI 增长 → 贪婪 ↑
//! - 基差斜率（15%）：基差走强 → 贪婪 ↑
//! - NLP 情绪（15%）：文本情绪得分
//!
//! 输出 0-100，阈值：0-25 极度恐惧 / 25-45 恐惧 / 45-55 中性 / 55-75 贪婪 / 75-100 极度贪婪。

// ── FGI 分类 ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FgiCategory {
    ExtremeFear,  //   0 – 25
    Fear,         //  25 – 45
    Neutral,      //  45 – 55
    Greed,        //  55 – 75
    ExtremeGreed, //  75 – 100
}

impl FgiCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            FgiCategory::ExtremeFear => "极度恐惧",
            FgiCategory::Fear => "恐惧",
            FgiCategory::Neutral => "中性",
            FgiCategory::Greed => "贪婪",
            FgiCategory::ExtremeGreed => "极度贪婪",
        }
    }
}

// ── 权重 ────────────────────────────────────────────────────────────────

const W_HV20: f64 = 0.25;
const W_MOMENTUM: f64 = 0.25;
const W_OI: f64 = 0.20;
const W_BASIS: f64 = 0.15;
const W_NLP: f64 = 0.15;

// ── FGI 计算器 ──────────────────────────────────────────────────────────

pub struct FearGreedIndex;

impl FearGreedIndex {
    /// 计算 Fear & Greed Index（0-100）。
    ///
    /// 每个输入因子需预先归一化到 0-100 贡献值：
    /// - `hv20`: 波动率贡献（高波动 → 低值）
    /// - `commodity_momentum`: 动量贡献（正动量 → 高值）
    /// - `oi_change_rate`: 持仓变化贡献（正变化 → 高值）
    /// - `basis_slope`: 基差贡献（正斜率 → 高值）
    /// - `nlp_sentiment`: NLP 情绪贡献（正面 → 高值）
    pub fn compute(
        hv20: f64,
        commodity_momentum: f64,
        oi_change_rate: f64,
        basis_slope: f64,
        nlp_sentiment: f64,
    ) -> f64 {
        let fgi = hv20 * W_HV20
            + commodity_momentum * W_MOMENTUM
            + oi_change_rate * W_OI
            + basis_slope * W_BASIS
            + nlp_sentiment * W_NLP;

        fgi.clamp(0.0, 100.0)
    }

    /// 将 FGI 值映射为分类。
    pub fn classify(fgi: f64) -> FgiCategory {
        match fgi {
            x if x < 25.0 => FgiCategory::ExtremeFear,
            x if x < 45.0 => FgiCategory::Fear,
            x if x < 55.0 => FgiCategory::Neutral,
            x if x < 75.0 => FgiCategory::Greed,
            _ => FgiCategory::ExtremeGreed,
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fgi_weights_sum_to_one() {
        let total = W_HV20 + W_MOMENTUM + W_OI + W_BASIS + W_NLP;
        assert!(
            (total - 1.0).abs() < 1e-9,
            "5 因子权重和应为 1.0，实际: {}",
            total
        );
    }

    #[test]
    fn test_fgi_all_neutral() {
        // 所有因子 50 = 完全中性
        let fgi = FearGreedIndex::compute(50.0, 50.0, 50.0, 50.0, 50.0);
        assert!(
            (fgi - 50.0).abs() < 0.01,
            "全中性输入应得 FGI=50，实际: {}",
            fgi
        );
    }

    #[test]
    fn test_fgi_extreme_greed() {
        // 所有因子 100 = 极度贪婪
        let fgi = FearGreedIndex::compute(100.0, 100.0, 100.0, 100.0, 100.0);
        assert!(fgi > 75.0, "全贪婪输入应得 >75，实际: {}", fgi);
        assert_eq!(FearGreedIndex::classify(fgi), FgiCategory::ExtremeGreed);
    }

    #[test]
    fn test_fgi_extreme_fear() {
        // 所有因子 0 = 极度恐惧
        let fgi = FearGreedIndex::compute(0.0, 0.0, 0.0, 0.0, 0.0);
        assert!(fgi < 25.0, "全恐惧输入应得 <25，实际: {}", fgi);
        assert_eq!(FearGreedIndex::classify(fgi), FgiCategory::ExtremeFear);
    }

    #[test]
    fn test_fgi_clamped() {
        // 超出范围应被 clamp 到 [0, 100]
        let fgi = FearGreedIndex::compute(200.0, 200.0, 200.0, 200.0, 200.0);
        assert!(fgi <= 100.0, "上界应为 100，实际: {}", fgi);

        let fgi = FearGreedIndex::compute(-50.0, -50.0, -50.0, -50.0, -50.0);
        assert!(fgi >= 0.0, "下界应为 0，实际: {}", fgi);
    }

    #[test]
    fn test_classify_boundaries() {
        assert_eq!(FearGreedIndex::classify(0.0), FgiCategory::ExtremeFear);
        assert_eq!(FearGreedIndex::classify(24.9), FgiCategory::ExtremeFear);
        assert_eq!(FearGreedIndex::classify(25.0), FgiCategory::Fear);
        assert_eq!(FearGreedIndex::classify(44.9), FgiCategory::Fear);
        assert_eq!(FearGreedIndex::classify(45.0), FgiCategory::Neutral);
        assert_eq!(FearGreedIndex::classify(54.9), FgiCategory::Neutral);
        assert_eq!(FearGreedIndex::classify(55.0), FgiCategory::Greed);
        assert_eq!(FearGreedIndex::classify(74.9), FgiCategory::Greed);
        assert_eq!(FearGreedIndex::classify(75.0), FgiCategory::ExtremeGreed);
        assert_eq!(FearGreedIndex::classify(100.0), FgiCategory::ExtremeGreed);
    }

    #[test]
    fn test_weighted_contribution() {
        // 仅动量=100，其余中性 50
        // FGI = 50*0.25 + 100*0.25 + 50*0.20 + 50*0.15 + 50*0.15
        //     = 12.5 + 25.0 + 10.0 + 7.5 + 7.5 = 62.5
        let fgi = FearGreedIndex::compute(50.0, 100.0, 50.0, 50.0, 50.0);
        assert!(
            (fgi - 62.5).abs() < 0.01,
            "仅动量=100 应得 62.5，实际: {}",
            fgi
        );
    }
}
