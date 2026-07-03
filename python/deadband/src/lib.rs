use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyList;

use deadband_core::{
    Orchestrator as CoreOrchestrator, ToolCallEvent, Intervention as CoreIntervention,
    RecoveryMetrics,
};

#[pyclass(name = "Orchestrator")]
struct PyOrchestrator {
    inner: CoreOrchestrator,
}

#[pymethods]
impl PyOrchestrator {
    #[new]
    fn new(policy_yaml: &str) -> PyResult<Self> {
        let inner = CoreOrchestrator::from_yaml(policy_yaml)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to init orchestrator: {}", e)))?;
        Ok(Self { inner })
    }

    fn process(
        &mut self,
        thread_id: &str,
        step: u64,
        tool_name: &str,
        arguments: &str,
    ) -> PyResult<(Option<PyIntervention>, Option<PyDetectionReport>)> {
        let args: serde_json::Value =
            serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
        let event = ToolCallEvent::started(thread_id, step, tool_name, args);

        let (intervention, report) =
            self.inner.process(event, &deadband_core::AdapterCapabilities::default());
        Ok((
            intervention.map(|i| PyIntervention { inner: i }),
            report.map(|r| PyDetectionReport { inner: r }),
        ))
    }

    fn record_intervention_outcome(
        &mut self,
        report: &PyDetectionReport,
        outcome: &str,
    ) {
        let out = match outcome {
            "applied" => deadband_core::InterventionOutcome::Applied,
            "unsupported" => deadband_core::InterventionOutcome::Unsupported,
            "failed" => deadband_core::InterventionOutcome::Failed,
            _ => deadband_core::InterventionOutcome::Failed,
        };
        self.inner.record_intervention_outcome(&report.inner, out, None);
    }

    fn get_metrics(&self) -> PyRecoveryMetrics {
        PyRecoveryMetrics {
            inner: self.inner.metrics().clone(),
        }
    }
}

#[pyclass(name = "Intervention")]
#[derive(Clone)]
struct PyIntervention {
    inner: CoreIntervention,
}

#[pymethods]
impl PyIntervention {
    fn is_continue(&self) -> bool {
        self.inner.is_continue()
    }

    fn is_abort(&self) -> bool {
        self.inner.is_abort()
    }

    fn is_retry(&self) -> bool {
        self.inner.is_retry()
    }

    fn is_replace_tool(&self) -> bool {
        self.inner.is_replace_tool()
    }

    fn is_inject_prompt(&self) -> bool {
        self.inner.is_inject_prompt()
    }

    fn reason(&self) -> Option<String> {
        self.inner.reason().map(|s| s.to_string())
    }

    fn delay_ms(&self) -> Option<u64> {
        self.inner.delay_ms()
    }

    fn prompt_content(&self) -> Option<String> {
        self.inner.prompt_content().map(|s| s.to_string())
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }
}

#[pyclass(name = "RecoveryMetrics")]
#[derive(Clone)]
struct PyRecoveryMetrics {
    inner: RecoveryMetrics,
}

#[pymethods]
impl PyRecoveryMetrics {
    fn intervention_count(&self) -> u64 {
        self.inner.intervention_count
    }

    fn prevented_calls(&self) -> u64 {
        self.inner.prevented_calls
    }

    fn recovery_time_ms(&self) -> u64 {
        self.inner.recovery_time_ms
    }

    fn to_json(&self) -> String {
        self.inner.to_json()
    }

    fn __repr__(&self) -> String {
        format!(
            "RecoveryMetrics(count={}, prevented={})",
            self.inner.intervention_count, self.inner.prevented_calls
        )
    }
}

#[pyclass(name = "DetectionReport")]
#[derive(Clone)]
struct PyDetectionReport {
    inner: deadband_core::DetectionReport,
}

#[pyfunction]
fn canonicalize_args(args_json: &str, volatile_fields: &Bound<'_, PyList>) -> String {
    let fields: Vec<String> = volatile_fields
        .iter()
        .filter_map(|f| unsafe { f.extract::<String>().ok() })
        .collect();
    deadband_core::canonicalize_args(args_json, &fields)
}

#[pymodule]
fn deadband(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyOrchestrator>()?;
    m.add_class::<PyIntervention>()?;
    m.add_class::<PyRecoveryMetrics>()?;
    m.add_class::<PyDetectionReport>()?;
    m.add_function(wrap_pyfunction!(canonicalize_args, m)?)?;
    Ok(())
}
