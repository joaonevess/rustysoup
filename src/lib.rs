mod attrs;
mod dom;
mod errors;
mod matcher;
mod parser;
mod python;
mod search;
mod selectors;
mod shared;
mod soup;
mod string;
mod tag;

use pyo3::prelude::*;

#[pyfunction]
fn _new_text_node(value: &str) -> string::NavigableString {
    let (document, id) = dom::Document::detached_text(value.to_string());
    string::NavigableString::new(shared::shared_document(document), id)
}

#[pyfunction]
fn _new_comment_node(value: &str) -> string::NavigableString {
    let (document, id) = dom::Document::detached_comment(value.to_string());
    string::NavigableString::new(shared::shared_document(document), id)
}

#[pyfunction]
fn _new_cdata_node(value: &str) -> string::NavigableString {
    let (document, id) = dom::Document::detached_cdata(value.to_string());
    string::NavigableString::new(shared::shared_document(document), id)
}

#[pyfunction]
fn _new_declaration_node(value: &str) -> string::NavigableString {
    let (document, id) = dom::Document::detached_declaration(value.to_string());
    string::NavigableString::new(shared::shared_document(document), id)
}

#[pyfunction]
fn _new_doctype_node(value: &str) -> string::NavigableString {
    let (document, id) = dom::Document::detached_doctype(value.to_string());
    string::NavigableString::new(shared::shared_document(document), id)
}

#[pyfunction]
fn _new_processing_instruction_node(value: &str) -> string::NavigableString {
    let (document, id) = dom::Document::detached_processing_instruction(value.to_string());
    string::NavigableString::new(shared::shared_document(document), id)
}

#[pyfunction]
fn _new_template_string_node(value: &str) -> string::NavigableString {
    let (document, id) = dom::Document::detached_template_string(value.to_string());
    string::NavigableString::new(shared::shared_document(document), id)
}

#[pymodule]
fn _rustysoup(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<soup::Soup>()?;
    m.add_class::<tag::AttributeDict>()?;
    m.add_class::<string::NavigableString>()?;
    m.add_class::<tag::Tag>()?;
    m.add_function(wrap_pyfunction!(_new_text_node, m)?)?;
    m.add_function(wrap_pyfunction!(_new_comment_node, m)?)?;
    m.add_function(wrap_pyfunction!(_new_cdata_node, m)?)?;
    m.add_function(wrap_pyfunction!(_new_declaration_node, m)?)?;
    m.add_function(wrap_pyfunction!(_new_doctype_node, m)?)?;
    m.add_function(wrap_pyfunction!(_new_processing_instruction_node, m)?)?;
    m.add_function(wrap_pyfunction!(_new_template_string_node, m)?)?;
    Ok(())
}
