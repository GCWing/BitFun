//! taiji-abnormal — 异常检测评分卡
//!
//! 5 个指标 ComputeNode + ScorecardFusionNode 加权融合。
//! 全部从 OHLCV 计算，零 L2 依赖，在线 per on_bar() 模式。

pub mod corr_fracture;
pub mod gap_alert;
pub mod scorecard;
pub mod trend_accel;
pub mod vol_anomaly;
pub mod vol_regime;

use taiji_engine::node::ComputeNode;
use taiji_engine::types::bar::RawBar;
use statrs::statistics::Statistics;

// ── 共享常量 ──────────────────────────────────────────────────────────

/// 在线缓冲区最大 bar 数量（≈ 1 年日线）。
/// 超过此限制后从头部丢弃旧数据，保持内存有界。
pub(crate) const MAX_BARS: usize = 300;

// ── 共享类型 ──────────────────────────────────────────────────────────

/// 5 个异常指标权重（总和 = 1.0）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AbnormalWeights {
    pub vol_regime: f64,
    pub vol_anomaly: f64,
    pub corr_fracture: f64,
    pub gap_alert: f64,
    pub trend_accel: f64,
}

impl Default for AbnormalWeights {
    fn default() -> Self {
        Self {
            vol_regime: 0.25,
            vol_anomaly: 0.20,
            corr_fracture: 0.15,
            gap_alert: 0.25,
            trend_accel: 0.15,
        }
    }
}

/// 告警阈值
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AlertThresholds {
    /// warn=70 → 告警
    pub warn: f64,
    /// reduce=85 → 降仓位
    pub reduce: f64,
    /// emergency=95 → 熔断
    pub emergency: f64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            warn: 70.0,
            reduce: 85.0,
            emergency: 95.0,
        }
    }
}

/// 告警等级
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum AbnormalLevel {
    Normal,
    Warn,
    Reduce,
    Emergency,
}

impl AbnormalLevel {
    pub fn from_score(score: f64, thresholds: &AlertThresholds) -> Self {
        if score >= thresholds.emergency {
            AbnormalLevel::Emergency
        } else if score >= thresholds.reduce {
            AbnormalLevel::Reduce
        } else if score >= thresholds.warn {
            AbnormalLevel::Warn
        } else {
            AbnormalLevel::Normal
        }
    }
}

// ── 异常指标 trait ────────────────────────────────────────────────────

/// 异常指标节点 trait。
/// 每个指标节点既是 ComputeNode，也暴露纯函数 compute_score 供测试。
pub trait AbnormalIndicator: ComputeNode {
    /// 从 OHLCV bars 计算异常分数 (0-100)。
    /// `lookback` 控制回溯窗口长度。
    fn compute_score(&self, bars: &[RawBar], lookback: usize) -> f64;
}

// ── 统计工具函数（委托给 statrs crate）───────────────────────────────

/// 算术平均
pub(crate) fn mean(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    data.mean()
}

/// 样本标准差（除以 n-1）
pub(crate) fn std_dev(data: &[f64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    data.std_dev()
}

/// Pearson 相关系数（statrs 不提供，保留手写实现）
pub(crate) fn pearson_r(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() || x.len() < 2 {
        return 0.0;
    }
    let mx = mean(x);
    let my = mean(y);
    let sx = std_dev(x);
    let sy = std_dev(y);
    if sx == 0.0 || sy == 0.0 {
        return 0.0;
    }
    let cov = x
        .iter()
        .zip(y.iter())
        .map(|(xi, yi)| (xi - mx) * (yi - my))
        .sum::<f64>()
        / (x.len() - 1) as f64;
    cov / (sx * sy)
}

/// 排序数组的百分位数（线性插值）。
/// 注意：statrs 的 `OrderStatistics::percentile` 需要 `&mut self`、
/// 接受 `usize` 参数且使用不同的插值公式，不适合替换此函数。
pub(crate) fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let k = (pct / 100.0) * (sorted.len() - 1) as f64;
    let lo = k.floor() as usize;
    let hi = k.ceil() as usize;
    if lo == hi || hi >= sorted.len() {
        return sorted[lo.min(sorted.len() - 1)];
    }
    let frac = k - lo as f64;
    sorted[lo] + frac * (sorted[hi] - sorted[lo])
}

/// 简单线性回归：`(slope, intercept)`（statrs 不提供，保留手写实现）
pub(crate) fn linear_regression(x: &[f64], y: &[f64]) -> (f64, f64) {
    if x.len() != y.len() || x.len() < 2 {
        return (0.0, 0.0);
    }
    let mx = mean(x);
    let my = mean(y);
    let cov = x
        .iter()
        .zip(y.iter())
        .map(|(xi, yi)| (xi - mx) * (yi - my))
        .sum::<f64>();
    let var_x = x.iter().map(|xi| (xi - mx).powi(2)).sum::<f64>();
    if var_x == 0.0 {
        return (0.0, my);
    }
    let slope = cov / var_x;
    let intercept = my - slope * mx;
    (slope, intercept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean_empty() {
        assert_eq!(mean(&[]), 0.0);
    }

    #[test]
    fn test_mean_basic() {
        assert!((mean(&[1.0, 2.0, 3.0]) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_std_dev_basic() {
        let s = std_dev(&[1.0, 2.0, 3.0]);
        assert!((s - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_pearson_r_perfect() {
        let r = pearson_r(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]);
        assert!((r - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_pearson_r_imperfect() {
        let r = pearson_r(&[1.0, 2.0, 3.0], &[3.0, 2.0, 1.0]);
        assert!((r + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_percentile_median() {
        let mut data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        data.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!((percentile(&data, 50.0) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_percentile_p80() {
        let mut data: Vec<f64> = (1..=10).map(|i| i as f64).collect();
        data.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p80 = percentile(&data, 80.0);
        assert!(p80 > 8.0 && p80 < 9.0);
    }

    #[test]
    fn test_alert_level_from_score() {
        let thresholds = AlertThresholds::default();
        assert_eq!(
            AbnormalLevel::from_score(50.0, &thresholds),
            AbnormalLevel::Normal
        );
        assert_eq!(AbnormalLevel::from_score(75.0, &thresholds), AbnormalLevel::Warn);
        assert_eq!(
            AbnormalLevel::from_score(90.0, &thresholds),
            AbnormalLevel::Reduce
        );
        assert_eq!(
            AbnormalLevel::from_score(98.0, &thresholds),
            AbnormalLevel::Emergency
        );
        // 边界值
        assert_eq!(AbnormalLevel::from_score(70.0, &thresholds), AbnormalLevel::Warn);
        assert_eq!(
            AbnormalLevel::from_score(85.0, &thresholds),
            AbnormalLevel::Reduce
        );
        assert_eq!(
            AbnormalLevel::from_score(95.0, &thresholds),
            AbnormalLevel::Emergency
        );
    }
}
