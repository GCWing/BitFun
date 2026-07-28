use pyo3::prelude::*;

/// 奖励计算器：r = log_return + α·diff_sharpe - β·dd_penalty - γ·cost - δ·holding
///
/// # 参数
/// - `alpha`: Sharpe 差异权重（鼓励策略改进）
/// - `beta`:  回撤惩罚权重（抑制大回撤）
/// - `gamma`: 交易成本权重（每笔交易固定成本）
/// - `delta`: 持仓惩罚权重（惩罚过度持仓）
#[pyclass]
#[derive(Clone)]
pub struct RewardCalculator {
    #[pyo3(get, set)]
    pub alpha: f64,
    #[pyo3(get, set)]
    pub beta: f64,
    #[pyo3(get, set)]
    pub gamma: f64,
    #[pyo3(get, set)]
    pub delta: f64,
}

#[pymethods]
impl RewardCalculator {
    #[new]
    fn new(alpha: f64, beta: f64, gamma: f64, delta: f64) -> Self {
        Self {
            alpha,
            beta,
            gamma,
            delta,
        }
    }

    /// Python 可调用的奖励计算（方便测试和调试）。
    fn calc(
        &self,
        log_return: f64,
        prev_sharpe: f64,
        curr_sharpe: f64,
        drawdown_pct: f64,
        traded: bool,
        is_holding: bool,
    ) -> f64 {
        self.calculate(
            log_return,
            prev_sharpe,
            curr_sharpe,
            drawdown_pct,
            traded,
            is_holding,
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "RewardCalculator(α={:.4}, β={:.4}, γ={:.4}, δ={:.4})",
            self.alpha, self.beta, self.gamma, self.delta
        )
    }
}

impl RewardCalculator {
    /// 计算单步奖励。
    ///
    /// r = log_return + α·(curr_sharpe - prev_sharpe) - β·drawdown_pct
    ///     - γ·[traded] - δ·[is_holding]
    pub fn calculate(
        &self,
        log_return: f64,
        prev_sharpe: f64,
        curr_sharpe: f64,
        drawdown_pct: f64,
        traded: bool,
        is_holding: bool,
    ) -> f64 {
        let sharpe_diff = curr_sharpe - prev_sharpe;
        let cost_penalty = if traded { self.gamma } else { 0.0 };
        let holding_penalty = if is_holding { self.delta } else { 0.0 };

        log_return + self.alpha * sharpe_diff
            - self.beta * drawdown_pct
            - cost_penalty
            - holding_penalty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_positive_return_no_penalty() {
        let calc = RewardCalculator::new(0.5, 1.0, 0.01, 0.005);
        let r = calc.calculate(0.02, 1.0, 1.05, 0.0, false, false);
        // r = 0.02 + 0.5*0.05 - 0 - 0 - 0 = 0.045
        assert!((r - 0.045).abs() < 1e-9);
    }

    #[test]
    fn test_negative_drawdown_penalty() {
        let calc = RewardCalculator::new(0.5, 1.0, 0.01, 0.005);
        let r = calc.calculate(0.01, 1.0, 1.0, 0.05, true, false);
        // r = 0.01 + 0 - 0.05 - 0.01 - 0 = -0.05
        assert!((r - (-0.05)).abs() < 1e-9);
    }

    #[test]
    fn test_with_holding_penalty() {
        let calc = RewardCalculator::new(0.5, 1.0, 0.01, 0.005);
        let r = calc.calculate(0.0, 1.0, 1.0, 0.0, false, true);
        // r = 0 + 0 - 0 - 0 - 0.005 = -0.005
        assert!((r - (-0.005)).abs() < 1e-9);
    }

    #[test]
    fn test_sharpe_decline() {
        let calc = RewardCalculator::new(1.0, 1.0, 0.01, 0.005);
        let r = calc.calculate(0.001, 1.2, 0.9, 0.0, false, false);
        // r = 0.001 + (-0.3) - 0 - 0 - 0 = -0.299
        assert!((r - (-0.299)).abs() < 1e-9);
    }
}
