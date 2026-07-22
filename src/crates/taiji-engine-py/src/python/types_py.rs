use pyo3::prelude::*;

#[pyclass]
#[derive(Clone)]
pub struct TickDataPy {
    #[pyo3(get)]
    pub instrument: String,
    #[pyo3(get)]
    pub last_price: f64,
    #[pyo3(get)]
    pub open_price: f64,
    #[pyo3(get)]
    pub highest_price: f64,
    #[pyo3(get)]
    pub lowest_price: f64,
    #[pyo3(get)]
    pub volume: f64,
    #[pyo3(get)]
    pub open_interest: f64,
    #[pyo3(get)]
    pub timestamp_ms: i64,
}

#[pyclass]
#[derive(Clone)]
pub struct RawBarPy {
    #[pyo3(get)]
    pub symbol: String,
    #[pyo3(get)]
    pub open: f64,
    #[pyo3(get)]
    pub high: f64,
    #[pyo3(get)]
    pub low: f64,
    #[pyo3(get)]
    pub close: f64,
    #[pyo3(get)]
    pub vol: f64,
    #[pyo3(get)]
    pub amount: f64,
    #[pyo3(get)]
    pub open_interest: Option<f64>,
    #[pyo3(get)]
    pub delta: Option<f64>,
}

#[pyclass]
#[derive(Clone)]
pub struct SignalPy {
    #[pyo3(get)]
    pub instrument: String,
    #[pyo3(get)]
    pub action: String,
    #[pyo3(get)]
    pub entry: Option<f64>,
    #[pyo3(get)]
    pub stop_loss: Option<f64>,
    #[pyo3(get)]
    pub take_profit: Option<f64>,
    #[pyo3(get)]
    pub size: Option<f64>,
    #[pyo3(get)]
    pub confidence: f64,
    #[pyo3(get)]
    pub source: String,
}
