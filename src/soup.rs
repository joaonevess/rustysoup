use crate::attrs::Attr;
use crate::dom::{Document, NodeId, NodeType};
use crate::errors::internal_panic;
use crate::matcher::{self, FindCriteria};
use crate::parser::{parse_html, parse_html_document};
use crate::python::{
    node_to_py, py_encode_string, render_inner_html_with_py_formatter_and_encoding,
    render_outer_html_with_py_formatter_and_encoding, render_prettify_with_py_formatter,
};
use crate::shared::{SharedDocument, read_document, shared_document};
use crate::tag::{
    AttributeDict, Tag, append_string_to_parent, append_tag_to_parent, extract_rustysoup_string,
    insert_string_into_parent, insert_tag_into_parent,
};
use encoding_rs::Encoding;
use pyo3::IntoPyObjectExt;
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::pybacked::{PyBackedBytes, PyBackedStr};
use pyo3::types::{
    PyAny, PyAnyMethods, PyBytes, PyDict, PyDictMethods, PyList, PySet, PyString, PyTuple,
};
use regex::Regex;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, LazyLock};

#[pyclass(module = "rustysoup")]
pub struct Soup {
    pub(crate) document: SharedDocument,
    original_encoding: Option<String>,
    declared_html_encoding: Option<String>,
    contains_replacement_characters: bool,
}

