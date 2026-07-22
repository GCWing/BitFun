use pyo3::prelude::*;

pub mod cache;
pub mod obs_builder;
pub mod python;
pub mod reward_calculator;
pub mod rl_env;

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", "0.1.0")?;
    m.add_class::<python::types_py::TickDataPy>()?;
    m.add_class::<python::types_py::RawBarPy>()?;
    m.add_class::<python::types_py::SignalPy>()?;
    m.add_class::<python::engine_py::PipelinePy>()?;
    m.add_class::<obs_builder::ObsBuilder>()?;
    m.add_class::<reward_calculator::RewardCalculator>()?;
    m.add_class::<rl_env::TaijiRLEnv>()?;
    Ok(())
}
