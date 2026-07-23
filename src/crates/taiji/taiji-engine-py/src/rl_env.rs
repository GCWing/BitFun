use crate::obs_builder::ObsBuilder;
use crate::python::engine_py::PipelinePy;
use crate::reward_calculator::RewardCalculator;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// 将 f64 向量转为 numpy ndarray（通过 Python GIL 调用 numpy.array）。
fn vec_to_numpy(py: Python<'_>, data: &[f64]) -> PyResult<PyObject> {
    let np = py.import("numpy")?;
    let arr = np.call_method1("array", (data.to_vec(),))?;
    Ok(arr.into())
}

/// TaijiRLEnv — 面向 taiji-engine Pipeline 的 Gym 强化学习环境。
///
/// # 动作空间
/// `Discrete(3)`:
/// - 0 → Long（做多）
/// - 1 → Neutral（空仓）
/// - 2 → Short（做空）
///
/// # 观测空间
/// `Box(low=-inf, high=inf, shape=(feature_dim,), dtype=float32)`
/// 由 ObsBuilder 从 StateStore 提取。
///
/// # 奖励函数
/// `r = log_return + α·diff_sharpe - β·dd_penalty - γ·cost - δ·holding`
/// 由 RewardCalculator 计算。
#[pyclass]
pub struct TaijiRLEnv {
    /// Python 端 Pipeline 包装（通过 GIL 访问）
    pipeline: Py<PipelinePy>,

    /// 观测构建器
    obs_builder: ObsBuilder,

    /// 奖励计算器
    reward_calc: RewardCalculator,

    /// 当前步数
    current_step: usize,

    /// 最大步数（episode 终止条件）
    total_steps: usize,

    /// 当前持仓列表
    positions: Vec<taiji_engine::risk::RiskPosition>,

    /// 初始资金
    initial_capital: f64,

    /// 当前资金
    capital: f64,

    /// 峰值资金（用于计算回撤）
    peak_capital: f64,

    /// 历史对数收益率（用于计算 Sharpe 近似）
    return_history: Vec<f64>,

    /// 上一步价格（用于计算 log_return）
    prev_price: f64,
}

#[pymethods]
impl TaijiRLEnv {
    #[new]
    #[pyo3(signature = (pipeline, obs_builder, reward_calc, total_steps=1000, initial_capital=1_000_000.0))]
    fn new(
        pipeline: Py<PipelinePy>,
        obs_builder: Py<ObsBuilder>,
        reward_calc: Py<RewardCalculator>,
        total_steps: usize,
        initial_capital: f64,
    ) -> PyResult<Self> {
        let (obs, rc) = Python::with_gil(|py| {
            let obs: ObsBuilder = obs_builder.extract(py)?;
            let rc: RewardCalculator = reward_calc.extract(py)?;
            Ok::<_, PyErr>((obs, rc))
        })?;

        Ok(Self {
            pipeline,
            obs_builder: obs,
            reward_calc: rc,
            current_step: 0,
            total_steps,
            positions: Vec::new(),
            initial_capital,
            capital: initial_capital,
            peak_capital: initial_capital,
            return_history: Vec::new(),
            prev_price: 0.0,
        })
    }