#[pymethods]
impl Soup {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let constructor_args = ConstructorArgs::parse(args, kwargs)?;
        validate_features(py, constructor_args.features.as_ref())?;
        let parse_mode = parse_mode_for_features(constructor_args.features.as_ref())?;
        let _ = (
            constructor_args.builder.as_ref(),
            constructor_args.element_classes.as_ref(),
        );
        let markup = parse_markup_input(
            constructor_args.markup.as_ref(),
            constructor_args.markup_provided,
            constructor_args.from_encoding.as_ref(),
            constructor_args.exclude_encodings.as_ref(),
        )?;
        let encoding_metadata = markup.encoding_metadata();
        let parse_only = parse_parse_only(constructor_args.parse_only.as_ref())?;
        let document = py
            .detach(move || catch_unwind(AssertUnwindSafe(|| markup.parse(parse_mode))))
            .map_err(|payload| internal_panic("parsing HTML", payload))?;
        let document = if let Some(filter) = &parse_only {
            filter.apply(py, document)?
        } else {
            document
        };
        Ok(Self {
            document: shared_document(document),
            original_encoding: encoding_metadata.original_encoding,
            declared_html_encoding: encoding_metadata.declared_html_encoding,
            contains_replacement_characters: encoding_metadata.contains_replacement_characters,
        })
    }

    #[getter(text)]
    fn text_prop(&self) -> String {
        let document = read_document(&self.document);
        document.text(document.root, "", false)
    }

    #[getter]
    fn name(&self) -> &'static str {
        if read_document(&self.document).root_decomposed {
            ""
        } else {
            "[document]"
        }
    }

    #[getter]
    fn parent(&self) -> Option<Tag> {
        None
    }

    #[getter]
    fn is_xml(&self) -> bool {
        false
    }

    #[getter]
    fn original_encoding(&self) -> Option<String> {
        self.original_encoding.clone()
    }

    #[getter]
    fn declared_html_encoding(&self) -> Option<String> {
        self.declared_html_encoding.clone()
    }

    #[getter]
    fn contains_replacement_characters(&self) -> bool {
        self.contains_replacement_characters
    }

    #[getter]
    fn hidden(&self) -> bool {
        !read_document(&self.document).root_decomposed
    }

    #[getter]
    fn attrs(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let document = read_document(&self.document);
        if document.root_decomposed {
            return Ok(py.None());
        }
        AttributeDict::new(Arc::clone(&self.document), document.root).into_py_any(py)
    }

    #[getter]
    fn can_be_empty_element(&self) -> Option<bool> {
        (!read_document(&self.document).root_decomposed).then_some(false)
    }

    fn __len__(&self) -> usize {
        let document = read_document(&self.document);
        if document.root_decomposed {
            0
        } else {
            document.child_count(document.root)
        }
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
                "BeautifulSoup objects only support equality comparisons",
            ));
        }

        let equal = if let Ok(other) = other.extract::<PyRef<'_, Soup>>() {
            soup_markup(&self.document) == soup_markup(&other.document)
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
    fn is_empty_element(&self) -> bool {
        false
    }

    #[pyo3(name = "isSelfClosing")]
    fn is_self_closing(&self) -> bool {
        false
    }

    #[getter]
    fn parser_class(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        py.get_type::<Self>().into_py_any(py)
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
        read_document(&self.document).root_decomposed
    }

    #[getter]
    fn known_xml(&self) -> bool {
        false
    }

    #[getter]
    fn title(&self) -> Option<Tag> {
        let document = read_document(&self.document);
        let criteria = FindCriteria::with_name(Some("title"));
        matcher::find_first(&document, document.root, false, &criteria)
            .map(|id| Tag::new(Arc::clone(&self.document), id))
    }

    #[getter]
    fn string(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let id = {
            let document = read_document(&self.document);
            document.tag_string_node(document.root)
        };
        id.map(|id| node_to_py(py, &self.document, id)).transpose()
    }

    #[getter]
    fn contents(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let nodes = {
            let document = read_document(&self.document);
            document.child_nodes(document.root)
        };
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
        let nodes = {
            let document = read_document(&self.document);
            document.descendant_nodes(document.root, false)
        };
        nodes_to_py_public(py, &self.document, nodes)
    }

    #[pyo3(name = "recursiveChildGenerator")]
    fn recursive_child_generator(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.descendants(py)
    }

    #[getter]
    fn parents(&self) -> Vec<Py<PyAny>> {
        Vec::new()
    }

    #[pyo3(name = "parentGenerator")]
    fn parent_generator(&self) -> Vec<Py<PyAny>> {
        Vec::new()
    }

    #[getter]
    fn next_element(&self) -> Option<Py<PyAny>> {
        None
    }

    #[getter(next)]
    fn next_legacy(&self) -> Option<Py<PyAny>> {
        None
    }

    #[getter]
    fn previous_element(&self) -> Option<Py<PyAny>> {
        None
    }

    #[getter(previous)]
    fn previous_legacy(&self) -> Option<Py<PyAny>> {
        None
    }

    #[getter]
    fn next_sibling(&self) -> Option<Py<PyAny>> {
        None
    }

    #[getter(nextSibling)]
    fn next_sibling_legacy(&self) -> Option<Py<PyAny>> {
        None
    }

    #[getter]
    fn previous_sibling(&self) -> Option<Py<PyAny>> {
        None
    }

    #[getter(previousSibling)]
    fn previous_sibling_legacy(&self) -> Option<Py<PyAny>> {
        None
    }

    #[getter]
    fn next_elements(&self) -> Vec<Py<PyAny>> {
        Vec::new()
    }

    #[pyo3(name = "nextGenerator")]
    fn next_generator(&self) -> Vec<Py<PyAny>> {
        Vec::new()
    }

    #[getter]
    fn previous_elements(&self) -> Vec<Py<PyAny>> {
        Vec::new()
    }

    #[pyo3(name = "previousGenerator")]
    fn previous_generator(&self) -> Vec<Py<PyAny>> {
        Vec::new()
    }

    #[getter]
    fn next_siblings(&self) -> Vec<Py<PyAny>> {
        Vec::new()
    }

    #[pyo3(name = "nextSiblingGenerator")]
    fn next_sibling_generator(&self) -> Vec<Py<PyAny>> {
        Vec::new()
    }

    #[getter]
    fn previous_siblings(&self) -> Vec<Py<PyAny>> {
        Vec::new()
    }

    #[pyo3(name = "previousSiblingGenerator")]
    fn previous_sibling_generator(&self) -> Vec<Py<PyAny>> {
        Vec::new()
    }

    #[getter]
    fn self_and_descendants(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.descendants(py)
    }

    #[getter]
    fn self_and_next_elements(&self) -> Vec<Py<PyAny>> {
        Vec::new()
    }

    #[getter]
    fn self_and_previous_elements(&self) -> Vec<Py<PyAny>> {
        Vec::new()
    }

    #[getter]
    fn self_and_next_siblings(&self) -> Vec<Py<PyAny>> {
        Vec::new()
    }

    #[getter]
    fn self_and_previous_siblings(&self) -> Vec<Py<PyAny>> {
        Vec::new()
    }

    #[getter]
    fn self_and_parents(&self) -> Vec<Py<PyAny>> {
        Vec::new()
    }

    #[getter]
    fn strings(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let root = read_document(&self.document).root;
        nodes_to_py_public(
            py,
            &self.document,
            collect_string_nodes(&self.document, root),
        )
    }

    #[getter]
    fn stripped_strings(&self) -> Vec<String> {
        let root = read_document(&self.document).root;
        collect_strings(&self.document, root, true)
    }

    #[pyo3(signature = (separator = "", strip = false, *args, **kwargs))]
    fn get_text(
        &self,
        separator: &str,
        strip: bool,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<String> {
        let text_types = text_type_selection_from_call(args, kwargs)?;
        let document = read_document(&self.document);
        Ok(document.text_with_options(
            document.root,
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
    fn get(&self, py: Python<'_>, name: &str, default: Option<Py<PyAny>>) -> Py<PyAny> {
        let _ = name;
        default.unwrap_or_else(|| py.None())
    }

    fn has_attr(&self, name: &str) -> bool {
        let _ = name;
        false
    }

    fn has_key(&self, name: &str) -> bool {
        self.has_attr(name)
    }

    fn get_attribute_list(&self, name: &str) -> Vec<String> {
        let _ = name;
        Vec::new()
    }

    fn append(&self, py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let root = read_document(&self.document).root;
        if let Ok(tag) = item.extract::<PyRef<'_, Tag>>() {
            let id = append_tag_to_parent(&self.document, root, &tag);
            return node_to_py(py, &self.document, id);
        }
        if let Some(string) = extract_rustysoup_string(item) {
            let id = append_string_to_parent(py, item, &self.document, root, &string)?;
            return node_to_py(py, &self.document, id);
        }
        if let Ok(text) = item.extract::<String>() {
            let id = crate::shared::write_document(&self.document).append_text(root, text);
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
        let root = read_document(&self.document).root;
        if let Ok(tag) = item.extract::<PyRef<'_, Tag>>() {
            let id = insert_tag_into_parent(&self.document, root, index, &tag);
            return node_to_py(py, &self.document, id);
        }
        if let Some(string) = extract_rustysoup_string(item) {
            let id = insert_string_into_parent(py, item, &self.document, root, index, &string)?;
            return node_to_py(py, &self.document, id);
        }
        if let Ok(text) = item.extract::<String>() {
            let id = crate::shared::write_document(&self.document).insert_text(root, index, text);
            return node_to_py(py, &self.document, id);
        }
        Err(PyTypeError::new_err(
            "rustysoup currently supports inserting strings and Tag objects",
        ))
    }

    fn index(&self, item: &Bound<'_, PyAny>) -> PyResult<usize> {
        let root = read_document(&self.document).root;
        let target = if let Ok(tag) = item.extract::<PyRef<'_, Tag>>() {
            if !Arc::ptr_eq(&self.document, &tag.document) {
                return Err(PyValueError::new_err("Tag.index: element not in tag"));
            }
            tag.id
        } else if let Some(string) = extract_rustysoup_string(item) {
            if !Arc::ptr_eq(&self.document, &string.document) {
                return Err(PyValueError::new_err("Tag.index: element not in tag"));
            }
            string.id
        } else {
            return Err(PyValueError::new_err("Tag.index: element not in tag"));
        };
        read_document(&self.document)
            .child_index(root, target)
            .ok_or_else(|| PyValueError::new_err("Tag.index: element not in tag"))
    }

    fn clear(&self) {
        let root = read_document(&self.document).root;
        crate::shared::write_document(&self.document).clear_children(root);
    }

    fn decompose(&self) {
        crate::shared::write_document(&self.document).decompose_root();
    }

    fn smooth(&self) {
        let root = read_document(&self.document).root;
        crate::shared::write_document(&self.document).smooth_text_nodes(root);
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
        let root = read_document(&self.document).root;
        find_first_compat(
            py,
            &self.document,
            root,
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
        let root = read_document(&self.document).root;
        find_all_compat(
            py,
            &self.document,
            root,
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
        let root = read_document(&self.document).root;
        if try_fast_find_all_into_py_list(
            py,
            &self.document,
            root,
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
            root,
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
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Option<Py<PyAny>> {
        let _ = (name, attrs, string, kwargs);
        None
    }

    #[pyo3(name = "findNext", signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_next_legacy(
        &self,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Option<Py<PyAny>> {
        self.find_next(name, attrs, string, kwargs)
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
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Vec<Py<PyAny>> {
        let _ = (name, attrs, string, limit, kwargs);
        Vec::new()
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
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Vec<Py<PyAny>> {
        self.find_all_next(name, attrs, string, limit, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_previous(
        &self,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Option<Py<PyAny>> {
        let _ = (name, attrs, string, kwargs);
        None
    }

    #[pyo3(name = "findPrevious", signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_previous_legacy(
        &self,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Option<Py<PyAny>> {
        self.find_previous(name, attrs, string, kwargs)
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
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Vec<Py<PyAny>> {
        let _ = (name, attrs, string, limit, kwargs);
        Vec::new()
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
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Vec<Py<PyAny>> {
        self.find_all_previous(name, attrs, string, limit, kwargs)
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
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Vec<Py<PyAny>> {
        self.find_all_previous(name, attrs, string, limit, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_parent(
        &self,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Option<Py<PyAny>> {
        let _ = (name, attrs, string, kwargs);
        None
    }

    #[pyo3(name = "findParent", signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_parent_legacy(
        &self,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Option<Py<PyAny>> {
        self.find_parent(name, attrs, string, kwargs)
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
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Vec<Py<PyAny>> {
        let _ = (name, attrs, string, limit, kwargs);
        Vec::new()
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
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Vec<Py<PyAny>> {
        self.find_parents(name, attrs, string, limit, kwargs)
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
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Vec<Py<PyAny>> {
        self.find_parents(name, attrs, string, limit, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_next_sibling(
        &self,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Option<Py<PyAny>> {
        let _ = (name, attrs, string, kwargs);
        None
    }

    #[pyo3(name = "findNextSibling", signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_next_sibling_legacy(
        &self,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Option<Py<PyAny>> {
        self.find_next_sibling(name, attrs, string, kwargs)
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
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Vec<Py<PyAny>> {
        let _ = (name, attrs, string, limit, kwargs);
        Vec::new()
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
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Vec<Py<PyAny>> {
        self.find_next_siblings(name, attrs, string, limit, kwargs)
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
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Vec<Py<PyAny>> {
        self.find_next_siblings(name, attrs, string, limit, kwargs)
    }

    #[pyo3(signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_previous_sibling(
        &self,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Option<Py<PyAny>> {
        let _ = (name, attrs, string, kwargs);
        None
    }

    #[pyo3(name = "findPreviousSibling", signature = (
        name = None,
        attrs = None,
        string = None,
        **kwargs
    ))]
    fn find_previous_sibling_legacy(
        &self,
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Option<Py<PyAny>> {
        self.find_previous_sibling(name, attrs, string, kwargs)
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
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Vec<Py<PyAny>> {
        let _ = (name, attrs, string, limit, kwargs);
        Vec::new()
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
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Vec<Py<PyAny>> {
        self.find_previous_siblings(name, attrs, string, limit, kwargs)
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
        name: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        limit: Option<usize>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Vec<Py<PyAny>> {
        self.find_previous_siblings(name, attrs, string, limit, kwargs)
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
        let root = read_document(&self.document).root;
        let ids = select_all_detached(py, &self.document, root, false, selector, limit)?;
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
        let root = read_document(&self.document).root;
        let ids = select_all_detached(py, &self.document, root, false, selector, limit)?;
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
        let root = read_document(&self.document).root;
        let ids = select_all_detached(py, &self.document, root, false, selector, 1)?;
        Ok(ids
            .into_iter()
            .next()
            .map(|id| Tag::new(Arc::clone(&self.document), id)))
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
            document.root,
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
        let markup = document.inner_html_with_encoding_options(document.root, true, encoding);
        py_encode_string(py, &markup, encoding, "strict")
    }

    #[pyo3(signature = (indent_level = None, eventual_encoding = "utf-8", formatter = None, iterator = None, **kwargs))]
    fn decode(
        &self,
        indent_level: Option<usize>,
        eventual_encoding: &str,
        formatter: Option<&Bound<'_, PyAny>>,
        iterator: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<String> {
        let _ = (indent_level, iterator, kwargs);
        let document = read_document(&self.document);
        render_outer_html_with_py_formatter_and_encoding(
            &document,
            document.root,
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
            &self.decode(None, encoding, formatter, None, None)?,
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
        let pretty = render_prettify_with_py_formatter(&document, document.root, formatter)?;
        if let Some(encoding) = encoding {
            return py_encode_string(py, &pretty, encoding, "xmlcharrefreplace")?.into_py_any(py);
        }
        pretty.into_py_any(py)
    }

    #[pyo3(signature = (
        name,
        namespace = None,
        nsprefix = None,
        attrs = None,
        sourceline = None,
        sourcepos = None,
        string = None,
        **kwargs
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new_tag(
        &self,
        name: &str,
        namespace: Option<&Bound<'_, PyAny>>,
        nsprefix: Option<&Bound<'_, PyAny>>,
        attrs: Option<&Bound<'_, PyAny>>,
        sourceline: Option<&Bound<'_, PyAny>>,
        sourcepos: Option<&Bound<'_, PyAny>>,
        string: Option<&Bound<'_, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Tag> {
        let _ = (namespace, nsprefix, sourceline, sourcepos);
        let attrs = collect_new_tag_attrs(attrs, kwargs)?;
        let html = render_new_tag(name, &attrs, string)?;
        let parsed = parse_html(&html);
        let parsed_id = parsed
            .descendant_elements(parsed.root, false)
            .into_iter()
            .next()
            .ok_or_else(|| PyValueError::new_err("could not create tag from rendered markup"))?;
        let id = {
            let mut document = crate::shared::write_document(&self.document);
            document.clone_detached_from(&parsed, parsed_id)
        };
        Ok(Tag::new(Arc::clone(&self.document), id))
    }

    #[pyo3(signature = (s, subclass = None))]
    fn new_string(
        &self,
        py: Python<'_>,
        s: &Bound<'_, PyAny>,
        subclass: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let value = s.str()?.to_str()?.to_string();
        let (document, id) = match string_subclass_name(subclass).as_deref() {
            Some("Comment") => Document::detached_comment(value),
            Some("CData") => Document::detached_cdata(value),
            Some("Declaration") => Document::detached_declaration(value),
            Some("Doctype") => Document::detached_doctype(value),
            Some("ProcessingInstruction") => Document::detached_processing_instruction(value),
            Some("TemplateString" | "RubyTextString" | "RubyParenthesisString") => {
                Document::detached_template_string(value)
            }
            _ => Document::detached_text(value),
        };
        let document = shared_document(document);
        node_to_py(py, &document, id)
    }

    fn __getattr__(&self, name: &str) -> Option<Tag> {
        let document = read_document(&self.document);
        let criteria = FindCriteria::with_name(Some(name));
        matcher::find_first(&document, document.root, false, &criteria)
            .map(|id| Tag::new(Arc::clone(&self.document), id))
    }

    fn __str__(&self) -> String {
        let document = read_document(&self.document);
        if document.root_decomposed {
            return "<></>".to_string();
        }
        document.outer_html(document.root)
    }

    fn __repr__(&self) -> String {
        self.__str__()
    }
}

fn soup_markup(document: &SharedDocument) -> String {
    let document = read_document(document);
    if document.root_decomposed {
        "<></>".to_string()
    } else {
        document.outer_html(document.root)
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn find_all_compat(
    py: Python<'_>,
    document: &SharedDocument,
    root: NodeId,
    name: Option<&Bound<'_, PyAny>>,
    attrs: Option<&Bound<'_, PyAny>>,
    recursive: bool,
    string: Option<&Bound<'_, PyAny>>,
    limit: Option<usize>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let nodes = find_all_compat_node_ids(
        py, document, root, name, attrs, recursive, string, limit, kwargs,
    )?;
    nodes_to_py_public(py, document, nodes)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn find_first_compat(
    py: Python<'_>,
    document: &SharedDocument,
    root: NodeId,
    name: Option<&Bound<'_, PyAny>>,
    attrs: Option<&Bound<'_, PyAny>>,
    recursive: bool,
    string: Option<&Bound<'_, PyAny>>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Option<Py<PyAny>>> {
    if let Some(id) =
        try_fast_find_first(py, document, root, name, attrs, recursive, string, kwargs)?
    {
        return id
            .map(|id| Tag::new(Arc::clone(document), id).into_py_any(py))
            .transpose();
    }

    Ok(find_all_compat(
        py,
        document,
        root,
        name,
        attrs,
        recursive,
        string,
        Some(1),
        kwargs,
    )?
    .into_iter()
    .next())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn find_all_compat_node_ids(
    py: Python<'_>,
    document: &SharedDocument,
    root: NodeId,
    name: Option<&Bound<'_, PyAny>>,
    attrs: Option<&Bound<'_, PyAny>>,
    recursive: bool,
    string: Option<&Bound<'_, PyAny>>,
    limit: Option<usize>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Vec<NodeId>> {
    if let Some(results) = try_fast_find_all(
        py, document, root, name, attrs, recursive, string, limit, kwargs,
    )? {
        return Ok(results);
    }

    let text_alias = if let Some(kwargs) = kwargs {
        kwargs.get_item("text")?
    } else {
        None
    };
    let string = string.or(text_alias.as_ref());
    let attr_filters = collect_attr_filters(attrs, kwargs)?;
    let name_is_absent = name.is_none_or(|value| value.is_none());
    let wants_strings = name_is_absent && string.is_some() && attr_filters.is_empty();
    let mut results = Vec::new();
    let mut current = {
        let document = read_document(document);
        document.node(root).first_child
    };

    while let Some(id) = current {
        let next = {
            let document = read_document(document);
            if recursive {
                document.next_in_subtree(root, id)
            } else {
                document.node(id).next_sibling
            }
        };
        if compat_candidate_matches(py, document, id, name, &attr_filters, string, wants_strings)? {
            results.push(id);
        }
        if limit.is_some_and(|value| value > 0 && results.len() >= value) {
            break;
        }
        current = next;
    }

    Ok(results)
}

#[allow(clippy::too_many_arguments)]
fn try_fast_find_all(
    py: Python<'_>,
    document: &SharedDocument,
    root: NodeId,
    name: Option<&Bound<'_, PyAny>>,
    attrs: Option<&Bound<'_, PyAny>>,
    recursive: bool,
    string: Option<&Bound<'_, PyAny>>,
    limit: Option<usize>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Option<Vec<NodeId>>> {
    if string.is_some() || kwargs_has_key(kwargs, "text")? {
        return Ok(None);
    }
    let Some(name_filter) = SimpleNameFilter::from_py(name)? else {
        return Ok(None);
    };
    let attr_filters = collect_attr_filters(attrs, kwargs)?;
    let Some(attr_filters) = SimpleAttrFilter::from_filters(py, &attr_filters)? else {
        return Ok(None);
    };

    let matched = {
        let document = read_document(document);
        let mut out = Vec::new();
        if recursive {
            push_fast_matches(
                &document,
                root,
                false,
                &name_filter,
                &attr_filters,
                limit,
                &mut out,
            );
        } else {
            let mut child = document.node(root).first_child;
            while let Some(current) = child {
                if fast_matches(&document, current, &name_filter, &attr_filters) {
                    out.push(current);
                    if limit.is_some_and(|value| value > 0 && out.len() >= value) {
                        break;
                    }
                }
                child = document.node(current).next_sibling;
            }
        }
        out
    };

    Ok(Some(matched))
}

#[allow(clippy::too_many_arguments)]
fn try_fast_find_first(
    py: Python<'_>,
    document: &SharedDocument,
    root: NodeId,
    name: Option<&Bound<'_, PyAny>>,
    attrs: Option<&Bound<'_, PyAny>>,
    recursive: bool,
    string: Option<&Bound<'_, PyAny>>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Option<Option<NodeId>>> {
    if string.is_some() || kwargs_has_key(kwargs, "text")? {
        return Ok(None);
    }
    let Some(name_filter) = SimpleNameFilter::from_py(name)? else {
        return Ok(None);
    };
    let attr_filters = collect_attr_filters(attrs, kwargs)?;
    let Some(attr_filters) = SimpleAttrFilter::from_filters(py, &attr_filters)? else {
        return Ok(None);
    };

    let document = read_document(document);
    if recursive {
        let mut current = document.node(root).first_child;
        while let Some(candidate) = current {
            if fast_matches(&document, candidate, &name_filter, &attr_filters) {
                return Ok(Some(Some(candidate)));
            }
            current = document.next_in_subtree(root, candidate);
        }
    } else {
        let mut child = document.node(root).first_child;
        while let Some(current) = child {
            if fast_matches(&document, current, &name_filter, &attr_filters) {
                return Ok(Some(Some(current)));
            }
            child = document.node(current).next_sibling;
        }
    }
    Ok(Some(None))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn try_fast_find_all_into_py_list(
    py: Python<'_>,
    document: &SharedDocument,
    root: NodeId,
    out: &Bound<'_, PyAny>,
    name: Option<&Bound<'_, PyAny>>,
    attrs: Option<&Bound<'_, PyAny>>,
    recursive: bool,
    string: Option<&Bound<'_, PyAny>>,
    limit: Option<usize>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<bool> {
    if string.is_some() || kwargs_has_key(kwargs, "text")? {
        return Ok(false);
    }
    let Some(name_filter) = SimpleNameFilter::from_py(name)? else {
        return Ok(false);
    };
    let attr_filters = collect_attr_filters(attrs, kwargs)?;
    let Some(attr_filters) = SimpleAttrFilter::from_filters(py, &attr_filters)? else {
        return Ok(false);
    };

    let out = out.cast::<PyList>()?;
    let document_guard = read_document(document);
    let mut count = 0usize;
    if recursive {
        append_fast_matches_to_py_list(
            py,
            document,
            &document_guard,
            root,
            false,
            &name_filter,
            &attr_filters,
            limit,
            out,
            &mut count,
        )?;
    } else {
        let mut child = document_guard.node(root).first_child;
        while let Some(current) = child {
            if fast_matches(&document_guard, current, &name_filter, &attr_filters) {
                out.append(Tag::new(Arc::clone(document), current).into_py_any(py)?)?;
                count += 1;
                if limit.is_some_and(|value| value > 0 && count >= value) {
                    break;
                }
            }
            child = document_guard.node(current).next_sibling;
        }
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn append_fast_matches_to_py_list(
    py: Python<'_>,
    shared_document: &SharedDocument,
    document: &Document,
    id: NodeId,
    include_self: bool,
    name_filter: &SimpleNameFilter,
    attr_filters: &[SimpleAttrFilter],
    limit: Option<usize>,
    out: &Bound<'_, PyList>,
    count: &mut usize,
) -> PyResult<()> {
    let mut current = if include_self {
        Some(id)
    } else {
        document.node(id).first_child
    };

    while let Some(candidate) = current {
        if fast_matches(document, candidate, name_filter, attr_filters) {
            out.append(Tag::new(Arc::clone(shared_document), candidate).into_py_any(py)?)?;
            *count += 1;
            if limit.is_some_and(|value| value > 0 && *count >= value) {
                return Ok(());
            }
        }
        current = document.next_in_subtree(id, candidate);
    }
    Ok(())
}

fn push_fast_matches(
    document: &Document,
    id: NodeId,
    include_self: bool,
    name_filter: &SimpleNameFilter,
    attr_filters: &[SimpleAttrFilter],
    limit: Option<usize>,
    out: &mut Vec<NodeId>,
) {
    let mut current = if include_self {
        Some(id)
    } else {
        document.node(id).first_child
    };

    while let Some(candidate) = current {
        if fast_matches(document, candidate, name_filter, attr_filters) {
            out.push(candidate);
            if limit.is_some_and(|value| value > 0 && out.len() >= value) {
                return;
            }
        }
        current = document.next_in_subtree(id, candidate);
    }
}

fn fast_matches(
    document: &Document,
    id: NodeId,
    name_filter: &SimpleNameFilter,
    attr_filters: &[SimpleAttrFilter],
) -> bool {
    let Some(element) = document.element(id) else {
        return false;
    };
    match name_filter {
        SimpleNameFilter::Any => {}
        SimpleNameFilter::Name(name) if element.tag_name() == name => {}
        SimpleNameFilter::Name(_) => return false,
    }
    let attrs = element.attrs.as_ref();
    attr_filters.iter().all(|filter| filter.matches(attrs))
}

enum SimpleNameFilter {
    Any,
    Name(String),
}

impl SimpleNameFilter {
    fn from_py(filter: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Self>> {
        let Some(filter) = filter else {
            return Ok(Some(Self::Any));
        };
        if filter.is_none() {
            return Ok(Some(Self::Any));
        }
        if let Ok(value) = filter.extract::<String>() {
            return Ok(Some(Self::Name(value.to_ascii_lowercase())));
        }
        Ok(None)
    }
}

enum SimpleAttrFilter {
    Exists(String),
    Missing(String),
    Exact(String, String),
    ContainsToken(String, String),
}

impl SimpleAttrFilter {
    fn from_filters(
        _py: Python<'_>,
        filters: &[(String, Bound<'_, PyAny>)],
    ) -> PyResult<Option<Vec<Self>>> {
        let mut out = Vec::with_capacity(filters.len());
        for (name, filter) in filters {
            if filter.is_none() {
                out.push(Self::Missing(name.clone()));
            } else if let Ok(flag) = filter.extract::<bool>() {
                out.push(if flag {
                    Self::Exists(name.clone())
                } else {
                    Self::Missing(name.clone())
                });
            } else if let Ok(value) = filter.extract::<String>() {
                if name == "class" && value.split_ascii_whitespace().count() == 1 {
                    out.push(Self::ContainsToken(name.clone(), value));
                } else {
                    out.push(Self::Exact(name.clone(), value));
                }
            } else {
                return Ok(None);
            }
        }
        Ok(Some(out))
    }

    fn matches(&self, attrs: &[Attr]) -> bool {
        match self {
            Self::Exists(name) => attr_str(attrs, name).is_some(),
            Self::Missing(name) => attr_str(attrs, name).is_none(),
            Self::Exact(name, expected) => match attr_value(attrs, name) {
                Some(Some(value)) => value == expected,
                Some(None) => expected.is_empty(),
                None => false,
            },
            Self::ContainsToken(name, expected) => attr_str(attrs, name).is_some_and(|value| {
                value
                    .split_ascii_whitespace()
                    .any(|token| token == expected)
            }),
        }
    }
}

#[inline]
fn attr_str<'a>(attrs: &'a [Attr], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .rev()
        .find(|attr| attr.name() == name)
        .and_then(|attr| attr.value.as_deref())
}

#[inline]
fn attr_value<'a>(attrs: &'a [Attr], name: &str) -> Option<Option<&'a str>> {
    attrs
        .iter()
        .rev()
        .find(|attr| attr.name() == name)
        .map(|attr| attr.value.as_deref())
}

fn kwargs_has_key(kwargs: Option<&Bound<'_, PyDict>>, key: &str) -> PyResult<bool> {
    Ok(kwargs
        .map(|kwargs| kwargs.contains(key))
        .transpose()?
        .unwrap_or(false))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn find_all_compat_in_nodes(
    py: Python<'_>,
    document: &SharedDocument,
    candidates: Vec<NodeId>,
    name: Option<&Bound<'_, PyAny>>,
    attrs: Option<&Bound<'_, PyAny>>,
    string: Option<&Bound<'_, PyAny>>,
    limit: Option<usize>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let nodes = find_all_compat_node_ids_in_nodes(
        py, document, candidates, name, attrs, string, limit, kwargs,
    )?;
    nodes_to_py_public(py, document, nodes)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn find_all_compat_node_ids_in_nodes(
    py: Python<'_>,
    document: &SharedDocument,
    candidates: Vec<NodeId>,
    name: Option<&Bound<'_, PyAny>>,
    attrs: Option<&Bound<'_, PyAny>>,
    string: Option<&Bound<'_, PyAny>>,
    limit: Option<usize>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Vec<NodeId>> {
    let text_alias = if let Some(kwargs) = kwargs {
        kwargs.get_item("text")?
    } else {
        None
    };
    let string = string.or(text_alias.as_ref());
    let attr_filters = collect_attr_filters(attrs, kwargs)?;
    let name_is_absent = name.is_none_or(|value| value.is_none());
    let wants_strings = name_is_absent && string.is_some() && attr_filters.is_empty();
    let mut results = Vec::new();

    for id in candidates {
        if compat_candidate_matches(py, document, id, name, &attr_filters, string, wants_strings)? {
            results.push(id);
        }

        if limit.is_some_and(|value| value > 0 && results.len() >= value) {
            break;
        }
    }

    Ok(results)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn find_all_compat_parent_nodes(
    py: Python<'_>,
    document: &SharedDocument,
    id: NodeId,
    name: Option<&Bound<'_, PyAny>>,
    attrs: Option<&Bound<'_, PyAny>>,
    string: Option<&Bound<'_, PyAny>>,
    limit: Option<usize>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let text_alias = if let Some(kwargs) = kwargs {
        kwargs.get_item("text")?
    } else {
        None
    };
    let string = string.or(text_alias.as_ref());
    let attr_filters = collect_attr_filters(attrs, kwargs)?;
    let name_is_absent = name.is_none_or(|value| value.is_none());
    let wants_strings = name_is_absent && string.is_some() && attr_filters.is_empty();
    let mut results = Vec::new();
    let mut parent = read_document(document).node(id).parent;

    while let Some(current) = parent {
        parent = read_document(document).node(current).parent;
        if compat_candidate_matches(
            py,
            document,
            current,
            name,
            &attr_filters,
            string,
            wants_strings,
        )? {
            results.push(node_to_py(py, document, current)?);
        }
        if limit.is_some_and(|value| value > 0 && results.len() >= value) {
            break;
        }
    }

    Ok(results)
}

#[derive(Clone, Copy)]
pub(crate) enum SiblingDirection {
    Next,
    Previous,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn find_all_compat_sibling_nodes(
    py: Python<'_>,
    document: &SharedDocument,
    id: NodeId,
    direction: SiblingDirection,
    name: Option<&Bound<'_, PyAny>>,
    attrs: Option<&Bound<'_, PyAny>>,
    string: Option<&Bound<'_, PyAny>>,
    limit: Option<usize>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let text_alias = if let Some(kwargs) = kwargs {
        kwargs.get_item("text")?
    } else {
        None
    };
    let string = string.or(text_alias.as_ref());
    let attr_filters = collect_attr_filters(attrs, kwargs)?;
    let name_is_absent = name.is_none_or(|value| value.is_none());
    let wants_strings = name_is_absent && string.is_some() && attr_filters.is_empty();
    let mut results = Vec::new();
    let mut sibling = {
        let document = read_document(document);
        let node = document.node(id);
        match direction {
            SiblingDirection::Next => node.next_sibling,
            SiblingDirection::Previous => node.prev_sibling,
        }
    };

    while let Some(current) = sibling {
        sibling = {
            let document = read_document(document);
            let node = document.node(current);
            match direction {
                SiblingDirection::Next => node.next_sibling,
                SiblingDirection::Previous => node.prev_sibling,
            }
        };
        if compat_candidate_matches(
            py,
            document,
            current,
            name,
            &attr_filters,
            string,
            wants_strings,
        )? {
            results.push(node_to_py(py, document, current)?);
        }
        if limit.is_some_and(|value| value > 0 && results.len() >= value) {
            break;
        }
    }

    Ok(results)
}

fn compat_candidate_matches(
    py: Python<'_>,
    document: &SharedDocument,
    id: NodeId,
    name: Option<&Bound<'_, PyAny>>,
    attr_filters: &[(String, Bound<'_, PyAny>)],
    string: Option<&Bound<'_, PyAny>>,
    wants_strings: bool,
) -> PyResult<bool> {
    let is_matchable_node = {
        let document = read_document(document);
        document.is_element(id) || matches!(document.node(id).node_type, NodeType::Document)
    };
    if is_matchable_node {
        if wants_strings {
            return Ok(false);
        }
        if !matches_name_filter(name, document, id)? {
            return Ok(false);
        }
        if !matches_attr_filters(py, document, id, attr_filters)? {
            return Ok(false);
        }
        if let Some(string_filter) = string {
            let (string_id, value) = {
                let document = read_document(document);
                let Some(string_id) = document.tag_string_node(id) else {
                    return Ok(false);
                };
                let Some(value) = document.node_string(string_id) else {
                    return Ok(false);
                };
                (string_id, value.to_string())
            };
            if !matches_string_node_filter(py, document, string_id, Some(&value), string_filter)? {
                return Ok(false);
            }
        }
        return Ok(true);
    }

    if wants_strings {
        let value = {
            let document = read_document(document);
            if !document.is_text_like(id) {
                return Ok(false);
            }
            let Some(value) = document.node_string(id) else {
                return Ok(false);
            };
            value.to_string()
        };
        if let Some(string_filter) = string
            && matches_string_node_filter(py, document, id, Some(&value), string_filter)?
        {
            return Ok(true);
        }
    }

    Ok(false)
}

pub(crate) fn select_all_detached(
    py: Python<'_>,
    document: &SharedDocument,
    root: NodeId,
    include_root: bool,
    selector: &str,
    limit: usize,
) -> PyResult<Vec<NodeId>> {
    let document = Arc::clone(document);
    let selector = selector.to_string();
    py.detach(move || {
        let document = read_document(&document);
        matcher::select_all(&document, root, include_root, &selector, limit)
    })
}

enum MarkupInput {
    Empty,
    Text(PyBackedStr),
    DecodedBytes(DecodedMarkup),
}

struct DecodedMarkup {
    text: String,
    encoding_metadata: EncodingMetadata,
}

struct ConstructorArgs<'py> {
    markup: Option<Bound<'py, PyAny>>,
    markup_provided: bool,
    features: Option<Bound<'py, PyAny>>,
    builder: Option<Bound<'py, PyAny>>,
    parse_only: Option<Bound<'py, PyAny>>,
    from_encoding: Option<Bound<'py, PyAny>>,
    exclude_encodings: Option<Bound<'py, PyAny>>,
    element_classes: Option<Bound<'py, PyAny>>,
}

impl<'py> ConstructorArgs<'py> {
    fn parse(args: &Bound<'py, PyTuple>, kwargs: Option<&Bound<'py, PyDict>>) -> PyResult<Self> {
        const ARG_NAMES: [&str; 7] = [
            "markup",
            "features",
            "builder",
            "parse_only",
            "from_encoding",
            "exclude_encodings",
            "element_classes",
        ];

        if args.len() > ARG_NAMES.len() {
            return Err(PyTypeError::new_err(format!(
                "BeautifulSoup() takes at most {} positional arguments ({} given)",
                ARG_NAMES.len(),
                args.len()
            )));
        }

        let mut parsed = Self {
            markup: None,
            markup_provided: false,
            features: None,
            builder: None,
            parse_only: None,
            from_encoding: None,
            exclude_encodings: None,
            element_classes: None,
        };

        for (index, name) in ARG_NAMES.iter().enumerate().take(args.len()) {
            parsed.set_arg(name, args.get_item(index)?)?;
        }

        if let Some(kwargs) = kwargs {
            for (key, value) in kwargs.iter() {
                let Ok(name) = key.extract::<String>() else {
                    continue;
                };
                parsed.set_arg(&name, value)?;
            }
        }

        Ok(parsed)
    }

    fn set_arg(&mut self, name: &str, value: Bound<'py, PyAny>) -> PyResult<()> {
        match name {
            "markup" => {
                if self.markup_provided {
                    return Err(duplicate_constructor_argument(name));
                }
                self.markup = Some(value);
                self.markup_provided = true;
            }
            "features" => set_constructor_slot(&mut self.features, value, name)?,
            "builder" => set_constructor_slot(&mut self.builder, value, name)?,
            "parse_only" => set_constructor_slot(&mut self.parse_only, value, name)?,
            "from_encoding" => set_constructor_slot(&mut self.from_encoding, value, name)?,
            "exclude_encodings" => {
                set_constructor_slot(&mut self.exclude_encodings, value, name)?;
            }
            "element_classes" => set_constructor_slot(&mut self.element_classes, value, name)?,
            _ => {}
        }
        Ok(())
    }
}

fn set_constructor_slot<'py>(
    slot: &mut Option<Bound<'py, PyAny>>,
    value: Bound<'py, PyAny>,
    name: &str,
) -> PyResult<()> {
    if slot.is_some() {
        return Err(duplicate_constructor_argument(name));
    }
    *slot = Some(value);
    Ok(())
}

fn duplicate_constructor_argument(name: &str) -> PyErr {
    PyTypeError::new_err(format!(
        "BeautifulSoup.__init__() got multiple values for argument '{name}'"
    ))
}

#[derive(Clone, Default)]
struct EncodingMetadata {
    original_encoding: Option<String>,
    declared_html_encoding: Option<String>,
    contains_replacement_characters: bool,
}

impl MarkupInput {
    fn parse(&self, mode: ParseMode) -> Document {
        match self {
            Self::Empty => mode.parse(""),
            Self::Text(text) => mode.parse(text.as_ref()),
            Self::DecodedBytes(decoded) => mode.parse(&decoded.text),
        }
    }

    fn encoding_metadata(&self) -> EncodingMetadata {
        match self {
            Self::DecodedBytes(decoded) => decoded.encoding_metadata.clone(),
            Self::Empty | Self::Text(_) => EncodingMetadata::default(),
        }
    }
}

#[derive(Clone, Copy)]
enum ParseMode {
    Fragment,
    FullDocument,
}

impl ParseMode {
    fn parse(self, html: &str) -> Document {
        match self {
            Self::Fragment => parse_html(html),
            Self::FullDocument => parse_html_document(html),
        }
    }
}

fn parse_markup_input(
    markup: Option<&Bound<'_, PyAny>>,
    markup_provided: bool,
    from_encoding: Option<&Bound<'_, PyAny>>,
    exclude_encodings: Option<&Bound<'_, PyAny>>,
) -> PyResult<MarkupInput> {
    let Some(markup) = markup else {
        return Ok(MarkupInput::Empty);
    };
    if markup.is_none() {
        return if markup_provided {
            Err(invalid_markup_type_error(markup))
        } else {
            Ok(MarkupInput::Empty)
        };
    }
    if let Ok(bytes) = markup.cast::<PyBytes>() {
        let bytes = PyBackedBytes::from(bytes.to_owned());
        let from_encoding = optional_encoding_label(from_encoding)?;
        let exclude_encodings = excluded_encoding_labels(exclude_encodings)?;
        return Ok(MarkupInput::DecodedBytes(decode_markup_bytes(
            bytes.as_ref(),
            from_encoding,
            &exclude_encodings,
        )));
    }
    if let Ok(text) = markup.cast::<PyString>() {
        return Ok(MarkupInput::Text(text.to_owned().try_into()?));
    }
    if markup.hasattr("read")? {
        let read = markup.getattr("read")?;
        if read.is_callable() {
            let contents = read.call0()?;
            return parse_markup_input(Some(&contents), true, from_encoding, exclude_encodings);
        }
    }
    Err(invalid_markup_type_error(markup))
}

fn invalid_markup_type_error(markup: &Bound<'_, PyAny>) -> PyErr {
    let description = markup
        .repr()
        .and_then(|value| value.extract::<String>())
        .unwrap_or_else(|_| "<object>".to_string());
    PyTypeError::new_err(format!(
        "Incoming markup is of an invalid type: {description}. Markup must be a string, a bytestring, or an open filehandle."
    ))
}

fn decode_markup_bytes(
    bytes: &[u8],
    from_encoding: Option<String>,
    exclude_encodings: &[String],
) -> DecodedMarkup {
    let declared_encoding = from_encoding
        .is_none()
        .then(|| detect_declared_encoding(bytes))
        .flatten();
    let mut candidates = Vec::new();
    if let Some(encoding) = &from_encoding {
        candidates.push(encoding.clone());
    } else if let Some(encoding) = &declared_encoding {
        candidates.push(encoding.clone());
    } else {
        candidates.push("utf-8".to_string());
        candidates.push("iso-8859-1".to_string());
    }

    for candidate in candidates {
        if from_encoding.is_none() && is_excluded_encoding(&candidate, exclude_encodings) {
            continue;
        }
        if let Some((text, had_errors)) = decode_with_encoding(bytes, &candidate)
            && !had_errors
        {
            return DecodedMarkup {
                text,
                encoding_metadata: EncodingMetadata {
                    original_encoding: Some(normalize_encoding_label(&candidate)),
                    declared_html_encoding: declared_encoding
                        .as_deref()
                        .map(normalize_encoding_label),
                    contains_replacement_characters: false,
                },
            };
        }
    }

    let (text, _, had_errors) = encoding_rs::UTF_8.decode(bytes);
    DecodedMarkup {
        text: text.into_owned(),
        encoding_metadata: EncodingMetadata {
            original_encoding: Some("utf-8".to_string()),
            declared_html_encoding: declared_encoding.as_deref().map(normalize_encoding_label),
            contains_replacement_characters: had_errors,
        },
    }
}

fn decode_with_encoding(bytes: &[u8], label: &str) -> Option<(String, bool)> {
    let encoding = encoding_for_label(label)?;
    let (text, _, had_errors) = encoding.decode(bytes);
    Some((text.into_owned(), had_errors))
}

fn encoding_for_label(label: &str) -> Option<&'static Encoding> {
    let normalized = normalize_encoding_label(label);
    Encoding::for_label(normalized.as_bytes()).or_else(|| match normalized.as_str() {
        "latin-1" | "latin1" | "iso_8859-1" | "iso8859-1" => Encoding::for_label(b"iso-8859-1"),
        "windows_1252" | "cp1252" => Encoding::for_label(b"windows-1252"),
        _ => None,
    })
}

fn detect_declared_encoding(bytes: &[u8]) -> Option<String> {
    static XML_ENCODING_RE: LazyLock<Option<Regex>> = LazyLock::new(|| {
        Regex::new(r#"(?i)<\?xml[^>]*\bencoding\s*=\s*["']?\s*([A-Za-z0-9._:-]+)"#).ok()
    });
    static CHARSET_RE: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::new(r#"(?i)\bcharset\s*=\s*["']?\s*([A-Za-z0-9._:-]+)"#).ok());

    let prefix_len = bytes.len().min(4096);
    let prefix = String::from_utf8_lossy(&bytes[..prefix_len]);
    XML_ENCODING_RE
        .as_ref()?
        .captures(&prefix)
        .and_then(|captures| captures.get(1))
        .or_else(|| {
            CHARSET_RE
                .as_ref()?
                .captures(&prefix)
                .and_then(|captures| captures.get(1))
        })
        .map(|encoding| normalize_encoding_label(encoding.as_str()))
}

fn optional_encoding_label(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<String>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    Ok(Some(extract_encoding_label(value)?))
}

fn excluded_encoding_labels(value: Option<&Bound<'_, PyAny>>) -> PyResult<Vec<String>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(Vec::new());
    };
    if let Ok(label) = value.extract::<String>() {
        return Ok(vec![normalize_encoding_label(&label)]);
    }
    let mut labels = Vec::new();
    for item in value.try_iter()? {
        labels.push(extract_encoding_label(&item?)?);
    }
    Ok(labels)
}

fn extract_encoding_label(value: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(label) = value.extract::<String>() {
        return Ok(normalize_encoding_label(&label));
    }
    Ok(normalize_encoding_label(value.str()?.to_str()?))
}

fn normalize_encoding_label(label: &str) -> String {
    label
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_lowercase()
}

fn is_excluded_encoding(label: &str, excluded: &[String]) -> bool {
    let label = normalize_encoding_label(label);
    excluded.iter().any(|excluded| {
        excluded == &label
            || encoding_for_label(excluded)
                .zip(encoding_for_label(&label))
                .is_some_and(|(left, right)| left.name() == right.name())
    })
}

fn validate_features(py: Python<'_>, features: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
    let Some(features) = features else {
        return Ok(());
    };
    if features.is_none() {
        return Ok(());
    }

    let requested = requested_features(features)?;
    if requested.is_empty()
        || requested
            .iter()
            .any(|feature| is_supported_html_feature(feature))
    {
        return Ok(());
    }

    Err(feature_not_found_error(py, &requested.join(",")))
}

fn parse_mode_for_features(features: Option<&Bound<'_, PyAny>>) -> PyResult<ParseMode> {
    let Some(features) = features else {
        return Ok(ParseMode::Fragment);
    };
    if features.is_none() {
        return Ok(ParseMode::Fragment);
    }

    let requested = requested_features(features)?;
    if requested
        .iter()
        .any(|feature| matches!(feature.as_str(), "html.parser" | "strict"))
    {
        return Ok(ParseMode::Fragment);
    }
    if requested.iter().any(|feature| {
        matches!(
            feature.as_str(),
            "html" | "fast" | "permissive" | "lxml" | "lxml-html"
        )
    }) {
        return Ok(ParseMode::FullDocument);
    }
    Ok(ParseMode::Fragment)
}

fn requested_features(features: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if let Ok(feature) = features.extract::<String>() {
        return Ok(vec![feature]);
    }

    let mut requested = Vec::new();
    for feature in features.try_iter()? {
        requested.push(feature?.extract::<String>()?);
    }
    Ok(requested)
}

fn is_supported_html_feature(feature: &str) -> bool {
    matches!(
        feature,
        "html" | "html.parser" | "fast" | "strict" | "permissive" | "lxml" | "lxml-html"
    )
}

fn feature_not_found_error(py: Python<'_>, features: &str) -> PyErr {
    let message = format!(
        "Couldn't find a tree builder with the features you requested: {features}. Do you need to install a parser library?"
    );
    match py
        .import("rustysoup")
        .and_then(|module| module.getattr("FeatureNotFound"))
        .and_then(|exception| exception.call1((message.clone(),)))
    {
        Ok(error) => PyErr::from_value(error),
        Err(_) => PyValueError::new_err(message),
    }
}

#[derive(Clone)]
struct ParseOnlyFilter {
    name: Option<ParseOnlyMatcher>,
    attrs: Vec<ParseOnlyAttrFilter>,
    string: Option<ParseOnlyMatcher>,
}

impl ParseOnlyFilter {
    fn apply(&self, py: Python<'_>, document: Document) -> PyResult<Document> {
        let mut out = Document::empty();
        self.clone_matching_children(py, &document, document.root, &mut out)?;
        Ok(out)
    }

    fn clone_matching_children(
        &self,
        py: Python<'_>,
        source: &Document,
        parent: NodeId,
        out: &mut Document,
    ) -> PyResult<()> {
        let mut stack = Vec::with_capacity(32);
        push_children_reverse(source, parent, &mut stack);

        while let Some(current) = stack.pop() {
            if self.matches_element(py, source, current)?
                || self.matches_text(py, source, current)?
            {
                let cloned = out.clone_detached_from(source, current);
                out.append_existing(out.root, cloned);
            } else {
                push_children_reverse(source, current, &mut stack);
            }
        }
        Ok(())
    }

    fn matches_element(&self, py: Python<'_>, document: &Document, id: NodeId) -> PyResult<bool> {
        let Some(element) = document.element(id) else {
            return Ok(false);
        };
        if self.name.is_none() && self.attrs.is_empty() && self.string.is_some() {
            return Ok(false);
        }
        if self.string.is_some() {
            return Ok(false);
        }
        if let Some(name) = &self.name
            && !name.matches(py, element.tag_name())?
        {
            return Ok(false);
        }
        for filter in &self.attrs {
            if !filter.matches(py, document, id)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn matches_text(&self, py: Python<'_>, document: &Document, id: NodeId) -> PyResult<bool> {
        if self.name.is_some() || !self.attrs.is_empty() {
            return Ok(false);
        }
        let Some(expected) = &self.string else {
            return Ok(false);
        };
        if !document.is_text_like(id) {
            return Ok(false);
        }
        let Some(value) = document.node_string(id) else {
            return Ok(false);
        };
        expected.matches(py, value)
    }
}

#[derive(Clone)]
enum ParseOnlyMatcher {
    Exact(String),
    AnyOf(Vec<ParseOnlyMatcher>),
    Regex(Regex),
    Callable(Arc<Py<PyAny>>),
}

impl ParseOnlyMatcher {
    fn matches(&self, py: Python<'_>, value: &str) -> PyResult<bool> {
        match self {
            Self::Exact(expected) => Ok(value == expected),
            Self::AnyOf(matchers) => {
                for matcher in matchers {
                    if matcher.matches(py, value)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Self::Regex(regex) => Ok(regex.is_match(value)),
            Self::Callable(callable) => callable.bind(py).call1((value,))?.is_truthy(),
        }
    }

    fn matches_class_value(&self, py: Python<'_>, value: &str) -> PyResult<bool> {
        match self {
            Self::Exact(expected) => Ok(value
                .split_ascii_whitespace()
                .any(|token| token == expected)),
            Self::AnyOf(matchers) => {
                for matcher in matchers {
                    if matcher.matches_class_value(py, value)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Self::Regex(regex) => Ok(regex.is_match(value)),
            Self::Callable(callable) => callable.bind(py).call1((value,))?.is_truthy(),
        }
    }
}

fn push_children_reverse(document: &Document, parent: NodeId, stack: &mut Vec<NodeId>) {
    let mut child = document.node(parent).last_child;
    while let Some(current) = child {
        stack.push(current);
        child = document.node(current).prev_sibling;
    }
}

#[derive(Clone)]
enum ParseOnlyAttrFilter {
    Exists(String),
    Missing(String),
    Value {
        name: String,
        matcher: ParseOnlyMatcher,
        match_class_token: bool,
    },
}

impl ParseOnlyAttrFilter {
    fn matches(&self, py: Python<'_>, document: &Document, id: NodeId) -> PyResult<bool> {
        match self {
            Self::Exists(name) => Ok(document.attr(id, name).is_some()),
            Self::Missing(name) => Ok(document.attr(id, name).is_none()),
            Self::Value {
                name,
                matcher,
                match_class_token,
            } => {
                let Some(value) = document.attr(id, name) else {
                    return Ok(false);
                };
                if *match_class_token {
                    matcher.matches_class_value(py, value)
                } else {
                    matcher.matches(py, value)
                }
            }
        }
    }
}

fn parse_parse_only(parse_only: Option<&Bound<'_, PyAny>>) -> PyResult<Option<ParseOnlyFilter>> {
    let Some(parse_only) = parse_only.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    let spec = parse_only.getattr("_rustysoup_parse_only").map_err(|_| {
        PyTypeError::new_err("rustysoup parse_only currently expects rustysoup.SoupStrainer")
    })?;
    let spec = spec.cast::<PyTuple>().map_err(|_| {
        PyTypeError::new_err("rustysoup parse_only currently expects rustysoup.SoupStrainer")
    })?;
    if spec.len() != 3 {
        return Err(PyTypeError::new_err(
            "rustysoup parse_only currently expects rustysoup.SoupStrainer",
        ));
    }

    let name = parse_optional_matcher(&spec.get_item(0)?, true)?;
    let attrs = parse_parse_only_attrs(&spec.get_item(1)?)?;
    let string = parse_optional_matcher(&spec.get_item(2)?, false)?;

    if name.is_none() && attrs.is_empty() && string.is_none() {
        return Ok(None);
    }

    Ok(Some(ParseOnlyFilter {
        name,
        attrs,
        string,
    }))
}

fn parse_parse_only_attrs(attrs: &Bound<'_, PyAny>) -> PyResult<Vec<ParseOnlyAttrFilter>> {
    if attrs.is_none() {
        return Ok(Vec::new());
    }
    let attrs = attrs
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("SoupStrainer attrs must be a dict"))?;
    let mut filters = Vec::with_capacity(attrs.len());
    for (key, value) in attrs.iter() {
        let name = normalize_kwarg_attr_name(&key.extract::<String>()?);
        if value.is_none() {
            filters.push(ParseOnlyAttrFilter::Missing(name));
        } else if let Ok(flag) = value.extract::<bool>() {
            filters.push(if flag {
                ParseOnlyAttrFilter::Exists(name)
            } else {
                ParseOnlyAttrFilter::Missing(name)
            });
        } else {
            let matcher = parse_matcher(&value, false)?;
            let match_class_token = name == "class";
            filters.push(ParseOnlyAttrFilter::Value {
                name,
                matcher,
                match_class_token,
            });
        }
    }
    Ok(filters)
}

fn parse_optional_matcher(
    value: &Bound<'_, PyAny>,
    ascii_lowercase: bool,
) -> PyResult<Option<ParseOnlyMatcher>> {
    if value.is_none() {
        return Ok(None);
    }
    parse_matcher(value, ascii_lowercase).map(Some)
}

fn parse_matcher(value: &Bound<'_, PyAny>, ascii_lowercase: bool) -> PyResult<ParseOnlyMatcher> {
    if is_sequence_filter(value) {
        let mut matchers = Vec::new();
        for item in value.try_iter()? {
            matchers.push(parse_matcher(&item?, ascii_lowercase)?);
        }
        return Ok(ParseOnlyMatcher::AnyOf(matchers));
    }
    if has_search(value)? {
        return Ok(ParseOnlyMatcher::Regex(parse_regex_matcher(value)?));
    }
    if value.is_callable() {
        return Ok(ParseOnlyMatcher::Callable(Arc::new(value.clone().unbind())));
    }
    let mut expected = value.str()?.to_str()?.to_string();
    if ascii_lowercase {
        expected = expected.to_ascii_lowercase();
    }
    Ok(ParseOnlyMatcher::Exact(expected))
}

fn parse_regex_matcher(value: &Bound<'_, PyAny>) -> PyResult<Regex> {
    let pattern = value.getattr("pattern")?.str()?.to_str()?.to_string();
    let flags = value
        .getattr("flags")
        .ok()
        .and_then(|flags| flags.extract::<u32>().ok())
        .unwrap_or(0);
    let mut enabled = String::new();
    if flags & 2 != 0 {
        enabled.push('i');
    }
    if flags & 8 != 0 {
        enabled.push('m');
    }
    if flags & 16 != 0 {
        enabled.push('s');
    }
    if flags & 64 != 0 {
        enabled.push('x');
    }
    let pattern = if enabled.is_empty() {
        pattern
    } else {
        format!("(?{enabled}:{pattern})")
    };
    Regex::new(&pattern)
        .map_err(|err| PyValueError::new_err(format!("invalid SoupStrainer regex: {err}")))
}

pub(crate) fn nodes_to_py_public(
    py: Python<'_>,
    document: &SharedDocument,
    nodes: Vec<NodeId>,
) -> PyResult<Vec<Py<PyAny>>> {
    nodes
        .into_iter()
        .map(|id| node_to_py(py, document, id))
        .collect()
}

pub(crate) fn append_nodes_to_py_list(
    py: Python<'_>,
    document: &SharedDocument,
    nodes: Vec<NodeId>,
    out: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let out = out.cast::<PyList>()?;
    for id in nodes {
        out.append(node_to_py(py, document, id)?)?;
    }
    Ok(())
}

pub(crate) fn collect_string_nodes(document: &SharedDocument, root: NodeId) -> Vec<NodeId> {
    let document = read_document(document);
    let include_template = document.is_template_element(root);
    document
        .descendant_nodes(root, false)
        .into_iter()
        .filter(|id| {
            (matches!(
                document.node(*id).node_type,
                NodeType::Text(_) | NodeType::CData(_)
            ) || (include_template
                && matches!(document.node(*id).node_type, NodeType::TemplateString(_))))
                && !document.is_inside_skipped_raw_text_element(root, *id)
        })
        .collect()
}

fn collect_strings(document: &SharedDocument, root: NodeId, strip: bool) -> Vec<String> {
    let document = read_document(document);
    collect_string_values(&document, root, strip)
}

pub(crate) fn collect_string_values(document: &Document, root: NodeId, strip: bool) -> Vec<String> {
    let include_template = document.is_template_element(root);
    document
        .descendant_nodes(root, false)
        .into_iter()
        .filter_map(|id| {
            if !(matches!(
                document.node(id).node_type,
                NodeType::Text(_) | NodeType::CData(_)
            ) || (include_template
                && matches!(document.node(id).node_type, NodeType::TemplateString(_))))
            {
                return None;
            }
            if document.is_inside_skipped_raw_text_element(root, id) {
                return None;
            }
            let value = document.node_string(id)?;
            if strip {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            } else {
                Some(value.to_string())
            }
        })
        .collect()
}

fn collect_attr_filters<'py>(
    attrs: Option<&Bound<'py, PyAny>>,
    kwargs: Option<&Bound<'py, PyDict>>,
) -> PyResult<Vec<(String, Bound<'py, PyAny>)>> {
    let mut filters = Vec::new();

    if let Some(attrs) = attrs.filter(|value| !value.is_none()) {
        if let Ok(dict) = attrs.cast::<PyDict>() {
            for (key, value) in dict.iter() {
                filters.push((key.extract::<String>()?, value));
            }
        } else if attrs.extract::<String>().is_ok() {
            filters.push(("class".to_string(), attrs.clone()));
        } else {
            return Err(PyTypeError::new_err("attrs must be a dict or string"));
        }
    }

    if let Some(kwargs) = kwargs {
        for (key, value) in kwargs.iter() {
            let key = key.extract::<String>()?;
            if key == "text" {
                continue;
            }
            filters.push((normalize_kwarg_attr_name(&key), value));
        }
    }

    Ok(filters)
}

fn collect_new_tag_attrs<'py>(
    attrs: Option<&Bound<'py, PyAny>>,
    kwargs: Option<&Bound<'py, PyDict>>,
) -> PyResult<Vec<(String, String)>> {
    let mut out = Vec::new();
    if let Some(attrs) = attrs.filter(|value| !value.is_none()) {
        let dict = attrs
            .cast::<PyDict>()
            .map_err(|_| PyTypeError::new_err("attrs must be a dict when provided"))?;
        for (key, value) in dict.iter() {
            out.push((key.extract::<String>()?, value.str()?.to_str()?.to_string()));
        }
    }
    if let Some(kwargs) = kwargs {
        for (key, value) in kwargs.iter() {
            out.push((key.extract::<String>()?, value.str()?.to_str()?.to_string()));
        }
    }
    Ok(out)
}

fn render_new_tag(
    name: &str,
    attrs: &[(String, String)],
    string: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let mut out = String::new();
    out.push('<');
    out.push_str(name);
    for (key, value) in attrs {
        out.push(' ');
        out.push_str(key);
        out.push_str("=\"");
        escape_html_attr(value, &mut out);
        out.push('"');
    }
    out.push('>');
    if let Some(string) = string.filter(|value| !value.is_none()) {
        escape_html_text(string.str()?.to_str()?, &mut out);
    }
    out.push_str("</");
    out.push_str(name);
    out.push('>');
    Ok(out)
}

fn string_subclass_name(subclass: Option<&Bound<'_, PyAny>>) -> Option<String> {
    subclass
        .filter(|value| !value.is_none())
        .and_then(|value| value.getattr("__name__").ok())
        .and_then(|name| name.extract::<String>().ok())
}

fn normalize_kwarg_attr_name(name: &str) -> String {
    if name == "class_" {
        "class".to_string()
    } else {
        name.to_string()
    }
}

fn matches_name_filter(
    filter: Option<&Bound<'_, PyAny>>,
    document: &SharedDocument,
    id: NodeId,
) -> PyResult<bool> {
    let Some(filter) = filter else {
        return Ok(true);
    };
    if filter.is_none() {
        return Ok(true);
    }
    if let Ok(flag) = filter.extract::<bool>() {
        return Ok(flag);
    }
    if is_sequence_filter(filter) {
        for item in filter.try_iter()? {
            if matches_name_filter(Some(&item?), document, id)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    let tag_name = {
        let document = read_document(document);
        match &document.node(id).node_type {
            NodeType::Document => "[document]".to_string(),
            NodeType::Element(element) => element.tag_name().to_string(),
            _ => return Ok(false),
        }
    };
    if let Ok(value) = filter.extract::<String>() {
        return Ok(tag_name == value.to_ascii_lowercase());
    }
    if has_search(filter)? {
        return filter
            .getattr("search")?
            .call1((tag_name.as_str(),))?
            .is_truthy();
    }
    if filter.is_callable() {
        let tag = Tag::new(Arc::clone(document), id);
        return filter.call1((tag,))?.is_truthy();
    }
    Ok(false)
}

fn matches_attr_filters(
    py: Python<'_>,
    document: &SharedDocument,
    id: NodeId,
    filters: &[(String, Bound<'_, PyAny>)],
) -> PyResult<bool> {
    for (name, filter) in filters {
        let value = read_document(document)
            .attr(id, name)
            .map(ToString::to_string);
        if !matches_value_filter(py, value.as_deref(), filter, name == "class")? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn matches_value_filter(
    py: Python<'_>,
    candidate: Option<&str>,
    filter: &Bound<'_, PyAny>,
    is_class: bool,
) -> PyResult<bool> {
    let _ = py;
    if filter.is_none() {
        return Ok(candidate.is_none());
    }
    if let Ok(flag) = filter.extract::<bool>() {
        return Ok(if flag {
            candidate.is_some()
        } else {
            candidate.is_none()
        });
    }
    if is_sequence_filter(filter) {
        for item in filter.try_iter()? {
            if matches_value_filter(py, candidate, &item?, is_class)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if has_search(filter)? {
        let Some(candidate) = candidate else {
            return Ok(false);
        };
        return filter.getattr("search")?.call1((candidate,))?.is_truthy();
    }
    if filter.is_callable() {
        return filter.call1((candidate,))?.is_truthy();
    }
    let expected = filter.str()?.to_str()?.to_string();
    let Some(candidate) = candidate else {
        return Ok(false);
    };
    if is_class && expected.split_ascii_whitespace().count() == 1 {
        Ok(candidate
            .split_ascii_whitespace()
            .any(|class| class == expected))
    } else {
        Ok(candidate == expected)
    }
}

fn matches_string_node_filter(
    py: Python<'_>,
    document: &SharedDocument,
    id: NodeId,
    candidate: Option<&str>,
    filter: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    if filter.is_none() {
        return Ok(candidate.is_none());
    }
    if let Ok(flag) = filter.extract::<bool>() {
        return Ok(if flag {
            candidate.is_some()
        } else {
            candidate.is_none()
        });
    }
    if is_sequence_filter(filter) {
        for item in filter.try_iter()? {
            if matches_string_node_filter(py, document, id, candidate, &item?)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if filter.is_callable() {
        return filter.call1((node_to_py(py, document, id)?,))?.is_truthy();
    }
    matches_value_filter(py, candidate, filter, false)
}

pub(crate) struct TextTypeSelection {
    pub is_default: bool,
    pub include_text: bool,
    pub include_cdata: bool,
    pub include_declaration: bool,
    pub include_template: bool,
    pub include_comments: bool,
    pub include_script: bool,
    pub include_stylesheet: bool,
    pub include_raw_text: bool,
    pub include_doctype: bool,
    pub include_processing_instruction: bool,
    pub include_root_raw_text: bool,
}

impl TextTypeSelection {
    fn default_text() -> Self {
        Self {
            is_default: true,
            include_text: true,
            include_cdata: true,
            include_declaration: false,
            include_template: false,
            include_comments: false,
            include_script: false,
            include_stylesheet: false,
            include_raw_text: false,
            include_doctype: false,
            include_processing_instruction: false,
            include_root_raw_text: true,
        }
    }
}

pub(crate) fn text_type_selection_from_call(
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<TextTypeSelection> {
    if args.len() > 1 {
        return Err(PyTypeError::new_err(
            "get_text() takes from 1 to 4 positional arguments",
        ));
    }

    let mut kw_types = None;
    if let Some(kwargs) = kwargs {
        for (key, value) in kwargs.iter() {
            let key = key.extract::<String>()?;
            if key == "types" {
                kw_types = Some(value);
            } else {
                return Err(PyTypeError::new_err(format!(
                    "get_text() got an unexpected keyword argument '{key}'"
                )));
            }
        }
    }
    if !args.is_empty() && kw_types.is_some() {
        return Err(PyTypeError::new_err(
            "get_text() got multiple values for argument 'types'",
        ));
    }

    if let Some(types) = kw_types {
        return text_type_selection_from_value(&types);
    }
    if !args.is_empty() {
        let types = args.get_item(0)?;
        return text_type_selection_from_value(&types);
    }
    Ok(TextTypeSelection::default_text())
}

fn text_type_selection_from_value(types: &Bound<'_, PyAny>) -> PyResult<TextTypeSelection> {
    if types.is_none() {
        return Ok(TextTypeSelection {
            is_default: false,
            include_text: true,
            include_cdata: true,
            include_declaration: true,
            include_template: true,
            include_comments: true,
            include_script: true,
            include_stylesheet: true,
            include_raw_text: true,
            include_doctype: true,
            include_processing_instruction: true,
            include_root_raw_text: true,
        });
    }

    if is_sequence_filter(types) {
        let mut selection = TextTypeSelection {
            is_default: false,
            include_text: false,
            include_cdata: false,
            include_declaration: false,
            include_template: false,
            include_comments: false,
            include_script: false,
            include_stylesheet: false,
            include_raw_text: false,
            include_doctype: false,
            include_processing_instruction: false,
            include_root_raw_text: false,
        };
        let mut saw_type = false;
        for item in types.try_iter()? {
            saw_type = true;
            include_text_type(&mut selection, &item?);
        }
        return Ok(if saw_type {
            selection
        } else {
            TextTypeSelection::default_text()
        });
    }

    let mut selection = TextTypeSelection {
        is_default: false,
        include_text: false,
        include_cdata: false,
        include_declaration: false,
        include_template: false,
        include_comments: false,
        include_script: false,
        include_stylesheet: false,
        include_raw_text: false,
        include_doctype: false,
        include_processing_instruction: false,
        include_root_raw_text: false,
    };
    include_text_type(&mut selection, types);
    Ok(selection)
}

fn include_text_type(selection: &mut TextTypeSelection, value: &Bound<'_, PyAny>) {
    let Ok(name) = value
        .getattr("__name__")
        .and_then(|name| name.extract::<String>())
    else {
        return;
    };
    match name.as_str() {
        "NavigableString" | "_NavigableString" => selection.include_text = true,
        "CData" => selection.include_cdata = true,
        "Declaration" => selection.include_declaration = true,
        "TemplateString" | "RubyTextString" | "RubyParenthesisString" => {
            selection.include_template = true;
        }
        "PreformattedString" => {
            selection.include_cdata = true;
            selection.include_declaration = true;
            selection.include_processing_instruction = true;
        }
        "Comment" => selection.include_comments = true,
        "Script" => selection.include_script = true,
        "Stylesheet" => selection.include_stylesheet = true,
        "Doctype" => selection.include_doctype = true,
        "ProcessingInstruction" => selection.include_processing_instruction = true,
        _ => {}
    }
}

fn is_sequence_filter(value: &Bound<'_, PyAny>) -> bool {
    value.cast::<PyList>().is_ok()
        || value.cast::<PyTuple>().is_ok()
        || value.cast::<PySet>().is_ok()
}

fn has_search(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(value.hasattr("search")? && value.getattr("search")?.is_callable())
}

fn escape_html_text(input: &str, out: &mut String) {
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

fn escape_html_attr(input: &str, out: &mut String) {
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}
