use crate::dom::{NodeId, NodeType, is_void_element};
use crate::matcher;
use crate::python::{
    node_to_py, py_encode_string, render_inner_html_with_py_formatter_and_encoding,
    render_outer_html_with_py_formatter_and_encoding, render_prettify_with_py_formatter,
};
use crate::shared::{SharedDocument, read_document, write_document};
use crate::soup::{
    DocumentOrderDirection, SiblingDirection, append_nodes_to_py_list, collect_string_nodes,
    collect_string_values, find_all_compat, find_all_compat_document_order_nodes,
    find_all_compat_node_ids, find_all_compat_parent_nodes, find_all_compat_sibling_nodes,
    find_first_compat, nodes_to_py_public, select_all_detached, text_type_selection_from_call,
    try_fast_find_all_into_py_list,
};
use crate::string::NavigableString;
use pyo3::IntoPyObjectExt;
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyKeyError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyDict, PyList, PyString, PyTuple};
use std::sync::Arc;

#[pyclass(module = "rustysoup", skip_from_py_object, freelist = 4096)]
#[derive(Clone)]
pub struct Tag {
    pub(crate) document: SharedDocument,
    pub(crate) id: NodeId,
}

impl Tag {
    pub(crate) fn new(document: SharedDocument, id: NodeId) -> Self {
        Self { document, id }
    }
}

#[pyclass(module = "rustysoup", mapping, skip_from_py_object, freelist = 1024)]
#[derive(Clone)]
pub struct AttributeDict {
    document: SharedDocument,
    id: NodeId,
}

impl AttributeDict {
    pub(crate) fn new(document: SharedDocument, id: NodeId) -> Self {
        Self { document, id }
    }

    fn as_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (name, value) in read_document(&self.document).attrs_map(self.id) {
            dict.set_item(
                name.clone(),
                attr_value_to_py(py, &self.document, self.id, &name, value)?,
            )?;
        }
        Ok(dict)
    }
}

#[pymethods]
impl AttributeDict {
    fn __len__(&self) -> usize {
        read_document(&self.document).attrs_map(self.id).len()
    }

    fn __contains__(&self, key: &str) -> bool {
        read_document(&self.document).attr_present(self.id, key)
    }

    fn __getitem__(&self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        tag_attr_to_py(py, &self.document, self.id, key)
    }

    fn __setitem__(&self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let value = py_attr_value_to_optional_string(value)?;
        write_document(&self.document).set_attr_value(self.id, key.to_string(), value);
        Ok(())
    }

    fn __delitem__(&self, key: &str) -> PyResult<()> {
        if !self.__contains__(key) {
            return Err(PyKeyError::new_err(key.to_string()));
        }
        write_document(&self.document).delete_attr(self.id, key);
        Ok(())
    }

    #[pyo3(signature = (key, default = None))]
    fn get(&self, py: Python<'_>, key: &str, default: Option<Py<PyAny>>) -> PyResult<Py<PyAny>> {
        match tag_attr_to_py(py, &self.document, self.id, key) {
            Ok(value) => Ok(value),
            Err(_) => Ok(default.unwrap_or_else(|| py.None())),
        }
    }

    #[pyo3(signature = (other = None, **kwargs))]
    fn update(
        &self,
        other: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        if let Some(other) = other.filter(|value| !value.is_none()) {
            if let Ok(dict) = other.cast::<PyDict>() {
                for (key, value) in dict.iter() {
                    self.set_item_from_py(&key.extract::<String>()?, &value)?;
                }
            } else {
                for pair in other.try_iter()? {
                    let pair = pair?;
                    let tuple = pair.cast::<PyTuple>().map_err(|_| {
                        PyTypeError::new_err("AttributeDict.update expected key/value pairs")
                    })?;
                    if tuple.len() != 2 {
                        return Err(PyTypeError::new_err(
                            "AttributeDict.update expected key/value pairs",
                        ));
                    }
                    let key = tuple.get_item(0)?.extract::<String>()?;
                    let value = tuple.get_item(1)?;
                    self.set_item_from_py(&key, &value)?;
                }
            }
        }
        if let Some(kwargs) = kwargs {
            for (key, value) in kwargs.iter() {
                self.set_item_from_py(&key.extract::<String>()?, &value)?;
            }
        }
        Ok(())
    }

    #[pyo3(signature = (*args))]
    fn pop(&self, py: Python<'_>, args: &Bound<'_, PyTuple>) -> PyResult<Py<PyAny>> {
        if args.is_empty() || args.len() > 2 {
            return Err(PyTypeError::new_err("pop expected 1 or 2 arguments"));
        }
        let key = args.get_item(0)?.extract::<String>()?;
        match tag_attr_to_py(py, &self.document, self.id, &key) {
            Ok(value) => {
                write_document(&self.document).delete_attr(self.id, &key);
                Ok(value)
            }
            Err(_) if args.len() == 2 => Ok(args.get_item(1)?.unbind()),
            Err(_) => Err(PyKeyError::new_err(key)),
        }
    }

    #[pyo3(signature = (key, default = None))]
    fn setdefault(
        &self,
        py: Python<'_>,
        key: &str,
        default: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        if let Ok(value) = tag_attr_to_py(py, &self.document, self.id, key) {
            return Ok(value);
        }
        let value = default
            .map(py_attr_value_to_optional_string)
            .transpose()?
            .flatten();
        write_document(&self.document).set_attr_value(self.id, key.to_string(), value);
        tag_attr_to_py(py, &self.document, self.id, key)
    }

    fn clear(&self) {
        write_document(&self.document).clear_attrs(self.id);
    }

