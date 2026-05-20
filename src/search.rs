use crate::attrs::Attr;
use crate::dom::{Document, NodeId, NodeType};
use crate::python::{element_nodes_to_py, node_to_py, nodes_to_py};
use crate::shared::{SharedDocument, read_document};
use crate::tag::Tag;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList, PySet, PyTuple};
use std::sync::Arc;

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
    nodes_to_py(py, document, nodes)
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
    if let Some(id) = try_fast_find_first(document, root, name, attrs, recursive, string, kwargs)? {
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
        document, root, name, attrs, recursive, string, limit, kwargs,
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
    let Some(attr_filters) = SimpleAttrFilter::from_filters(&attr_filters)? else {
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
    let Some(attr_filters) = SimpleAttrFilter::from_filters(&attr_filters)? else {
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
    let Some(attr_filters) = SimpleAttrFilter::from_filters(&attr_filters)? else {
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
    fn from_filters(filters: &[(String, Bound<'_, PyAny>)]) -> PyResult<Option<Vec<Self>>> {
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
fn find_all_compat_node_ids_in_nodes(
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

#[derive(Clone, Copy)]
pub(crate) enum RelativeSearch {
    NextElements,
    PreviousElements,
    Parents,
    NextSiblings,
    PreviousSiblings,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn find_first_compat_relative_node(
    py: Python<'_>,
    document: &SharedDocument,
    id: NodeId,
    axis: RelativeSearch,
    name: Option<&Bound<'_, PyAny>>,
    attrs: Option<&Bound<'_, PyAny>>,
    string: Option<&Bound<'_, PyAny>>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Option<Py<PyAny>>> {
    Ok(find_all_compat_relative_nodes(
        py,
        document,
        id,
        axis,
        name,
        attrs,
        string,
        Some(1),
        kwargs,
    )?
    .into_iter()
    .next())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn find_all_compat_relative_nodes(
    py: Python<'_>,
    document: &SharedDocument,
    id: NodeId,
    axis: RelativeSearch,
    name: Option<&Bound<'_, PyAny>>,
    attrs: Option<&Bound<'_, PyAny>>,
    string: Option<&Bound<'_, PyAny>>,
    limit: Option<usize>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Vec<Py<PyAny>>> {
    match axis {
        RelativeSearch::NextElements | RelativeSearch::PreviousElements => {
            find_all_compat_document_order_nodes(
                py, document, id, axis, name, attrs, string, limit, kwargs,
            )
        }
        RelativeSearch::Parents
        | RelativeSearch::NextSiblings
        | RelativeSearch::PreviousSiblings => find_all_compat_relative_stream(
            py, document, id, axis, name, attrs, string, limit, kwargs,
        ),
    }
}

impl RelativeSearch {
    fn next_after(self, document: &Document, id: NodeId) -> Option<NodeId> {
        match self {
            Self::NextElements => document.next_element_node(id),
            Self::PreviousElements => document.previous_element_node(id),
            Self::Parents => document.node(id).parent,
            Self::NextSiblings => document.node(id).next_sibling,
            Self::PreviousSiblings => document.node(id).prev_sibling,
        }
    }

    fn document_order_nodes(self, document: &Document, id: NodeId) -> Option<Vec<NodeId>> {
        match self {
            Self::NextElements => Some(document.next_element_nodes(id)),
            Self::PreviousElements => Some(document.previous_element_nodes(id)),
            Self::Parents | Self::NextSiblings | Self::PreviousSiblings => None,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn find_all_compat_document_order_nodes(
    py: Python<'_>,
    document: &SharedDocument,
    id: NodeId,
    axis: RelativeSearch,
    name: Option<&Bound<'_, PyAny>>,
    attrs: Option<&Bound<'_, PyAny>>,
    string: Option<&Bound<'_, PyAny>>,
    limit: Option<usize>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Vec<Py<PyAny>>> {
    if let Some(nodes) =
        try_fast_find_all_document_order(document, id, axis, name, attrs, string, limit, kwargs)?
    {
        return element_nodes_to_py(py, document, nodes);
    }

    if limit.is_none_or(|value| value == 0) {
        let candidates = {
            let document = read_document(document);
            axis.document_order_nodes(&document, id)
        };
        if let Some(candidates) = candidates {
            let nodes = find_all_compat_node_ids_in_nodes(
                py, document, candidates, name, attrs, string, limit, kwargs,
            )?;
            return nodes_to_py(py, document, nodes);
        }
    }

    find_all_compat_relative_stream(py, document, id, axis, name, attrs, string, limit, kwargs)
}

#[allow(clippy::too_many_arguments)]
fn try_fast_find_all_document_order(
    document: &SharedDocument,
    id: NodeId,
    axis: RelativeSearch,
    name: Option<&Bound<'_, PyAny>>,
    attrs: Option<&Bound<'_, PyAny>>,
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
    let Some(attr_filters) = SimpleAttrFilter::from_filters(&attr_filters)? else {
        return Ok(None);
    };

    let document = read_document(document);
    let mut out = Vec::new();
    let mut candidate = axis.next_after(&document, id);
    while let Some(current) = candidate {
        if fast_matches(&document, current, &name_filter, &attr_filters) {
            out.push(current);
            if limit.is_some_and(|value| value > 0 && out.len() >= value) {
                break;
            }
        }
        candidate = axis.next_after(&document, current);
    }

    Ok(Some(out))
}

#[allow(clippy::too_many_arguments)]
fn find_all_compat_relative_stream(
    py: Python<'_>,
    document: &SharedDocument,
    id: NodeId,
    axis: RelativeSearch,
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
    let mut candidate = {
        let document = read_document(document);
        axis.next_after(&document, id)
    };

    while let Some(current) = candidate {
        candidate = {
            let document = read_document(document);
            axis.next_after(&document, current)
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

pub(crate) fn normalize_kwarg_attr_name(name: &str) -> String {
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

pub(crate) fn is_sequence_filter(value: &Bound<'_, PyAny>) -> bool {
    value.cast::<PyList>().is_ok()
        || value.cast::<PyTuple>().is_ok()
        || value.cast::<PySet>().is_ok()
}

pub(crate) fn has_search(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(value.hasattr("search")? && value.getattr("search")?.is_callable())
}
