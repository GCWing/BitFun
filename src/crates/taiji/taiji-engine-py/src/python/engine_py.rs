use pyo3::prelude::*;
use std::sync::{Arc, Mutex};
use taiji_engine::config::PipelineConfig;
use taiji_engine::pipeline::Pipeline;
use taiji_engine::store::StateStore;

/// Python 端可操作的 Pipeline 包装
#[pyclass]
pub struct PipelinePy {
    inner: Mutex<Option<Pipeline>>,
    /// 缓存的 StateStore Arc（与 inner Pipeline 指向同一实例），
    /// 供 ObsBuilder / TaijiRLEnv 无锁读取。
    state: Mutex<Option<Arc<StateStore>>>,
}

#[pymethods]
impl PipelinePy {
    #[new]
    fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            state: Mutex::new(None),
        }
    }

    /// 从 YAML 字符串加载配置并创建 Pipeline
    // #[allow] — pyo3 method, not a Rust constructor; takes &self for Python ergonomics.
    #[allow(clippy::wrong_self_convention)]
    fn from_yaml(&self, yaml_str: &str) -> PyResult<()> {
        let config: PipelineConfig = serde_yaml::from_str(yaml_str)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        let pipeline = Pipeline::from_config(config)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        // 缓存 StateStore Arc（与 inner Pipeline 共享同一实例）
        let store_arc = pipeline.state_store_arc();
        *self.state.lock().unwrap() = Some(store_arc);
        *self.inner.lock().unwrap() = Some(pipeline);
        Ok(())
    }

    /// 返回 Pipeline 状态信息
    pub fn status(&self) -> PyResult<String> {
        let guard = self.inner.lock().unwrap();
        match guard.as_ref() {
            Some(p) => {
                let s = p.status();
                Ok(format!(
                    "state: {:?}, nodes: {}, ticks: {}, signals: {}",
                    s.state,
                    s.nodes.len(),
                    s.total_ticks,
                    s.total_signals
                ))
            }
            None => Ok("not initialized".into()),
        }
    }

    fn __repr__(&self) -> String {
        match self.status() {
            Ok(s) => format!("PipelinePy({})", s),
            Err(_) => "PipelinePy(error)".into(),
        }
    }
}

impl PipelinePy {
    /// 获取内部 Pipeline 的 StateStore 引用（用于观测构建）。
    /// 返回 None 当 Pipeline 尚未初始化。
    ///
    /// # Safety precondition（调用者契约）
    ///
    /// 返回的 `&StateStore` 引用通过 unsafe 从 `Arc<StateStore>` 裸指针转换而来。
    /// 调用者 **不得** 在引用存活期间调用任何会替换 `self.state` 的方法
    /// （如 `from_yaml`）。违反此契约会导致 use-after-free。
    ///
    /// 在当前代码库中该契约由以下机制保证：
    /// - 所有调用者都持有 Python GIL（`Python::with_gil`），
    ///   使得 `from_yaml`（pyo3 方法）无法在 GIL 帧内并发执行。
    /// - 没有调用者在 `state_store()` 返回后、引用最后一次使用前调用 `from_yaml`。
    /// - `StateStore` 内部全量使用 `DashMap`（interior mutability），
    ///   因此通过 `&StateStore` 的并发读取始终是 data-race-free 的。
    pub fn state_store(&self) -> Option<&StateStore> {
        let guard = self.state.lock().unwrap();
        match guard.as_ref() {
            Some(arc) => {
                let ptr: *const StateStore = Arc::as_ptr(arc);
                // SAFETY:
                // - `Arc::as_ptr` 返回的指针指向 Arc 持有的堆分配。只要该 Arc
                //   的引用计数 > 0，堆分配就保持有效。
                // - 当前函数持有 `self.state` 的 Mutex 锁，在此期间 `self.state`
                //   不会被替换。但锁在函数返回时释放，此后安全性依赖调用者契约：
                //   在返回的 `&StateStore` 引用存活期间不得调用 `from_yaml`。
                // - 所有调用者均在 Python GIL 帧内运行，因此 pyo3 方法 `from_yaml`
                //   不会并发执行。
                // - `StateStore` 内部使用 `DashMap` 提供 interior mutability，
                //   因此即使存在多个 `&StateStore` 引用，读取操作也不会产生数据竞争。
                // - 返回引用的生命周期与 `&self` 绑定。在 pyo3 中 `&self` 来自
                //   `Py<PipelinePy>::borrow(py)`，该 borrow 在 GIL 闭包结束时释放，
                //   因此返回引用无法逃逸出 GIL 帧。
                Some(unsafe { &*ptr })
            }
            None => None,
        }
    }
}
