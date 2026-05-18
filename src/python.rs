use crate::dom::{Document, NodeId, NodeType};
use crate::shared::{SharedDocument, read_document};
use crate::string::NavigableString;
use crate::tag::Tag;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyString};
use std::sync::Arc;

pub(crate) fn node_to_py(
    py: Python<'_>,
    document: &SharedDocument,
    id: NodeId,
) -> PyResult<Py<PyAny>> {
    let node_kind = {
        let document = read_document(document);
        match &document.node(id).node_type {
            NodeType::Element(_) | NodeType::Document => 0,
            NodeType::Text(_) => match document.raw_text_parent_name(id) {
                Some("script") => 5,
                Some("style") => 6,
                _ => 1,
            },
            NodeType::CData(_) => 7,
            NodeType::Declaration(_) => 8,
            NodeType::TemplateString(_) => 9,
            NodeType::Comment(_) => 2,
            NodeType::Doctype(_) => 3,
            NodeType::ProcessingInstruction(_) => 4,
        }
    };

    match node_kind {
        0 => Tag::new(Arc::clone(document), id).into_py_any(py),
        1..=9 => {
            let inner = NavigableString::new(Arc::clone(document), id).into_py_any(py)?;
            let rustysoup = py.import("rustysoup")?;
            let cls = match node_kind {
                2 => rustysoup.getattr("Comment")?,
                3 => rustysoup.getattr("Doctype")?,
                4 => rustysoup.getattr("ProcessingInstruction")?,
                5 => rustysoup.getattr("Script")?,
                6 => rustysoup.getattr("Stylesheet")?,
                7 => rustysoup.getattr("CData")?,
                8 => rustysoup.getattr("Declaration")?,
                9 => rustysoup.getattr("TemplateString")?,
                _ => rustysoup.getattr("NavigableString")?,
            };
            Ok(cls.call1((inner,))?.unbind())
        }
        _ => String::new().into_py_any(py),
    }
}

pub(crate) fn py_encode_string<'py>(
    py: Python<'py>,
    value: &str,
    encoding: &str,
    errors: &str,
) -> PyResult<Bound<'py, PyBytes>> {
    Ok(PyString::new(py, value)
        .call_method1("encode", (encoding, errors))?
        .cast_into::<PyBytes>()?)
}

enum FormatterMode<'py> {
    Escaped,
    Raw,
    Callable(&'py Bound<'py, PyAny>),
}

fn formatter_mode<'py>(formatter: Option<&'py Bound<'py, PyAny>>) -> PyResult<FormatterMode<'py>> {
    let Some(formatter) = formatter else {
        return Ok(FormatterMode::Raw);
    };
    if formatter.is_none() {
        return Ok(FormatterMode::Raw);
    }
    if formatter.is_callable() {
        return Ok(FormatterMode::Callable(formatter));
    }
    if let Ok(name) = formatter.extract::<String>() {
        return match name.as_str() {
            "minimal" | "html" => Ok(FormatterMode::Escaped),
            "html5" => Ok(FormatterMode::Raw),
            _ => Err(PyKeyError::new_err(name)),
        };
    }
    Err(PyKeyError::new_err(formatter.repr()?.to_string()))
}

fn call_formatter(formatter: &Bound<'_, PyAny>, value: &str) -> PyResult<String> {
    Ok(formatter.call1((value,))?.str()?.to_str()?.to_string())
}

pub(crate) fn render_outer_html_with_py_formatter_and_encoding(
    document: &Document,
    id: NodeId,
    formatter: Option<&Bound<'_, PyAny>>,
    eventual_encoding: &str,
) -> PyResult<String> {
    match formatter_mode(formatter)? {
        FormatterMode::Escaped => {
            Ok(document.outer_html_with_encoding_options(id, true, eventual_encoding))
        }
        FormatterMode::Raw => {
            Ok(document.outer_html_with_encoding_options(id, false, eventual_encoding))
        }
        FormatterMode::Callable(formatter) => {
            let mut callback = |value: &str| call_formatter(formatter, value);
            document.outer_html_with_callback_formatter_and_encoding(
                id,
                &mut callback,
                eventual_encoding,
            )
        }
    }
}

pub(crate) fn render_inner_html_with_py_formatter_and_encoding(
    document: &Document,
    id: NodeId,
    formatter: Option<&Bound<'_, PyAny>>,
    eventual_encoding: &str,
) -> PyResult<String> {
    match formatter_mode(formatter)? {
        FormatterMode::Escaped => {
            Ok(document.inner_html_with_encoding_options(id, true, eventual_encoding))
        }
        FormatterMode::Raw => {
            Ok(document.inner_html_with_encoding_options(id, false, eventual_encoding))
        }
        FormatterMode::Callable(formatter) => {
            let mut callback = |value: &str| call_formatter(formatter, value);
            document.inner_html_with_callback_formatter_and_encoding(
                id,
                &mut callback,
                eventual_encoding,
            )
        }
    }
}

pub(crate) fn render_prettify_with_py_formatter(
    document: &Document,
    id: NodeId,
    formatter: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    match formatter_mode(formatter)? {
        FormatterMode::Escaped => Ok(document.prettify_with_options(id, true)),
        FormatterMode::Raw => Ok(document.prettify_with_options(id, false)),
        FormatterMode::Callable(formatter) => {
            let mut callback = |value: &str| call_formatter(formatter, value);
            document.prettify_with_callback_formatter(id, &mut callback)
        }
    }
}