    /// 重置环境到初始状态，返回初始观测（numpy ndarray）。
    fn reset(&mut self) -> PyResult<PyObject> {
        self.current_step = 0;
        self.capital = self.initial_capital;
        self.peak_capital = self.initial_capital;
        self.return_history.clear();
        self.positions.clear();
        self.prev_price = 0.0;

        let obs_values = Python::with_gil(|py| {
            let pipeline = self.pipeline.borrow(py);
            let store = pipeline.state_store().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("Pipeline not initialized")
            })?;
            self.obs_builder.build(store)
        })?;

        Python::with_gil(|py| {
            let list: Vec<f64> = obs_values.extract(py)?;
            vec_to_numpy(py, &list)
        })
    }

    /// 执行一步交易动作，返回 `(observation, reward, done, info)`。
    ///
    /// # 参数
    /// - `action`: 0=Long, 1=Neutral, 2=Short
    fn step(&mut self, action: usize) -> PyResult<(PyObject, f64, bool, PyObject)> {
        self.current_step += 1;

        // 1. 从 Pipeline 获取当前价格并计算 log_return
        let (current_price, log_return) = Python::with_gil(|py| {
            let pipeline = self.pipeline.borrow(py);
            let store = pipeline.state_store().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("Pipeline not initialized")
            })?;

            let price = store
                .get_raw(&"last_price".into())
                .and_then(|v| match v {
                    taiji_engine::types::state::StateValue::F64(p) => Some(p),
                    _ => None,
                })
                .unwrap_or(0.0);

            let lr = if self.prev_price > 0.0 && price > 0.0 {
                (price / self.prev_price).ln()
            } else {
                0.0
            };

            Ok::<_, PyErr>((price, lr))
        })?;

        // 2. 判断是否交易 & 是否持仓
        let traded = action != 1; // neutral = no trade
        let is_holding = !self.positions.is_empty();

        // 3. 更新持仓
        if traded {
            match action {
                0 => {
                    self.positions.push(taiji_engine::risk::RiskPosition {
                        instrument: "default".into(),
                        volume: 1.0,
                        avg_price: current_price,
                    });
                }
                2 => {
                    self.positions.clear();
                    self.positions.push(taiji_engine::risk::RiskPosition {
                        instrument: "default".into(),
                        volume: -1.0,
                        avg_price: current_price,
                    });
                }
                _ => {}
            }
        }

        // 4. 更新资金
        if !self.positions.is_empty() && self.prev_price > 0.0 {
            let total_volume: f64 = self.positions.iter().map(|p| p.volume).sum();
            let pnl = total_volume * (current_price - self.prev_price);
            self.capital += pnl;
        }
        if self.capital > self.peak_capital {
            self.peak_capital = self.capital;
        }
        self.prev_price = current_price;

        // 5. 记录收益率
        self.return_history.push(log_return);

        // 6. 计算当前 Sharpe（滚动窗口近似）
        let window = 20usize.min(self.return_history.len());
        let recent = recent_window(&self.return_history, window);
        let (curr_sharpe, _) = sharpe_ratio(&recent);

        let prev_sharpe = if self.return_history.len() > window + 1 {
            let prev_recent = recent_window(
                &self.return_history[..self.return_history.len() - 1],
                window,
            );
            sharpe_ratio(&prev_recent).0
        } else {
            0.0
        };

        // 7. 计算回撤
        let drawdown_pct = if self.peak_capital > 0.0 {
            (self.peak_capital - self.capital) / self.peak_capital
        } else {
            0.0
        };

        // 8. 计算奖励
        let reward = self.reward_calc.calculate(
            log_return,
            prev_sharpe,
            curr_sharpe,
            drawdown_pct,
            traded,
            is_holding,
        );

        // 9. 是否终止
        let done = self.current_step >= self.total_steps;

        // 10. 构建观测
        let obs = Python::with_gil(|py| {
            let pipeline = self.pipeline.borrow(py);
            let store = pipeline.state_store().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("Pipeline not initialized")
            })?;
            let values = self.obs_builder.build(store)?;
            let list: Vec<f64> = values.extract(py)?;
            vec_to_numpy(py, &list)
        })?;

        // 11. info dict
        let info = Python::with_gil(|py| {
            let dict = PyDict::new(py);
            dict.set_item("step", self.current_step)?;
            dict.set_item("capital", self.capital)?;
            dict.set_item("peak_capital", self.peak_capital)?;
            dict.set_item("drawdown_pct", drawdown_pct)?;
            dict.set_item("sharpe", curr_sharpe)?;
            dict.set_item("log_return", log_return)?;
            dict.set_item("positions_count", self.positions.len())?;
            Ok::<_, PyErr>(dict.into())
        })?;

        Ok((obs, reward, done, info))
    }

    /// 可视化（占位实现）。
    fn render(&self) -> PyResult<()> {
        Python::with_gil(|py| {
            let pipeline = self.pipeline.borrow(py);
            let status = pipeline.status()?;
            println!(
                "[TaijiRLEnv] step={}/{} capital={:.2} positions={}\n  pipeline: {}",
                self.current_step,
                self.total_steps,
                self.capital,
                self.positions.len(),
                status,
            );
            Ok(())
        })
    }

    /// 关闭环境，释放资源。
    fn close(&self) -> PyResult<()> {
        Ok(())
    }

    // ── 属性 ──

    /// 动作空间：Discrete(3)
    #[getter]
    fn action_space(&self) -> PyResult<PyObject> {
        Python::with_gil(|py| {
            let gym_spaces = py.import("gymnasium.spaces")?;
            let discrete = gym_spaces.call_method1("Discrete", (3u32,))?;
            Ok(discrete.into())
        })
    }

    /// 观测空间：Box(low=-inf, high=inf, shape=(feature_dim,), dtype=float32)
    #[getter]
    fn observation_space(&self) -> PyResult<PyObject> {
        Python::with_gil(|py| {
            let dim = self.obs_builder.feature_dim();
            let gym_spaces = py.import("gymnasium.spaces")?;
            let np = py.import("numpy")?;
            // np.inf (float32)
            let inf = np
                .getattr("float32")?
                .call_method1("__call__", (np.getattr("inf")?,))?;
            // -np.inf
            let neg_inf: PyObject = inf.call_method0("__neg__")?.into();
            let low = np.call_method1("full", ((dim,), &neg_inf))?;
            let high = np.call_method1("full", ((dim,), &inf))?;
            let box_space = gym_spaces.call_method1("Box", (low, high))?;
            Ok(box_space.into())
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "TaijiRLEnv(step={}/{}, capital={:.2}, positions={}, obs_dim={})",
            self.current_step,
            self.total_steps,
            self.capital,
            self.positions.len(),
            self.obs_builder.feature_dim(),
        )
    }
}

