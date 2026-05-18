use crate::dom::{NodeId, NodeType};
use crate::python::node_to_py;
use crate::shared::{SharedDocument, read_document, write_document};
use crate::soup::find_all_compat_in_nodes;
use crate::tag::{
    Tag, extract_rustysoup_string, insert_string_after, insert_string_before, insert_tag_after,
    insert_tag_before,
};
use pyo3::IntoPyObjectExt;
use pyo3::basic::CompareOp;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::sync::Arc;

#[pyclass(
    name = "_NavigableString",
    module = "rustysoup",
    skip_from_py_object,
    freelist = 2048
)]
#[derive(Clone)]
pub struct NavigableString {
    pub(crate) document: SharedDocument,
    pub(crate) id: NodeId,
}

impl NavigableString {
    pub(crate) fn new(document: SharedDocument, id: NodeId) -> Self {
        Self { document, id }
    }

    fn value(&self) -> String {
        read_document(&self.document)
            .node_string(self.id)
            .unwrap_or_default()
            .to_string()
    }
}

#[pymethods]
impl NavigableString {
    #[getter]
    fn name(&self) -> Option<String> {
        None
    }

    #[getter]
    fn hidden(&self) -> bool {
        false
    }

    #[getter]
    fn decomposed(&self) -> bool {
        false
    }

    #[getter]
    fn known_xml(&self) -> Option<bool> {
        None
    }

    #[getter]
    fn parent(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let id = read_document(&self.document).parent_node(self.id);
        id.map(|id| node_to_py(py, &self.document, id)).transpose()
    }

    #[getter]
    fn parents(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let nodes = read_document(&self.document).parent_nodes(self.id);
        nodes
            .into_iter()
            .map(|id| node_to_py(py, &self.document, id))
            .collect()
    }

    #[pyo3(name = "parentGenerator")]
    fn parent_generator(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.parents(py)
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
        nodes
            .into_iter()
            .map(|id| node_to_py(py, &self.document, id))
            .collect()
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
        nodes
            .into_iter()
            .map(|id| node_to_py(py, &self.document, id))
            .collect()
    }

    #[pyo3(name = "previousGenerator")]
    fn previous_generator(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.previous_elements(py)
    }

    #[getter]
    fn self_and_next_elements(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let mut nodes = Vec::new();
        nodes.push(self.id);
        nodes.extend(read_document(&self.document).next_element_nodes(self.id));
        nodes
            .into_iter()
            .map(|id| node_to_py(py, &self.document, id))
            .collect()
    }

    #[getter]
    fn self_and_previous_elements(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let mut nodes = Vec::new();
        nodes.push(self.id);
        nodes.extend(read_document(&self.document).previous_element_nodes(self.id));
        nodes
            .into_iter()
            .map(|id| node_to_py(py, &self.document, id))
            .collect()
    }

    #[getter]
    fn self_and_parents(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let mut nodes = Vec::new();
        nodes.push(self.id);
        nodes.extend(read_document(&self.document).parent_nodes(self.id));
        nodes
            .into_iter()
            .map(|id| node_to_py(py, &self.document, id))
            .collect()
    }

    #[getter]
    fn next_sibling(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let id = read_document(&self.document).node(self.id).next_sibling;
        id.map(|id| node_to_py(py, &self.document, id)).transpose()
    }

    #[getter]
    fn next_siblings(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let nodes = read_document(&self.document).sibling_nodes_after(self.id);
        nodes
            .into_iter()
            .map(|id| node_to_py(py, &self.document, id))
            .collect()
    }

    #[getter(nextSibling)]
    fn next_sibling_legacy(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.next_sibling(py)
    }

    #[getter]
    fn previous_sibling(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let id = read_document(&self.document).node(self.id).prev_sibling;
        id.map(|id| node_to_py(py, &self.document, id)).transpose()
    }

    #[getter]
    fn previous_siblings(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let nodes = read_document(&self.document).sibling_nodes_before(self.id);
        nodes
            .into_iter()
            .map(|id| node_to_py(py, &self.document, id))
            .collect()
    }

    #[getter(previousSibling)]
    fn previous_sibling_legacy(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.previous_sibling(py)
    }

    #[getter]
    fn self_and_next_siblings(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let mut nodes = Vec::new();
        nodes.push(self.id);
        nodes.extend(read_document(&self.document).sibling_nodes_after(self.id));
        nodes
            .into_iter()
            .map(|id| node_to_py(py, &self.document, id))
            .collect()
    }

    #[getter]
    fn self_and_previous_siblings(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let mut nodes = Vec::new();
        nodes.push(self.id);
        nodes.extend(read_document(&self.document).sibling_nodes_before(self.id));
        nodes
            .into_iter()
            .map(|id| node_to_py(py, &self.document, id))
            .collect()
    }

    #[pyo3(name = "nextSiblingGenerator")]
    fn next_sibling_generator(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.next_siblings(py)
    }

    #[pyo3(name = "previousSiblingGenerator")]
    fn previous_sibling_generator(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.previous_siblings(py)
    }

    fn __str__(&self) -> String {
        self.value()
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.value())
    }

    fn __bool__(&self) -> bool {
        !self.value().is_empty()
    }

    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        let left = self.value();
        let right = if let Ok(other_string) = other.extract::<String>() {
            other_string
        } else {
            return Ok(matches!(op, CompareOp::Ne));
        };

        Ok(match op {
            CompareOp::Eq => left == right,
            CompareOp::Ne => left != right,
            CompareOp::Lt => left < right,
            CompareOp::Le => left <= right,
            CompareOp::Gt => left > right,
            CompareOp::Ge => left >= right,
        })
    }

    #[getter]
    fn is_text(&self) -> bool {
        matches!(
            read_document(&self.document).node(self.id).node_type,
            NodeType::Text(_)
        )
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
        let nodes = read_document(&self.document).next_element_nodes(self.id);
        find_all_compat_in_nodes(
            py,
            &self.document,
            nodes,
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
        let nodes = read_document(&self.document).previous_element_nodes(self.id);
        find_all_compat_in_nodes(
            py,
            &self.document,
            nodes,
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
        let nodes = read_document(&self.document).parent_nodes(self.id);
        find_all_compat_in_nodes(
            py,
            &self.document,
            nodes,
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
        let nodes = read_document(&self.document).sibling_nodes_after(self.id);
        find_all_compat_in_nodes(
            py,
            &self.document,
            nodes,
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
        let nodes = read_document(&self.document).sibling_nodes_before(self.id);
        find_all_compat_in_nodes(
            py,
            &self.document,
            nodes,
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

    fn extract(&self) -> Self {
        write_document(&self.document).detach(self.id);
        self.clone()
    }

    fn decompose(&self) {
        write_document(&self.document).detach(self.id);
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
}