    fn popitem(&self, py: Python<'_>) -> PyResult<(String, Py<PyAny>)> {
        let popped = {
            let mut document = write_document(&self.document);
            let Some(attr) = document.pop_attr(self.id) else {
                return Err(PyKeyError::new_err("popitem(): dictionary is empty"));
            };
            (
                attr.name().to_string(),
                attr.value.as_ref().map(ToString::to_string),
            )
        };
        let (name, value) = popped;
        Ok((
            name.clone(),
            detached_attr_value_to_py(py, &self.document, self.id, &name, value)?,
        ))
    }

    #[pyo3(signature = (iterable, value = None))]
    fn fromkeys(
        &self,
        py: Python<'_>,
        iterable: &Bound<'_, PyAny>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        for key in iterable.try_iter()? {
            let key = key?;
            match value {
                Some(value) => dict.set_item(key, value)?,
                None => dict.set_item(key, py.None())?,
            }
        }
        Ok(dict.unbind())
    }

    fn keys(&self) -> Vec<String> {
        read_document(&self.document)
            .attrs_map(self.id)
            .into_keys()
            .collect()
    }

    fn values(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        read_document(&self.document)
            .attrs_map(self.id)
            .into_iter()
            .map(|(name, value)| attr_value_to_py(py, &self.document, self.id, &name, value))
            .collect()
    }

    fn items(&self, py: Python<'_>) -> PyResult<Vec<(String, Py<PyAny>)>> {
        read_document(&self.document)
            .attrs_map(self.id)
            .into_iter()
            .map(|(name, value)| {
                let py_value = attr_value_to_py(py, &self.document, self.id, &name, value)?;
                Ok((name, py_value))
            })
            .collect()
    }

    fn copy(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        Ok(self.as_dict(py)?.unbind())
    }

    fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let keys = PyList::new(py, self.keys())?;
        Ok(keys.call_method0("__iter__")?.unbind())
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self.as_dict(py)?.repr()?.to_str()?.to_string())
    }

    fn __richcmp__(
        &self,
        py: Python<'_>,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
    ) -> PyResult<bool> {
        let dict = self.as_dict(py)?;
        match op {
            CompareOp::Eq => dict.as_any().eq(other),
            CompareOp::Ne => dict.as_any().ne(other),
            _ => Err(PyTypeError::new_err(
                "AttributeDict only supports equality comparisons",
            )),
        }
    }
}

impl AttributeDict {
    fn set_item_from_py(&self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let value = py_attr_value_to_optional_string(value)?;
        write_document(&self.document).set_attr_value(self.id, key.to_string(), value);
        Ok(())
    }
}

#[pymethods]
impl Tag {
    #[getter]
    fn name(&self) -> String {
        let document = read_document(&self.document);
        match &document.node(self.id).node_type {
            NodeType::Document => "[document]".to_string(),
            NodeType::Element(element) => element.tag_name().to_string(),
            _ => String::new(),
        }
    }

    #[setter(name)]
    fn set_name(&self, name: &str) {
        write_document(&self.document).set_tag_name(self.id, name.to_string());
    }

    #[getter(text)]
    fn text_prop(&self) -> String {
        read_document(&self.document).text(self.id, "", false)
    }

    #[getter]
    fn hidden(&self) -> bool {
        false
    }

    #[getter]
    fn parser_class(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        py.get_type::<crate::soup::Soup>().into_py_any(py)
    }

    #[getter(parserClass)]
    fn parser_class_legacy(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.parser_class(py)
    }

    #[getter]
    fn namespace(&self) -> Option<String> {
        None
    }

    #[getter]
    fn prefix(&self) -> Option<String> {
        None
    }

    #[getter]
    fn sourceline(&self) -> Option<usize> {
        None
    }

    #[getter]
    fn sourcepos(&self) -> Option<usize> {
        None
    }

    #[getter]
    fn decomposed(&self) -> bool {
        self.name().is_empty()
    }

    #[getter]
    fn known_xml(&self) -> bool {
        false
    }

    #[getter]
    fn can_be_empty_element(&self) -> bool {
        let document = read_document(&self.document);
        document
            .element(self.id)
            .is_some_and(|element| is_void_element(element.tag_name()))
    }

    #[getter]
    fn is_empty_element(&self) -> bool {
        let document = read_document(&self.document);
        document
            .element(self.id)
            .is_some_and(|element| is_void_element(element.tag_name()))
            && document.node(self.id).first_child.is_none()
    }

    #[pyo3(name = "isSelfClosing")]
    fn is_self_closing(&self) -> bool {
        self.is_empty_element()
    }

    fn __len__(&self) -> usize {
        read_document(&self.document).child_count(self.id)
    }

    fn __bool__(&self) -> bool {
        true
    }

    fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let children = PyList::new(py, self.contents(py)?)?;
        Ok(children.call_method0("__iter__")?.unbind())
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        if !matches!(op, CompareOp::Eq | CompareOp::Ne) {
            return Err(PyTypeError::new_err(
                "Tag objects only support equality comparisons",
            ));
        }