// ── 内部辅助函数 ──

/// 从收益率历史中提取最近 `window` 个元素。
fn recent_window(history: &[f64], window: usize) -> Vec<f64> {
    if history.len() <= window {
        history.to_vec()
    } else {
        history[history.len() - window..].to_vec()
    }
}

/// 计算 Sharpe ratio（均值 / 标准差）。n <= 1 时返回 (0.0, 0.0)。
fn sharpe_ratio(returns: &[f64]) -> (f64, f64) {
    let n = returns.len() as f64;
    if n <= 1.0 {
        return (0.0, 0.0);
    }
    let mean = returns.iter().sum::<f64>() / n;
    let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
    if variance <= 0.0 {
        (0.0, mean)
    } else {
        (mean / variance.sqrt(), mean)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sharpe_ratio_empty() {
        let (s, m) = sharpe_ratio(&[]);
        assert!((s - 0.0).abs() < 1e-9);
        assert!((m - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_sharpe_ratio_single() {
        let (s, _) = sharpe_ratio(&[0.01]);
        assert!((s - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_sharpe_ratio_constant() {
        let (s, m) = sharpe_ratio(&[0.01, 0.01, 0.01, 0.01]);
        assert!((s - 0.0).abs() < 1e-9);
        assert!((m - 0.01).abs() < 1e-9);
    }

    #[test]
    fn test_sharpe_ratio_varying() {
        let returns = vec![0.01, 0.02, -0.01, 0.03, 0.00];
        let (s, _) = sharpe_ratio(&returns);
        let mean = 0.01f64;
        let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / 4.0;
        let expected = mean / var.sqrt();
        assert!((s - expected).abs() < 1e-9);
    }

    #[test]
    fn test_recent_window_full() {
        let history = vec![1.0, 2.0, 3.0];
        let w = recent_window(&history, 5);
        assert_eq!(w, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_recent_window_partial() {
        let history = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let w = recent_window(&history, 3);
        assert_eq!(w, vec![3.0, 4.0, 5.0]);
    }
}
