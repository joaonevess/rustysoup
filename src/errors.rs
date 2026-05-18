use pyo3::PyErr;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use std::any::Any;

pub fn invalid_selector(selector: &str) -> PyErr {
    PyValueError::new_err(format!("Invalid CSS selector: {selector:?}"))
}

pub fn internal_panic(context: &str, payload: Box<dyn Any + Send>) -> PyErr {
    let message = if let Some(value) = payload.downcast_ref::<&'static str>() {
        *value
    } else if let Some(value) = payload.downcast_ref::<String>() {
        value.as_str()
    } else {
        "unknown panic payload"
    };
    PyRuntimeError::new_err(format!(
        "rustysoup internal error while {context}: {message}"
    ))
}