        let equal = if let Ok(other) = other.extract::<PyRef<'_, Tag>>() {
            tag_markup(&self.document, self.id) == tag_markup(&other.document, other.id)
        } else {
            false
        };
        Ok(if matches!(op, CompareOp::Eq) {
            equal
        } else {
            !equal
        })
    }

    #[getter]
    fn string(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let id = read_document(&self.document).tag_string_node(self.id);
        id.map(|id| node_to_py(py, &self.document, id)).transpose()
    }

    #[setter(string)]
    fn set_string(&self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let text = value.str()?.to_str()?.to_string();
        let mut document = write_document(&self.document);
        document.clear_children(self.id);
        document.append_text(self.id, text);
        Ok(())
    }

    #[getter]
    fn attrs(&self) -> AttributeDict {
        AttributeDict::new(Arc::clone(&self.document), self.id)
    }

    #[getter]
    fn parent(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let id = read_document(&self.document).parent_node(self.id);
        id.map(|id| node_to_py(py, &self.document, id)).transpose()
    }

    #[getter]
    fn parents(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let nodes = read_document(&self.document).parent_nodes(self.id);
        nodes_to_py_public(py, &self.document, nodes)
    }

    #[pyo3(name = "parentGenerator")]
    fn parent_generator(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.parents(py)
    }

    #[getter]
    fn contents(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let nodes = read_document(&self.document).child_nodes(self.id);
        nodes_to_py_public(py, &self.document, nodes)
    }

    #[getter]
    fn children(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.contents(py)
    }

    #[pyo3(name = "childGenerator")]
    fn child_generator(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.contents(py)
    }

    #[getter]
    fn descendants(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let nodes = read_document(&self.document).descendant_nodes(self.id, false);
        nodes_to_py_public(py, &self.document, nodes)
    }

    #[pyo3(name = "recursiveChildGenerator")]
    fn recursive_child_generator(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.descendants(py)
    }

    #[getter]
    fn next_sibling(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let id = read_document(&self.document).node(self.id).next_sibling;
        id.map(|id| node_to_py(py, &self.document, id)).transpose()
    }

    #[getter]
    fn previous_sibling(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let id = read_document(&self.document).node(self.id).prev_sibling;
        id.map(|id| node_to_py(py, &self.document, id)).transpose()
    }

    #[getter(nextSibling)]
    fn next_sibling_legacy(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.next_sibling(py)
    }

    #[getter(previousSibling)]
    fn previous_sibling_legacy(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.previous_sibling(py)
    }

    #[getter]
    fn next_siblings(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let nodes = read_document(&self.document).sibling_nodes_after(self.id);
        nodes_to_py_public(py, &self.document, nodes)
    }

    #[getter]
    fn previous_siblings(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let nodes = read_document(&self.document).sibling_nodes_before(self.id);
        nodes_to_py_public(py, &self.document, nodes)
    }

    #[getter]
    fn next_element(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let id = read_document(&self.document).next_element_node(self.id);
        id.map(|id| node_to_py(py, &self.document, id)).transpose()
    }

    #[getter(next)]
    fn next_legacy(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.next_element(py)
    }

    #[getter]
    fn next_elements(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let nodes = read_document(&self.document).next_element_nodes(self.id);
        nodes_to_py_public(py, &self.document, nodes)
    }

    #[pyo3(name = "nextGenerator")]
    fn next_generator(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.next_elements(py)
    }

    #[getter]
    fn previous_element(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let id = read_document(&self.document).previous_element_node(self.id);
        id.map(|id| node_to_py(py, &self.document, id)).transpose()
    }

    #[getter(previous)]
    fn previous_legacy(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.previous_element(py)
    }

    #[getter]
    fn previous_elements(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let nodes = read_document(&self.document).previous_element_nodes(self.id);
        nodes_to_py_public(py, &self.document, nodes)
    }

    #[pyo3(name = "previousGenerator")]
    fn previous_generator(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.previous_elements(py)
    }

    #[getter]
    fn strings(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        nodes_to_py_public(
            py,
            &self.document,
            collect_string_nodes(&self.document, self.id),
        )
    }

    #[getter]
    fn stripped_strings(&self) -> Vec<String> {
        collect_strings(&self.document, self.id, true)
    }

    #[pyo3(signature = (separator = "", strip = false, *args, **kwargs))]
    fn get_text(
        &self,
        separator: &str,
        strip: bool,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<String> {
        let mut text_types = text_type_selection_from_call(args, kwargs)?;
        let document = read_document(&self.document);
        if text_types.is_default && document.is_template_element(self.id) {
            text_types.include_template = true;
        }
        Ok(document.text_with_options(
            self.id,
            separator,
            strip,
            text_types.include_text,
            text_types.include_cdata,
            text_types.include_declaration,
            text_types.include_template,
            text_types.include_comments,
            text_types.include_script,
            text_types.include_stylesheet,
            text_types.include_raw_text,
            text_types.include_doctype,
            text_types.include_processing_instruction,
            text_types.include_root_raw_text,
        ))
    }

    #[pyo3(name = "getText", signature = (separator = "", strip = false, *args, **kwargs))]
    fn get_text_legacy(
        &self,
        separator: &str,
        strip: bool,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<String> {
        self.get_text(separator, strip, args, kwargs)
    }

    #[pyo3(signature = (name, default = None))]
    fn get(&self, py: Python<'_>, name: &str, default: Option<Py<PyAny>>) -> PyResult<Py<PyAny>> {
        match tag_attr_to_py(py, &self.document, self.id, name) {
            Ok(value) => Ok(value),
            Err(_) => Ok(default.unwrap_or_else(|| py.None())),
        }
    }

    fn has_attr(&self, name: &str) -> bool {
        read_document(&self.document).attr_present(self.id, name)
    }

    fn has_key(&self, name: &str) -> bool {
        self.has_attr(name)
    }

    fn get_attribute_list(&self, name: &str) -> Vec<String> {
        let Some(value) = read_document(&self.document)
            .attr(self.id, name)
            .map(ToString::to_string)
        else {
            return Vec::new();
        };
        if is_multi_valued_attr(name) {
            value.split_ascii_whitespace().map(String::from).collect()
        } else {
            vec![value]
        }
    }

    fn append(&self, py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(tag) = item.extract::<PyRef<'_, Tag>>() {
            let id = append_tag_to_parent(&self.document, self.id, &tag);
            return node_to_py(py, &self.document, id);
        }
        if let Some(string) = extract_rustysoup_string(item) {
            let id = append_string_to_parent(py, item, &self.document, self.id, &string)?;
            return node_to_py(py, &self.document, id);
        }
        if let Ok(text) = item.extract::<String>() {
            let id = write_document(&self.document).append_text(self.id, text);
            return node_to_py(py, &self.document, id);
        }
        Err(PyTypeError::new_err(
            "rustysoup currently supports appending strings, NavigableString, and Tag objects",
        ))
    }

    fn extend(&self, py: Python<'_>, items: &Bound<'_, PyAny>) -> PyResult<Vec<Py<PyAny>>> {
        let mut out = Vec::new();
        for item in items.try_iter()? {
            let item = item?;
            out.push(self.append(py, &item)?);
        }
        Ok(out)
    }

    fn insert(&self, py: Python<'_>, index: usize, item: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(tag) = item.extract::<PyRef<'_, Tag>>() {
            let id = insert_tag_into_parent(&self.document, self.id, index, &tag);
            return node_to_py(py, &self.document, id);
        }
        if let Some(string) = extract_rustysoup_string(item) {
            let id = insert_string_into_parent(py, item, &self.document, self.id, index, &string)?;
            return node_to_py(py, &self.document, id);
        }
        if let Ok(text) = item.extract::<String>() {
            let id = write_document(&self.document).insert_text(self.id, index, text);
            return node_to_py(py, &self.document, id);
        }
        Err(PyTypeError::new_err(
            "rustysoup currently supports inserting strings and Tag objects",
        ))
    }

    fn index(&self, item: &Bound<'_, PyAny>) -> PyResult<usize> {
        let target = if let Ok(tag) = item.extract::<PyRef<'_, Tag>>() {
            tag.id
        } else if let Some(string) = extract_rustysoup_string(item) {
            string.id
        } else {
            return Err(PyValueError::new_err("Tag.index: element not in tag"));
        };
        read_document(&self.document)
            .child_index(self.id, target)
            .ok_or_else(|| PyValueError::new_err("Tag.index: element not in tag"))
    }

    fn extract(&self) -> Tag {
        write_document(&self.document).detach(self.id);
        self.clone()
    }

    fn decompose(&self) {
        write_document(&self.document).decompose_node(self.id);
    }

    fn clear(&self) {
        write_document(&self.document).clear_children(self.id);
    }

    fn replace_with(&self, py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(tag) = item.extract::<PyRef<'_, Tag>>() {
            let _ = insert_tag_before(&self.document, self.id, &tag);
            write_document(&self.document).detach(self.id);
            return self.clone().into_py_any(py);
        }
        if let Some(string) = extract_rustysoup_string(item) {
            let _ = insert_string_before(py, item, &self.document, self.id, &string)?;
            write_document(&self.document).detach(self.id);
            return self.clone().into_py_any(py);
        }
        if let Ok(text) = item.extract::<String>() {
            write_document(&self.document).insert_text_before(self.id, text);
            write_document(&self.document).detach(self.id);
            return self.clone().into_py_any(py);
        }
        Err(PyTypeError::new_err(
            "rustysoup currently supports replacing with strings and Tag objects",
        ))
    }

    #[pyo3(name = "replaceWith")]
    fn replace_with_legacy(&self, py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        self.replace_with(py, item)
    }

    fn insert_before(&self, py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(tag) = item.extract::<PyRef<'_, Tag>>() {
            let id = insert_tag_before(&self.document, self.id, &tag);
            return node_to_py(py, &self.document, id);
        }
        if let Some(string) = extract_rustysoup_string(item) {
            let id = insert_string_before(py, item, &self.document, self.id, &string)?;
            return node_to_py(py, &self.document, id);
        }
        if let Ok(text) = item.extract::<String>() {
            let id = write_document(&self.document).insert_text_before(self.id, text);
            return node_to_py(py, &self.document, id);
        }
        Err(PyTypeError::new_err(
            "rustysoup currently supports inserting strings and Tag objects",
        ))
    }

    fn insert_after(&self, py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        if let Ok(tag) = item.extract::<PyRef<'_, Tag>>() {
            let id = insert_tag_after(&self.document, self.id, &tag);
            return node_to_py(py, &self.document, id);
        }
        if let Some(string) = extract_rustysoup_string(item) {
            let id = insert_string_after(py, item, &self.document, self.id, &string)?;
            return node_to_py(py, &self.document, id);
        }
        if let Ok(text) = item.extract::<String>() {
            let id = write_document(&self.document).insert_text_after(self.id, text);
            return node_to_py(py, &self.document, id);
        }
        Err(PyTypeError::new_err(
            "rustysoup currently supports inserting strings and Tag objects",
        ))
    }

    fn wrap(&self, wrapper: PyRef<'_, Tag>) -> Tag {
        let id = if Arc::ptr_eq(&self.document, &wrapper.document) {
            write_document(&self.document).insert_before_existing(self.id, wrapper.id);
            write_document(&self.document).append_existing(wrapper.id, self.id);
            wrapper.id
        } else {
            let source = read_document(&wrapper.document);
            write_document(&self.document).wrap_with_clone_from(self.id, &source, wrapper.id)
        };
        Tag::new(Arc::clone(&self.document), id)
    }

    fn unwrap(&self) -> Tag {
        write_document(&self.document).unwrap_node(self.id);
        self.clone()
    }

    fn replace_with_children(&self) -> Tag {
        self.unwrap()
    }

    #[pyo3(name = "replaceWithChildren")]
    fn replace_with_children_legacy(&self) -> Tag {
        self.replace_with_children()
    }

    fn smooth(&self) {
        write_document(&self.document).smooth_text_nodes(self.id);
    }

    #[getter]
    fn self_and_descendants(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let nodes = read_document(&self.document).descendant_nodes(self.id, true);
        nodes_to_py_public(py, &self.document, nodes)
    }

    #[getter]
    fn self_and_next_elements(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let mut nodes = Vec::new();
        nodes.push(self.id);
        nodes.extend(read_document(&self.document).next_element_nodes(self.id));
        nodes_to_py_public(py, &self.document, nodes)
    }

    #[getter]
    fn self_and_previous_elements(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let mut nodes = Vec::new();
        nodes.push(self.id);
        nodes.extend(read_document(&self.document).previous_element_nodes(self.id));
        nodes_to_py_public(py, &self.document, nodes)
    }

    #[getter]
    fn self_and_next_siblings(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let mut nodes = Vec::new();
        nodes.push(self.id);
        nodes.extend(read_document(&self.document).sibling_nodes_after(self.id));
        nodes_to_py_public(py, &self.document, nodes)
    }

    #[getter]
    fn self_and_previous_siblings(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let mut nodes = Vec::new();
        nodes.push(self.id);
        nodes.extend(read_document(&self.document).sibling_nodes_before(self.id));
        nodes_to_py_public(py, &self.document, nodes)
    }

    #[getter]
    fn self_and_parents(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let mut nodes = Vec::new();
        nodes.push(self.id);
        nodes.extend(read_document(&self.document).parent_nodes(self.id));
        nodes_to_py_public(py, &self.document, nodes)
    }

    #[pyo3(name = "nextSiblingGenerator")]
    fn next_sibling_generator(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.next_siblings(py)
    }

    #[pyo3(name = "previousSiblingGenerator")]
    fn previous_sibling_generator(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.previous_siblings(py)
    }

    #[pyo3(signature = (selector, namespaces = None, limit = 0, **kwargs))]
    fn select(
        &self,
        py: Python<'_>,
        selector: &str,
        namespaces: Option<&Bound<'_, PyAny>>,
        limit: usize,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Tag>> {
        let _ = (namespaces, kwargs);
        let ids = select_all_detached(py, &self.document, self.id, false, selector, limit)?;
        Ok(ids
            .into_iter()
            .map(|id| Tag::new(Arc::clone(&self.document), id))
            .collect())
    }

    #[pyo3(signature = (result_set, selector, namespaces = None, limit = 0, **kwargs))]
    fn _select_into_result_set(
        &self,
        py: Python<'_>,
        result_set: &Bound<'_, PyAny>,
        selector: &str,
        namespaces: Option<&Bound<'_, PyAny>>,
        limit: usize,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let _ = (namespaces, kwargs);
        let out = result_set.cast::<PyList>()?;
        let ids = select_all_detached(py, &self.document, self.id, false, selector, limit)?;
        for id in ids {
            out.append(Tag::new(Arc::clone(&self.document), id).into_py_any(py)?)?;
        }
        Ok(())
    }

    #[pyo3(signature = (selector, namespaces = None, **kwargs))]
    fn select_one(
        &self,
        py: Python<'_>,
        selector: &str,
        namespaces: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<Tag>> {
        let _ = (namespaces, kwargs);
        let ids = select_all_detached(py, &self.document, self.id, false, selector, 1)?;
        Ok(ids
            .into_iter()
            .next()
            .map(|id| Tag::new(Arc::clone(&self.document), id)))
    }

    fn _matches_selector(&self, selector: &str) -> PyResult<bool> {
        let document = read_document(&self.document);
        matcher::matches_selector(&document, self.id, selector)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        recursive = true,
        string = None,
        **kwargs
    ))]
    fn find(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        recursive: bool,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<Py<PyAny>>> {
        find_first_compat(
            py,
            &self.document,
            self.id,
            name,
            attrs,
            recursive,
            string,
            kwargs,
        )
    }

    #[pyo3(name = "findChild", signature = (
        name = None,
        attrs = None,
        recursive = true,
        string = None,
        **kwargs
    ))]
    fn find_child_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        recursive: bool,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<Py<PyAny>>> {
        self.find(py, name, attrs, recursive, string, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        recursive = true,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn find_all(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        recursive: bool,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        find_all_compat(
            py,
            &self.document,
            self.id,
            name,
            attrs,
            recursive,
            string,
            limit,
            kwargs,
        )
    }

    #[pyo3(signature = (
        result_set,
        name = None,
        attrs = None,
        recursive = true,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn _find_all_into_result_set(
        &self,
        py: Python<'_>,
        result_set: &Bound<'_, PyAny>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        recursive: bool,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        if try_fast_find_all_into_py_list(
            py,
            &self.document,
            self.id,
            result_set,
            name,
            attrs,
            recursive,
            string,
            limit,
            kwargs,
        )? {
            return Ok(());
        }
        let nodes = find_all_compat_node_ids(
            py,
            &self.document,
            self.id,
            name,
            attrs,
            recursive,
            string,
            limit,
            kwargs,
        )?;
        append_nodes_to_py_list(py, &self.document, nodes, result_set)
    }

    #[pyo3(name = "findChildren", signature = (
        name = None,
        attrs = None,
        recursive = true,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn find_children_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        recursive: bool,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        self.find_all(py, name, attrs, recursive, string, limit, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        recursive = true,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn __call__(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        recursive: bool,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        self.find_all(py, name, attrs, recursive, string, limit, kwargs)
    }

    #[pyo3(name = "findAll", signature = (
        name = None,
        attrs = None,
        recursive = true,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn find_all_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        recursive: bool,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        self.find_all(py, name, attrs, recursive, string, limit, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_next(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<Py<PyAny>>> {
        Ok(self
            .find_all_next(py, name, attrs, string, Some(1), kwargs)?
            .into_iter()
            .next())
    }

    #[pyo3(name = "findNext", signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_next_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<Py<PyAny>>> {
        self.find_next(py, name, attrs, string, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn find_all_next(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        find_all_compat_document_order_nodes(
            py,
            &self.document,
            self.id,
            DocumentOrderDirection::Next,
            name,
            attrs,
            string,
            limit,
            kwargs,
        )
    }

    #[pyo3(name = "findAllNext", signature = (
        name = None,
        attrs = None,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn find_all_next_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        self.find_all_next(py, name, attrs, string, limit, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_previous(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<Py<PyAny>>> {
        Ok(self
            .find_all_previous(py, name, attrs, string, Some(1), kwargs)?
            .into_iter()
            .next())
    }

    #[pyo3(name = "findPrevious", signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_previous_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<Py<PyAny>>> {
        self.find_previous(py, name, attrs, string, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn find_all_previous(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        find_all_compat_document_order_nodes(
            py,
            &self.document,
            self.id,
            DocumentOrderDirection::Previous,
            name,
            attrs,
            string,
            limit,
            kwargs,
        )
    }

    #[pyo3(name = "findAllPrevious", signature = (
        name = None,
        attrs = None,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn find_all_previous_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        self.find_all_previous(py, name, attrs, string, limit, kwargs)
    }

    #[pyo3(name = "fetchAllPrevious", signature = (
        name = None,
        attrs = None,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn fetch_all_previous_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        self.find_all_previous(py, name, attrs, string, limit, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_parent(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<Py<PyAny>>> {
        Ok(self
            .find_parents(py, name, attrs, string, Some(1), kwargs)?
            .into_iter()
            .next())
    }

    #[pyo3(name = "findParent", signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_parent_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<Py<PyAny>>> {
        self.find_parent(py, name, attrs, string, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn find_parents(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        find_all_compat_parent_nodes(
            py,
            &self.document,
            self.id,
            name,
            attrs,
            string,
            limit,
            kwargs,
        )
    }

    #[pyo3(name = "findParents", signature = (
        name = None,
        attrs = None,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn find_parents_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        self.find_parents(py, name, attrs, string, limit, kwargs)
    }

    #[pyo3(name = "fetchParents", signature = (
        name = None,
        attrs = None,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn fetch_parents_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        self.find_parents(py, name, attrs, string, limit, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_next_sibling(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<Py<PyAny>>> {
        Ok(self
            .find_next_siblings(py, name, attrs, string, Some(1), kwargs)?
            .into_iter()
            .next())
    }

    #[pyo3(name = "findNextSibling", signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_next_sibling_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<Py<PyAny>>> {
        self.find_next_sibling(py, name, attrs, string, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn find_next_siblings(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        find_all_compat_sibling_nodes(
            py,
            &self.document,
            self.id,
            SiblingDirection::Next,
            name,
            attrs,
            string,
            limit,
            kwargs,
        )
    }

    #[pyo3(name = "findNextSiblings", signature = (
        name = None,
        attrs = None,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn find_next_siblings_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        self.find_next_siblings(py, name, attrs, string, limit, kwargs)
    }

    #[pyo3(name = "fetchNextSiblings", signature = (
        name = None,
        attrs = None,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn fetch_next_siblings_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        self.find_next_siblings(py, name, attrs, string, limit, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_previous_sibling(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<Py<PyAny>>> {
        Ok(self
            .find_previous_siblings(py, name, attrs, string, Some(1), kwargs)?
            .into_iter()
            .next())
    }

    #[pyo3(name = "findPreviousSibling", signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_previous_sibling_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Option<Py<PyAny>>> {
        self.find_previous_sibling(py, name, attrs, string, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn find_previous_siblings(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        find_all_compat_sibling_nodes(
            py,
            &self.document,
            self.id,
            SiblingDirection::Previous,
            name,
            attrs,
            string,
            limit,
            kwargs,
        )
    }

    #[pyo3(name = "findPreviousSiblings", signature = (
        name = None,
        attrs = None,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn find_previous_siblings_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        self.find_previous_siblings(py, name, attrs, string, limit, kwargs)
    }

    #[pyo3(name = "fetchPreviousSiblings", signature = (
        name = None,
        attrs = None,
        string = None,
        limit = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn fetch_previous_siblings_legacy(
        &self,
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        self.find_previous_siblings(py, name, attrs, string, limit, kwargs)
    }

    #[pyo3(signature = (indent_level = None, eventual_encoding = "utf-8", formatter = None))]
    fn decode_contents(
        &self,
        indent_level: Option<usize>,
        eventual_encoding: &str,
        formatter: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<String> {
        let _ = indent_level;
        let document = read_document(&self.document);
        render_inner_html_with_py_formatter_and_encoding(
            &document,
            self.id,
            formatter,
            eventual_encoding,
        )
    }

    #[pyo3(signature = (indent_level = None, encoding = "utf-8", formatter = None))]
    fn encode_contents<'py>(
        &self,
        py: Python<'py>,
        indent_level: Option<usize>,
        encoding: &str,
        formatter: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let _ = indent_level;
        py_encode_string(
            py,
            &self.decode_contents(None, encoding, formatter)?,
            encoding,
            "strict",
        )
    }

    #[pyo3(name = "renderContents", signature = (encoding = "utf-8", prettyPrint = false, indentLevel = 0))]
    #[allow(non_snake_case)]
    fn render_contents<'py>(
        &self,
        py: Python<'py>,
        encoding: &str,
        prettyPrint: bool,
        indentLevel: usize,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let _ = (prettyPrint, indentLevel);
        let document = read_document(&self.document);
        let markup = document.inner_html_with_encoding_options(self.id, true, encoding);
        py_encode_string(py, &markup, encoding, "strict")
    }

    #[pyo3(signature = (indent_level = None, eventual_encoding = "utf-8", formatter = None, iterator = None))]
    fn decode(
        &self,
        indent_level: Option<usize>,
        eventual_encoding: &str,
        formatter: Option<&Bound<'_, PyAny>>,
        iterator: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<String> {
        let _ = (indent_level, iterator);
        let document = read_document(&self.document);
        render_outer_html_with_py_formatter_and_encoding(
            &document,
            self.id,
            formatter,
            eventual_encoding,
        )
    }

    #[pyo3(signature = (encoding = "utf-8", indent_level = None, formatter = None, errors = "xmlcharrefreplace"))]
    fn encode<'py>(
        &self,
        py: Python<'py>,
        encoding: &str,
        indent_level: Option<usize>,
        formatter: Option<&Bound<'_, PyAny>>,
        errors: &str,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let _ = indent_level;
        py_encode_string(
            py,
            &self.decode(None, encoding, formatter, None)?,
            encoding,
            errors,
        )
    }

    #[pyo3(signature = (encoding = None, formatter = None))]
    fn prettify(
        &self,
        py: Python<'_>,
        encoding: Option<&str>,
        formatter: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let document = read_document(&self.document);
        let pretty = render_prettify_with_py_formatter(&document, self.id, formatter)?;
        if let Some(encoding) = encoding {
            return py_encode_string(py, &pretty, encoding, "xmlcharrefreplace")?.into_py_any(py);
        }
        pretty.into_py_any(py)
    }

    fn __getattr__(&self, name: &str) -> Option<Tag> {
        let document = read_document(&self.document);
        let criteria = crate::matcher::FindCriteria::with_name(Some(name));
        matcher::find_first(&document, self.id, false, &criteria)
            .map(|id| Tag::new(Arc::clone(&self.document), id))
    }

    fn __getitem__(&self, py: Python<'_>, key: &str) -> PyResult<Py<PyAny>> {
        tag_attr_to_py(py, &self.document, self.id, key)
    }

    fn __setitem__(&self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let value = py_attr_value_to_optional_string(value)?;
        write_document(&self.document).set_attr_value(self.id, key.to_string(), value);
        Ok(())
    }

    fn __delitem__(&self, key: &str) {
        write_document(&self.document).delete_attr(self.id, key);
    }

    fn __str__(&self) -> String {
        read_document(&self.document).outer_html(self.id)
    }

    fn __repr__(&self) -> String {
        self.__str__()
    }
}

fn tag_markup(document: &SharedDocument, id: NodeId) -> String {
    read_document(document).outer_html(id)
}

pub(crate) fn append_tag_to_parent(document: &SharedDocument, parent: NodeId, tag: &Tag) -> NodeId {
    if Arc::ptr_eq(document, &tag.document) {
        write_document(document).append_existing(parent, tag.id);
        tag.id
    } else {
        let source = read_document(&tag.document);
        write_document(document).append_clone_from(parent, &source, tag.id)
    }
}

pub(crate) fn insert_tag_into_parent(
    document: &SharedDocument,
    parent: NodeId,
    index: usize,
    tag: &Tag,
) -> NodeId {
    if Arc::ptr_eq(document, &tag.document) {
        write_document(document).insert_existing(parent, index, tag.id);
        tag.id
    } else {
        let source = read_document(&tag.document);
        write_document(document).insert_clone_from(parent, index, &source, tag.id)
    }
}

pub(crate) fn insert_tag_before(document: &SharedDocument, sibling: NodeId, tag: &Tag) -> NodeId {
    if Arc::ptr_eq(document, &tag.document) {
        write_document(document).insert_before_existing(sibling, tag.id);
        tag.id
    } else {
        let source = read_document(&tag.document);
        write_document(document).insert_clone_before_from(sibling, &source, tag.id)
    }
}

pub(crate) fn insert_tag_after(document: &SharedDocument, sibling: NodeId, tag: &Tag) -> NodeId {
    if Arc::ptr_eq(document, &tag.document) {
        write_document(document).insert_after_existing(sibling, tag.id);
        tag.id
    } else {
        let source = read_document(&tag.document);
        write_document(document).insert_clone_after_from(sibling, &source, tag.id)
    }
}

pub(crate) fn extract_rustysoup_string(item: &Bound<'_, PyAny>) -> Option<NavigableString> {
    if let Ok(inner) = item.extract::<PyRef<'_, NavigableString>>() {
        return Some(inner.clone());
    }
    let inner = item.getattr("_inner").ok()?;
    inner
        .extract::<PyRef<'_, NavigableString>>()
        .ok()
        .map(|inner| inner.clone())
}

pub(crate) fn append_string_to_parent(
    py: Python<'_>,
    item: &Bound<'_, PyAny>,
    document: &SharedDocument,
    parent: NodeId,
    string: &NavigableString,
) -> PyResult<NodeId> {
    let id = if Arc::ptr_eq(document, &string.document) {
        write_document(document).append_existing(parent, string.id);
        string.id
    } else {
        let source = read_document(&string.document);
        write_document(document).append_clone_from(parent, &source, string.id)
    };
    update_python_string_inner(py, item, document, id)?;
    Ok(id)
}

pub(crate) fn insert_string_into_parent(
    py: Python<'_>,
    item: &Bound<'_, PyAny>,
    document: &SharedDocument,
    parent: NodeId,
    index: usize,
    string: &NavigableString,
) -> PyResult<NodeId> {
    let id = if Arc::ptr_eq(document, &string.document) {
        write_document(document).insert_existing(parent, index, string.id);
        string.id
    } else {
        let source = read_document(&string.document);
        write_document(document).insert_clone_from(parent, index, &source, string.id)
    };
    update_python_string_inner(py, item, document, id)?;
    Ok(id)
}

pub(crate) fn insert_string_before(
    py: Python<'_>,
    item: &Bound<'_, PyAny>,
    document: &SharedDocument,
    sibling: NodeId,
    string: &NavigableString,
) -> PyResult<NodeId> {
    let id = if Arc::ptr_eq(document, &string.document) {
        write_document(document).insert_before_existing(sibling, string.id);
        string.id
    } else {
        let source = read_document(&string.document);
        write_document(document).insert_clone_before_from(sibling, &source, string.id)
    };
    update_python_string_inner(py, item, document, id)?;
    Ok(id)
}

pub(crate) fn insert_string_after(
    py: Python<'_>,
    item: &Bound<'_, PyAny>,
    document: &SharedDocument,
    sibling: NodeId,
    string: &NavigableString,
) -> PyResult<NodeId> {
    let id = if Arc::ptr_eq(document, &string.document) {
        write_document(document).insert_after_existing(sibling, string.id);
        string.id
    } else {
        let source = read_document(&string.document);
        write_document(document).insert_clone_after_from(sibling, &source, string.id)
    };
    update_python_string_inner(py, item, document, id)?;
    Ok(id)
}

fn update_python_string_inner(
    py: Python<'_>,
    item: &Bound<'_, PyAny>,
    document: &SharedDocument,
    id: NodeId,
) -> PyResult<()> {
    if item.getattr("_inner").is_ok() {
        let inner = NavigableString::new(Arc::clone(document), id).into_py_any(py)?;
        item.setattr("_inner", inner)?;
    }
    Ok(())
}

fn py_attr_value_to_optional_string(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    if let Ok(list) = value.cast::<PyList>() {
        let mut parts = Vec::new();
        for item in list.iter() {
            parts.push(item.str()?.to_str()?.to_string());
        }
        return Ok(Some(parts.join(" ")));
    }
    if let Ok(tuple) = value.cast::<PyTuple>() {
        let mut parts = Vec::new();
        for item in tuple.iter() {
            parts.push(item.str()?.to_str()?.to_string());
        }
        return Ok(Some(parts.join(" ")));
    }
    Ok(Some(value.str()?.to_str()?.to_string()))
}

fn tag_attr_to_py(
    py: Python<'_>,
    document: &SharedDocument,
    id: NodeId,
    key: &str,
) -> PyResult<Py<PyAny>> {
    if !is_multi_valued_attr(key) {
        let document = read_document(document);
        let value = document
            .attr_value(id, key)
            .ok_or_else(|| PyKeyError::new_err(key.to_string()))?;
        return match value {
            Some(value) => PyString::new(py, value).into_py_any(py),
            None => Ok(py.None()),
        };
    }

    let value = read_document(document)
        .attr_value(id, key)
        .map(|value| value.map(ToString::to_string))
        .ok_or_else(|| PyKeyError::new_err(key.to_string()))?;
    attr_value_to_py(py, document, id, key, value)
}

fn attr_value_to_py(
    py: Python<'_>,
    document: &SharedDocument,
    id: NodeId,
    name: &str,
    value: Option<String>,
) -> PyResult<Py<PyAny>> {
    let Some(value) = value else {
        return Ok(py.None());
    };
    if is_multi_valued_attr(name) {
        let values = value
            .split_ascii_whitespace()
            .map(String::from)
            .collect::<Vec<_>>();
        let owner = AttributeDict::new(Arc::clone(document), id).into_py_any(py)?;
        let rustysoup = py.import("rustysoup")?;
        let cls = rustysoup.getattr("AttributeValueList")?;
        Ok(cls.call1((values, owner, name))?.unbind())
    } else {
        value.into_py_any(py)
    }
}

fn detached_attr_value_to_py(
    py: Python<'_>,
    document: &SharedDocument,
    id: NodeId,
    name: &str,
    value: Option<String>,
) -> PyResult<Py<PyAny>> {
    let Some(value) = value else {
        return Ok(py.None());
    };
    if is_multi_valued_attr(name) {
        let values = value
            .split_ascii_whitespace()
            .map(String::from)
            .collect::<Vec<_>>();
        let owner = AttributeDict::new(Arc::clone(document), id).into_py_any(py)?;
        let rustysoup = py.import("rustysoup")?;
        let cls = rustysoup.getattr("AttributeValueList")?;
        Ok(cls.call1((values, owner, py.None()))?.unbind())
    } else {
        value.into_py_any(py)
    }
}

fn is_multi_valued_attr(name: &str) -> bool {
    matches!(
        name,
        "class"
            | "rel"
            | "rev"
            | "accept-charset"
            | "headers"
            | "accesskey"
            | "dropzone"
            | "ping"
            | "sandbox"
    )
}

fn collect_strings(document: &SharedDocument, root: NodeId, strip: bool) -> Vec<String> {
    let document = read_document(document);
    collect_string_values(&document, root, strip)
}
