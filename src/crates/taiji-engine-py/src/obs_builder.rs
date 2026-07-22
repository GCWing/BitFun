use pyo3::prelude::*;
use pyo3::types::PyList;
use taiji_engine::store::StateStore;
use taiji_engine::types::state::StateValue;

/// 从 StateStore 构造观测向量的构建器。
///
/// `feature_list` 中的每一项都是一个 StateKey，ObsBuilder 按顺序从 StateStore
/// 中提取对应值，组装成固定维度的 f64 观测向量。支持以下值类型：
/// - `F64` → 直接取值
/// - `Bool` → true→1.0, false→0.0
/// - `Usize` → 转为 f64
/// - `Bars` → 取最新 Bar 的 close/vol/oi/delta（自动展开为 4 维）
/// - `Json` → 尝试解析为 f64
#[pyclass]
#[derive(Clone)]
pub struct ObsBuilder {
    #[pyo3(get)]
    pub feature_list: Vec<String>,
}

#[pymethods]
impl ObsBuilder {
    #[new]
    fn new(feature_list: Vec<String>) -> Self {
        Self { feature_list }
    }

    fn __repr__(&self) -> String {
        format!(
            "ObsBuilder(feature_dim={}, features={:?})",
            self.feature_dim(),
            self.feature_list
        )
    }
}

impl ObsBuilder {
    /// 从 StateStore 构造观测向量，返回 Python list[float]。
    pub fn build(&self, state: &StateStore) -> PyResult<PyObject> {
        Python::with_gil(|py| {
            let mut values: Vec<f64> = Vec::new();
            for key in &self.feature_list {
                match state.get_raw(key) {
                    Some(StateValue::F64(v)) => values.push(v),
                    Some(StateValue::Bool(b)) => values.push(if b { 1.0 } else { 0.0 }),
                    Some(StateValue::Usize(u)) => values.push(u as f64),
                    Some(StateValue::Bars(bars)) => {
                        if let Some(last) = bars.last() {
                            values.push(last.close);
                            values.push(last.vol);
                            values.push(last.open_interest.unwrap_or(0.0));
                            values.push(last.delta.unwrap_or(0.0));
                        } else {
                            values.extend(&[0.0_f64; 4]);
                        }
                    }
                    Some(StateValue::Json(json)) => {
                        if let Some(n) = json.as_f64() {
                            values.push(n);
                        } else if let Some(n) = json.as_i64() {
                            values.push(n as f64);
                        } else {
                            values.push(0.0);
                        }
                    }
                    _ => values.push(0.0),
                }
            }

            let list = PyList::new(py, values)?;
            Ok(list.into())
        })
    }

    /// 返回观测向量的维度（Bars 类型 feature 展开为 4 维）。
    pub fn feature_dim(&self) -> usize {
        self.feature_list
            .iter()
            .map(|key| if key.starts_with("bars") { 4 } else { 1 })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::sync::Arc;
    use std::sync::Once;
    use taiji_engine::types::bar::RawBar;

    static PYTHON_INIT: Once = Once::new();

    fn init_python() {
        PYTHON_INIT.call_once(|| {
            pyo3::prepare_freethreaded_python();
        });
    }

    fn make_bar(close: f64, vol: f64, oi: Option<f64>, delta: Option<f64>) -> RawBar {
        RawBar {
            symbol: "test".into(),
            dt: Utc::now(),
            freq: taiji_engine::types::bar::Freq::F1,
            id: 0,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            close,
            vol,
            amount: 0.0,
            open_interest: oi,
            delta,
        }
    }

    #[test]
    fn test_feature_dim_bars_expansion() {
        let builder = ObsBuilder::new(vec![
            "bars:1m".into(),
            "volume_ratio".into(),
            "signal_confidence".into(),
        ]);
        // bars:1m → 4, volume_ratio → 1, signal_confidence → 1 = 6
        assert_eq!(builder.feature_dim(), 6);
    }

    #[test]
    fn test_feature_dim_greater_than_10() {
        let builder = ObsBuilder::new(vec![
            "bars:1m".into(),
            "bars:5m".into(),
            "volume_ratio".into(),
            "oi_delta".into(),
            "active_trade_diff".into(),
            "net_long".into(),
            "net_short".into(),
            "trend_strength".into(),
        ]);
        // 4 + 4 + 6 = 14 > 10
        assert!(builder.feature_dim() > 10);
    }

    #[test]
    fn test_build_f64_values() {
        init_python();
        let store = StateStore::new();
        store.set("price".into(), StateValue::F64(100.5), "n1".into());
        store.set("volume_ratio".into(), StateValue::F64(0.8), "n1".into());
        store.set("signal".into(), StateValue::Bool(true), "n1".into());

        let builder = ObsBuilder::new(vec!["price".into(), "volume_ratio".into(), "signal".into()]);

        let obs = builder.build(&store).unwrap();
        Python::with_gil(|py| {
            let list = obs.bind(py).downcast::<PyList>().unwrap();
            assert_eq!(list.len(), 3);
            let v0: f64 = list.get_item(0).unwrap().extract().unwrap();
            let v1: f64 = list.get_item(1).unwrap().extract().unwrap();
            let v2: f64 = list.get_item(2).unwrap().extract().unwrap();
            assert!((v0 - 100.5).abs() < 1e-9);
            assert!((v1 - 0.8).abs() < 1e-9);
            assert!((v2 - 1.0).abs() < 1e-9);
        });
    }

    #[test]
    fn test_build_bars_expansion() {
        init_python();
        let store = StateStore::new();
        let bar = make_bar(4000.0, 1000.0, Some(50000.0), Some(200.0));
        store.set(
            "bars:1m".into(),
            StateValue::Bars(Arc::new(vec![Arc::new(bar)])),
            "bar_gen".into(),
        );

        let builder = ObsBuilder::new(vec!["bars:1m".into()]);
        let obs = builder.build(&store).unwrap();
        Python::with_gil(|py| {
            let list = obs.bind(py).downcast::<PyList>().unwrap();
            assert_eq!(list.len(), 4);
            let close: f64 = list.get_item(0).unwrap().extract().unwrap();
            let vol: f64 = list.get_item(1).unwrap().extract().unwrap();
            let oi: f64 = list.get_item(2).unwrap().extract().unwrap();
            let delta: f64 = list.get_item(3).unwrap().extract().unwrap();
            assert!((close - 4000.0).abs() < 1e-9);
            assert!((vol - 1000.0).abs() < 1e-9);
            assert!((oi - 50000.0).abs() < 1e-9);
            assert!((delta - 200.0).abs() < 1e-9);
        });
    }
}
