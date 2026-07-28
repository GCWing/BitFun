//! Welford's online algorithm — single-pass mean, variance, and CDF.
//! O(1) space, O(1) per update. Numerically stable for streaming tick data.

/// Welford's online statistics accumulator.
///
/// Computes count, mean, variance (sample), standard deviation, and
/// normal-approximation CDF in a single pass without storing all values.
#[derive(Debug, Clone)]
pub struct WelfordStats {
    count: u64,
    mean: f64,
    m2: f64,
}

impl Default for WelfordStats {
    fn default() -> Self {
        Self::new()
    }
}

impl WelfordStats {
    pub fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
        }
    }

    /// Add a single observation. O(1).
    pub fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    /// Merge another WelfordStats into this one. O(1).
    ///
    /// Correctly combines two independently-computed statistics
    /// (e.g., parallel bucket aggregation).
    pub fn merge(&mut self, other: &WelfordStats) {
        if other.count == 0 {
            return;
        }
        if self.count == 0 {
            *self = other.clone();
            return;
        }
        let total = self.count + other.count;
        let delta = other.mean - self.mean;
        self.m2 +=
            other.m2 + delta * delta * (self.count as f64 * other.count as f64) / total as f64;
        self.mean =
            (self.count as f64 * self.mean + other.count as f64 * other.mean) / total as f64;
        self.count = total;
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn mean(&self) -> f64 {
        self.mean
    }

    /// Sample variance (Bessel-corrected). Returns 0.0 when count < 2.
    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / (self.count - 1) as f64
        }
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Normal-approximation CDF: P(X ≤ value).
    ///
    /// Returns a value in [0, 1]. Uses the Abramowitz–Stegun rational
    /// Chebyshev approximation for erf (max error ~1.5e-7).
    /// Degenerate cases: count < 2 → 0.5, std_dev = 0 → step function.
    pub fn cdf(&self, value: f64) -> f64 {
        if self.count < 2 {
            return 0.5;
        }
        let sd = self.std_dev();
        if sd == 0.0 {
            return if value >= self.mean { 1.0 } else { 0.0 };
        }
        let z = (value - self.mean) / sd;
        0.5 * (1.0 + erf_approx(z / std::f64::consts::SQRT_2))
    }
}

/// Abramowitz & Stegun §7.1.26 rational Chebyshev approximation for erf(x).
/// Max absolute error: 1.5 × 10⁻⁷.
fn erf_approx(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    let p = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;

    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
    sign * y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean_variance_batch() {
        let mut w = WelfordStats::new();
        for v in [1.0f64, 2.0, 3.0, 4.0, 5.0] {
            w.update(v);
        }
        assert_eq!(w.count(), 5);
        assert!((w.mean() - 3.0).abs() < 1e-10);
        // Sample variance: Σ(x-μ)²/(n-1) = (4+1+0+1+4)/4 = 10/4 = 2.5
        assert!((w.variance() - 2.5).abs() < 1e-10);
        assert!((w.std_dev() - 2.5f64.sqrt()).abs() < 1e-10);
    }

    #[test]
    fn test_mean_variance_single() {
        let mut w = WelfordStats::new();
        w.update(42.0);
        assert_eq!(w.count(), 1);
        assert!((w.mean() - 42.0).abs() < 1e-10);
        assert!((w.variance() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_merge_equivalent_to_batch() {
        let mut batch = WelfordStats::new();
        for v in [1.0, 2.0, 3.0, 4.0, 5.0] {
            batch.update(v);
        }

        let mut w1 = WelfordStats::new();
        for v in [1.0, 2.0, 3.0] {
            w1.update(v);
        }
        let mut w2 = WelfordStats::new();
        for v in [4.0, 5.0] {
            w2.update(v);
        }
        w1.merge(&w2);

        assert_eq!(w1.count(), batch.count());
        assert!((w1.mean() - batch.mean()).abs() < 1e-10);
        assert!((w1.variance() - batch.variance()).abs() < 1e-10);
    }

    #[test]
    fn test_merge_with_empty() {
        let mut w1 = WelfordStats::new();
        w1.update(1.0);
        w1.update(2.0);

        let w2 = WelfordStats::new(); // empty
        w1.merge(&w2);
        assert_eq!(w1.count(), 2);
        assert!((w1.mean() - 1.5).abs() < 1e-10);
    }

    #[test]
    fn test_merge_into_empty() {
        let mut w1 = WelfordStats::new();
        let mut w2 = WelfordStats::new();
        w2.update(1.0);
        w2.update(3.0);

        w1.merge(&w2);
        assert_eq!(w1.count(), 2);
        assert!((w1.mean() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_cdf_range_0_to_1() {
        let mut w = WelfordStats::new();
        for v in [1.0, 2.0, 3.0, 4.0, 5.0] {
            w.update(v);
        }
        // Very low value → CDF close to 0
        let low = w.cdf(-1000.0);
        assert!(low >= 0.0 && low < 0.01, "low cdf = {}", low);
        // Very high value → CDF close to 1
        let high = w.cdf(1000.0);
        assert!(high > 0.99 && high <= 1.0, "high cdf = {}", high);
        // At mean → CDF ≈ 0.5
        let mid = w.cdf(3.0);
        assert!((mid - 0.5).abs() < 0.05, "mid cdf = {}", mid);
    }

    #[test]
    fn test_cdf_degenerate() {
        let w1 = WelfordStats::new();
        assert!((w1.cdf(0.0) - 0.5).abs() < 1e-10);

        let mut w2 = WelfordStats::new();
        w2.update(7.0);
        assert!((w2.cdf(0.0) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_cdf_constant_value() {
        let mut w = WelfordStats::new();
        for _ in 0..5 {
            w.update(10.0);
        }
        // All values equal → std_dev = 0 → step function
        assert!((w.cdf(9.9) - 0.0).abs() < 1e-10);
        assert!((w.cdf(10.0) - 1.0).abs() < 1e-10);
        assert!((w.cdf(10.1) - 1.0).abs() < 1e-10);
    }

    /// Quick sanity: Welford mean/variance must match naive batch formula.
    #[test]
    fn test_against_naive_batch() {
        let data: Vec<f64> = (0..1000).map(|i| (i as f64 * 0.37).sin()).collect();

        let mut w = WelfordStats::new();
        for &v in &data {
            w.update(v);
        }

        let n = data.len() as f64;
        let batch_mean: f64 = data.iter().sum::<f64>() / n;
        let batch_var: f64 = data.iter().map(|v| (v - batch_mean).powi(2)).sum::<f64>() / (n - 1.0);

        assert!((w.mean() - batch_mean).abs() < 1e-12);
        assert!((w.variance() - batch_var).abs() < 1e-12);
    }
}
