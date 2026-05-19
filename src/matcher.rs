use crate::attrs::Attr;
use crate::dom::{Document, NodeId};
use crate::errors::invalid_selector;
use crate::selectors;
use pyo3::PyResult;
use smallvec::SmallVec;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static SELECTOR_CACHE: OnceLock<Mutex<HashMap<String, selectors::CompiledSelector>>> =
    OnceLock::new();
const SELECTOR_CACHE_MAX_ENTRIES: usize = 1024;

#[derive(Clone, Debug, Default)]
pub struct FindCriteria {
    pub name: Option<String>,
}

impl FindCriteria {
    pub fn with_name(name: Option<&str>) -> Self {
        Self {
            name: name.map(|value| value.to_ascii_lowercase()),
        }
    }
}

pub fn matches_find_criteria(document: &Document, id: NodeId, criteria: &FindCriteria) -> bool {
    let Some(element) = document.element(id) else {
        return false;
    };

    if let Some(name) = &criteria.name
        && element.tag_name() != name
    {
        return false;
    }

    true
}

pub fn find_first(
    document: &Document,
    root: NodeId,
    include_root: bool,
    criteria: &FindCriteria,
) -> Option<NodeId> {
    document.find_descendant_element(root, include_root, |id| {
        matches_find_criteria(document, id, criteria)
    })
}

pub fn select_all(
    document: &Document,
    root: NodeId,
    include_root: bool,
    selector: &str,
    limit: usize,
) -> PyResult<Vec<NodeId>> {
    let mut out = Vec::new();
    select_all_into(document, root, include_root, selector, limit, |id| {
        out.push(id);
        Ok(())
    })?;
    Ok(out)
}

pub fn select_all_into(
    document: &Document,
    root: NodeId,
    include_root: bool,
    selector: &str,
    limit: usize,
    mut on_match: impl FnMut(NodeId) -> PyResult<()>,
) -> PyResult<()> {
    let selector = expand_selector_aliases(selector).map_err(|()| invalid_selector(selector))?;
    let selector = selector.as_ref();
    if let Some(simple) = FastSelector::parse(selector) {
        return simple.select_into(document, root, include_root, limit, &mut on_match);
    }
    if let Some(chain) = FastSelectorChain::parse(selector) {
        return chain.select_into(document, root, include_root, limit, &mut on_match);
    }

    let not_has = extract_not_has_filter(selector).map_err(|()| invalid_selector(selector))?;
    let selector_after_not_has = not_has
        .as_ref()
        .map(|filter| filter.selector.as_str())
        .unwrap_or(selector);
    let has =
        extract_has_filter(selector_after_not_has).map_err(|()| invalid_selector(selector))?;
    let selector_after_has = has
        .as_ref()
        .map(|filter| filter.selector.as_str())
        .unwrap_or(selector_after_not_has);
    let contains =
        extract_contains_filter(selector_after_has).map_err(|()| invalid_selector(selector))?;
    let selector_to_compile = contains
        .as_ref()
        .map(|filter| filter.selector.as_str())
        .unwrap_or(selector_after_has);
    let compiled = compile_cached(selector_to_compile).map_err(|()| invalid_selector(selector))?;
    let mut count = 0usize;
    let mut current = if include_root {
        Some(root)
    } else {
        document.node(root).first_child
    };
    while let Some(id) = current {
        let next = document.next_in_subtree(root, id);
        if !document.is_element(id) {
            current = next;
            continue;
        }
        if !selectors::matches(document, id, &compiled) {
            current = next;
            continue;
        }
        if contains
            .as_ref()
            .is_some_and(|filter| !filter.matches(document, id))
        {
            current = next;
            continue;
        }
        if let Some(filter) = &has
            && !filter.matches(document, id)?
        {
            current = next;
            continue;
        }
        if let Some(filter) = &not_has
            && !filter.matches(document, id)?
        {
            current = next;
            continue;
        }
        on_match(id)?;
        count += 1;
        if limit > 0 && count >= limit {
            break;
        }
        current = next;
    }
    Ok(())
}

pub fn matches_selector(document: &Document, id: NodeId, selector: &str) -> PyResult<bool> {
    if !document.is_element(id) {
        return Ok(false);
    }
    let selector = expand_selector_aliases(selector).map_err(|()| invalid_selector(selector))?;
    let selector = selector.as_ref();
    if let Some(simple) = FastSelector::parse(selector) {
        return Ok(simple.matches(document, id));
    }
    if let Some(chain) = FastSelectorChain::parse(selector) {
        return Ok(chain.matches(document, id, document.root));
    }

    let not_has = extract_not_has_filter(selector).map_err(|()| invalid_selector(selector))?;
    let selector_after_not_has = not_has
        .as_ref()
        .map(|filter| filter.selector.as_str())
        .unwrap_or(selector);
    let has =
        extract_has_filter(selector_after_not_has).map_err(|()| invalid_selector(selector))?;
    let selector_after_has = has
        .as_ref()
        .map(|filter| filter.selector.as_str())
        .unwrap_or(selector_after_not_has);
    let contains =
        extract_contains_filter(selector_after_has).map_err(|()| invalid_selector(selector))?;
    let selector_to_compile = contains
        .as_ref()
        .map(|filter| filter.selector.as_str())
        .unwrap_or(selector_after_has);
    let compiled = compile_cached(selector_to_compile).map_err(|()| invalid_selector(selector))?;
    if !selectors::matches(document, id, &compiled) {
        return Ok(false);
    }
    if contains
        .as_ref()
        .is_some_and(|filter| !filter.matches(document, id))
    {
        return Ok(false);
    }
    if let Some(filter) = &has
        && !filter.matches(document, id)?
    {
        return Ok(false);
    }
    if let Some(filter) = &not_has
        && !filter.matches(document, id)?
    {
        return Ok(false);
    }
    Ok(true)
}

fn expand_selector_aliases(selector: &str) -> Result<Cow<'_, str>, ()> {
    let mut selectors = vec![selector.to_string()];
    let mut changed = false;

    for _ in 0..16 {
        let mut expanded = Vec::new();
        let mut changed_this_round = false;

        for selector in selectors {
            let Some((start, end, inner)) = find_selector_alias_call(&selector)? else {
                expanded.push(selector);
                continue;
            };
            let branches = split_selector_list(&inner)?;
            for branch in branches {
                let mut next = String::with_capacity(selector.len() + branch.len());
                next.push_str(&selector[..start]);
                next.push_str(branch);
                next.push_str(&selector[end..]);
                expanded.push(next);
            }
            changed = true;
            changed_this_round = true;
        }

        selectors = expanded;
        if !changed_this_round {
            return Ok(if changed {
                Cow::Owned(selectors.join(", "))
            } else {
                Cow::Borrowed(selector)
            });
        }
    }

    Err(())
}

fn find_selector_alias_call(selector: &str) -> Result<Option<(usize, usize, String)>, ()> {
    let mut quote = None;
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut index = 0usize;

    while index < selector.len() {
        let rest = &selector[index..];
        let ch = rest.chars().next().ok_or(())?;

        if let Some(quote_char) = quote {
            index += ch.len_utf8();
            if ch == quote_char {
                quote = None;
            }
            continue;
        }

        if bracket_depth == 0 && paren_depth == 0 {
            for prefix in [":is(", ":where(", ":matches("] {
                if rest.starts_with(prefix) {
                    let (end, inner) = parse_pseudo_call(selector, index, prefix)?;
                    return Ok(Some((index, end, inner)));
                }
            }
        }

        match ch {
            '"' | '\'' => {
                quote = Some(ch);
            }
            '[' => {
                bracket_depth += 1;
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
            }
            '(' if bracket_depth == 0 => {
                paren_depth += 1;
            }
            ')' if bracket_depth == 0 => {
                paren_depth = paren_depth.saturating_sub(1);
            }
            _ => {}
        }
        index += ch.len_utf8();
    }

    if quote.is_some() || bracket_depth != 0 || paren_depth != 0 {
        return Err(());
    }
    Ok(None)
}

fn split_selector_list(selector: &str) -> Result<Vec<&str>, ()> {
    let mut parts = Vec::new();
    let mut quote = None;
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut start = 0usize;
    let mut index = 0usize;

    while index < selector.len() {
        let ch = selector[index..].chars().next().ok_or(())?;

        if let Some(quote_char) = quote {
            index += ch.len_utf8();
            if ch == quote_char {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                quote = Some(ch);
            }
            '[' => {
                bracket_depth += 1;
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
            }
            '(' if bracket_depth == 0 => {
                paren_depth += 1;
            }
            ')' if bracket_depth == 0 => {
                paren_depth = paren_depth.saturating_sub(1);
            }
            ',' if bracket_depth == 0 && paren_depth == 0 => {
                let part = selector[start..index].trim();
                if part.is_empty() {
                    return Err(());
                }
                parts.push(part);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
        index += ch.len_utf8();
    }

    if quote.is_some() || bracket_depth != 0 || paren_depth != 0 {
        return Err(());
    }
    let part = selector[start..].trim();
    if part.is_empty() {
        return Err(());
    }
    parts.push(part);
    Ok(parts)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FastSelector<'a> {
    tag_name: Option<&'a str>,
    id: Option<&'a str>,
    classes: SmallVec<[&'a str; 2]>,
    attrs: SmallVec<[FastAttrSelector<'a>; 2]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FastAttrSelector<'a> {
    Exists(&'a str),
    Exact(&'a str, &'a str),
}

impl<'a> FastSelector<'a> {
    fn parse(selector: &'a str) -> Option<Self> {
        let selector = selector.trim();
        if selector.is_empty()
            || selector.as_bytes().iter().any(|byte| {
                matches!(
                    byte,
                    b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'+' | b'~' | b',' | b':'
                )
            })
        {
            return None;
        }

        let mut index = 0;
        let mut tag_name = None;
        let mut id = None;
        let mut classes = SmallVec::new();
        let mut attrs = SmallVec::new();

        if selector.as_bytes().first().is_some_and(is_ident_start) {
            let (name, next) = parse_identifier(selector, index)?;
            if contains_ascii_uppercase(name) {
                return None;
            }
            tag_name = Some(name);
            index = next;
        } else if selector.as_bytes().first() == Some(&b'*') {
            index = 1;
        }

        while index < selector.len() {
            match selector.as_bytes()[index] {
                b'.' => {
                    let (class_name, next) = parse_identifier(selector, index + 1)?;
                    classes.push(class_name);
                    index = next;
                }
                b'#' => {
                    if id.is_some() {
                        return None;
                    }
                    let (id_value, next) = parse_identifier(selector, index + 1)?;
                    id = Some(id_value);
                    index = next;
                }
                b'[' => {
                    let (attr, next) = parse_attr_selector(selector, index)?;
                    attrs.push(attr);
                    index = next;
                }
                _ => return None,
            }
        }

        Some(Self {
            tag_name,
            id,
            classes,
            attrs,
        })
    }

    fn select_into(
        &self,
        document: &Document,
        root: NodeId,
        include_root: bool,
        limit: usize,
        on_match: &mut impl FnMut(NodeId) -> PyResult<()>,
    ) -> PyResult<()> {
        let mut count = 0usize;
        let mut current = if include_root {
            Some(root)
        } else {
            document.node(root).first_child
        };

        while let Some(id) = current {
            if self.matches(document, id) {
                on_match(id)?;
                count += 1;
                if limit > 0 && count >= limit {
                    break;
                }
            }
            current = document.next_in_subtree(root, id);
        }
        Ok(())
    }

    fn matches(&self, document: &Document, id: NodeId) -> bool {
        let Some(element) = document.element(id) else {
            return false;
        };
        let attrs = element.attrs.as_ref();
        if let Some(tag_name) = &self.tag_name
            && element.tag_name() != *tag_name
        {
            return false;
        }
        if let Some(expected) = &self.id
            && attr_str(attrs, "id") != Some(*expected)
        {
            return false;
        }
        if self.classes.iter().any(|expected| {
            !attr_str(attrs, "class").is_some_and(|value| {
                value
                    .split_ascii_whitespace()
                    .any(|class_name| class_name == *expected)
            })
        }) {
            return false;
        }
        self.attrs.iter().all(|attr| match attr {
            FastAttrSelector::Exists(name) => attr_present(attrs, name),
            FastAttrSelector::Exact(name, value) => attr_equals(attrs, name, value),
        })
    }
}

#[inline]
fn attr_present(attrs: &[Attr], name: &str) -> bool {
    attrs.iter().any(|attr| attr.name() == name)
}

#[inline]
fn attr_str<'a>(attrs: &'a [Attr], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|attr| attr.name() == name)
        .and_then(|attr| attr.value.as_deref())
}

#[inline]
fn attr_value<'a>(attrs: &'a [Attr], name: &str) -> Option<Option<&'a str>> {
    attrs
        .iter()
        .find(|attr| attr.name() == name)
        .map(|attr| attr.value.as_deref())
}

fn attr_equals(attrs: &[Attr], name: &str, expected: &str) -> bool {
    match attr_value(attrs, name) {
        Some(Some(value)) => value == expected,
        Some(None) => expected.is_empty(),
        None => false,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FastSelectorChain<'a> {
    selectors: SmallVec<[FastSelector<'a>; 4]>,
    combinators: SmallVec<[FastCombinator; 4]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FastCombinator {
    Descendant,
    Child,
}

impl<'a> FastSelectorChain<'a> {
    fn parse(selector: &'a str) -> Option<Self> {
        let selector = selector.trim();
        if selector.is_empty()
            || selector
                .as_bytes()
                .iter()
                .any(|byte| matches!(byte, b'+' | b'~' | b',' | b':'))
        {
            return None;
        }

        let mut parts: SmallVec<[&str; 4]> = SmallVec::new();
        let mut combinators = SmallVec::new();
        let mut pending = FastCombinator::Descendant;
        let mut quote = None;
        let mut bracket_depth = 0usize;
        let mut part_start = None;
        let mut index = 0usize;
        let bytes = selector.as_bytes();

        while index < selector.len() {
            let byte = bytes[index];
            if let Some(quote_byte) = quote {
                if byte == quote_byte {
                    quote = None;
                }
                index += 1;
                continue;
            }

            match byte {
                b'"' | b'\'' if bracket_depth > 0 => {
                    quote = Some(byte);
                    part_start.get_or_insert(index);
                    index += 1;
                }
                b'[' => {
                    bracket_depth += 1;
                    part_start.get_or_insert(index);
                    index += 1;
                }
                b']' if bracket_depth > 0 => {
                    bracket_depth -= 1;
                    index += 1;
                }
                b'>' if bracket_depth == 0 => {
                    push_fast_selector_part(
                        selector,
                        &mut parts,
                        &mut combinators,
                        &mut pending,
                        part_start.take(),
                        index,
                    )?;
                    if parts.is_empty() || pending == FastCombinator::Child {
                        return None;
                    }
                    pending = FastCombinator::Child;
                    index += 1;
                    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
                        index += 1;
                    }
                }
                _ if bracket_depth == 0 && byte.is_ascii_whitespace() => {
                    push_fast_selector_part(
                        selector,
                        &mut parts,
                        &mut combinators,
                        &mut pending,
                        part_start.take(),
                        index,
                    )?;
                    index += 1;
                    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
                        index += 1;
                    }
                    if bytes.get(index) != Some(&b'>') && !parts.is_empty() {
                        pending = FastCombinator::Descendant;
                    }
                }
                _ => {
                    part_start.get_or_insert(index);
                    index += 1;
                }
            }
        }

        if quote.is_some() || bracket_depth != 0 {
            return None;
        }
        push_fast_selector_part(
            selector,
            &mut parts,
            &mut combinators,
            &mut pending,
            part_start.take(),
            selector.len(),
        )?;
        if parts.len() < 2 || pending == FastCombinator::Child {
            return None;
        }

        let selectors = parts
            .into_iter()
            .map(FastSelector::parse)
            .collect::<Option<SmallVec<[FastSelector<'a>; 4]>>>()?;
        Some(Self {
            selectors,
            combinators,
        })
    }

    fn select_into(
        &self,
        document: &Document,
        root: NodeId,
        include_root: bool,
        limit: usize,
        on_match: &mut impl FnMut(NodeId) -> PyResult<()>,
    ) -> PyResult<()> {
        if self.selectors.len() == 2 && self.combinators.as_slice() == [FastCombinator::Descendant]
        {
            return select_descendant_pair_into(
                document,
                root,
                include_root,
                &self.selectors[0],
                &self.selectors[1],
                limit,
                on_match,
            );
        }

        let mut count = 0usize;
        let mut current = if include_root {
            Some(root)
        } else {
            document.node(root).first_child
        };

        while let Some(id) = current {
            if self.matches(document, id, root) {
                on_match(id)?;
                count += 1;
                if limit > 0 && count >= limit {
                    break;
                }
            }
            current = document.next_in_subtree(root, id);
        }
        Ok(())
    }

    fn matches(&self, document: &Document, id: NodeId, root: NodeId) -> bool {
        let Some(last) = self.selectors.last() else {
            return false;
        };
        if !last.matches(document, id) {
            return false;
        }

        let mut current = id;
        for index in (0..self.combinators.len()).rev() {
            match self.combinators[index] {
                FastCombinator::Child => {
                    let Some(parent) = document.parent_element(current) else {
                        return false;
                    };
                    if !self.selectors[index].matches(document, parent) {
                        return false;
                    }
                    current = parent;
                }
                FastCombinator::Descendant => {
                    let Some(parent) =
                        matching_ancestor(document, root, current, &self.selectors[index])
                    else {
                        return false;
                    };
                    current = parent;
                }
            }
        }
        true
    }
}

fn select_descendant_pair_into(
    document: &Document,
    root: NodeId,
    include_root: bool,
    ancestor: &FastSelector<'_>,
    target: &FastSelector<'_>,
    limit: usize,
    on_match: &mut impl FnMut(NodeId) -> PyResult<()>,
) -> PyResult<()> {
    let mut count = 0usize;
    let mut stack: SmallVec<[(NodeId, bool); 64]> = SmallVec::new();
    if include_root {
        stack.push((root, false));
    } else {
        let root_matches_ancestor = ancestor.matches(document, root);
        push_descendant_pair_children(document, root, root_matches_ancestor, &mut stack);
    }

    while let Some((id, has_matching_ancestor)) = stack.pop() {
        let child_has_matching_ancestor = if document.is_element(id) {
            if has_matching_ancestor && target.matches(document, id) {
                on_match(id)?;
                count += 1;
                if limit > 0 && count >= limit {
                    break;
                }
            }
            has_matching_ancestor || ancestor.matches(document, id)
        } else {
            has_matching_ancestor
        };

        push_descendant_pair_children(document, id, child_has_matching_ancestor, &mut stack);
    }

    Ok(())
}

fn push_descendant_pair_children(
    document: &Document,
    parent: NodeId,
    has_matching_ancestor: bool,
    stack: &mut SmallVec<[(NodeId, bool); 64]>,
) {
    let mut child = document.node(parent).last_child;
    while let Some(current) = child {
        stack.push((current, has_matching_ancestor));
        child = document.node(current).prev_sibling;
    }
}

fn push_fast_selector_part<'a>(
    selector: &'a str,
    parts: &mut SmallVec<[&'a str; 4]>,
    combinators: &mut SmallVec<[FastCombinator; 4]>,
    pending: &mut FastCombinator,
    part_start: Option<usize>,
    part_end: usize,
) -> Option<()> {
    let Some(start) = part_start else {
        return Some(());
    };
    let part = selector[start..part_end].trim();
    if part.is_empty() {
        return None;
    }
    if !parts.is_empty() {
        combinators.push(*pending);
    }
    parts.push(part);
    *pending = FastCombinator::Descendant;
    Some(())
}

fn matching_ancestor(
    document: &Document,
    root: NodeId,
    id: NodeId,
    selector: &FastSelector<'_>,
) -> Option<NodeId> {
    let mut ancestor = document.parent_element(id);
    while let Some(parent) = ancestor {
        if selector.matches(document, parent) {
            return Some(parent);
        }
        if parent == root {
            break;
        }
        ancestor = document.parent_element(parent);
    }
    None
}

fn parse_attr_selector<'a>(
    selector: &'a str,
    start: usize,
) -> Option<(FastAttrSelector<'a>, usize)> {
    let mut index = start + 1;
    let (name, next) = parse_identifier(selector, index)?;
    if contains_ascii_uppercase(name) {
        return None;
    }
    index = next;
    if index >= selector.len() {
        return None;
    }
    match selector.as_bytes()[index] {
        b']' => Some((FastAttrSelector::Exists(name), index + 1)),
        b'=' => {
            index += 1;
            let (value, next) = parse_attr_value(selector, index)?;
            index = next;
            (selector.as_bytes().get(index) == Some(&b']'))
                .then_some((FastAttrSelector::Exact(name, value), index + 1))
        }
        _ => None,
    }
}

fn parse_attr_value(selector: &str, start: usize) -> Option<(&str, usize)> {
    if start >= selector.len() {
        return None;
    }
    let bytes = selector.as_bytes();
    if matches!(bytes[start], b'"' | b'\'') {
        let quote = bytes[start];
        let value_start = start + 1;
        let mut index = value_start;
        while index < selector.len() && bytes[index] != quote {
            index += 1;
        }
        return (index < selector.len()).then_some((&selector[value_start..index], index + 1));
    }

    let value_start = start;
    let mut index = start;
    while index < selector.len() && bytes[index] != b']' {
        index += 1;
    }
    (index > value_start).then_some((&selector[value_start..index], index))
}

fn parse_identifier(selector: &str, start: usize) -> Option<(&str, usize)> {
    let mut index = start;
    let bytes = selector.as_bytes();
    if !bytes.get(index).is_some_and(is_ident_start) {
        return None;
    }
    index += 1;
    while bytes.get(index).is_some_and(is_ident_continue) {
        index += 1;
    }
    Some((&selector[start..index], index))
}

fn is_ident_start(byte: &u8) -> bool {
    byte.is_ascii_alphabetic() || *byte == b'_'
}

fn is_ident_continue(byte: &u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit() || *byte == b'-'
}

fn contains_ascii_uppercase(value: &str) -> bool {
    value.as_bytes().iter().any(u8::is_ascii_uppercase)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ContainsFilter {
    selector: String,
    needles: Vec<String>,
}

impl ContainsFilter {
    fn matches(&self, document: &Document, id: NodeId) -> bool {
        let text = document.text(id, "", false);
        self.needles.iter().any(|needle| text.contains(needle))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HasFilter {
    selector: String,
    inner_selector: String,
}

impl HasFilter {
    fn matches(&self, document: &Document, id: NodeId) -> PyResult<bool> {
        has_inner_selector(document, id, &self.inner_selector)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NotHasFilter {
    selector: String,
    inner_selector: String,
}

impl NotHasFilter {
    fn matches(&self, document: &Document, id: NodeId) -> PyResult<bool> {
        Ok(!has_inner_selector(document, id, &self.inner_selector)?)
    }
}

fn has_inner_selector(document: &Document, id: NodeId, inner_selector: &str) -> PyResult<bool> {
    if let Some(child_selector) = inner_selector.trim().strip_prefix('>') {
        let child_selector = child_selector.trim();
        if child_selector.is_empty() {
            return Ok(false);
        }
        let compiled =
            compile_cached(child_selector).map_err(|()| invalid_selector(inner_selector))?;
        let mut child = document.node(id).first_child;
        while let Some(current) = child {
            if document.is_element(current) && selectors::matches(document, current, &compiled) {
                return Ok(true);
            }
            child = document.node(current).next_sibling;
        }
        return Ok(false);
    }
    Ok(!select_all(document, id, false, inner_selector, 1)?.is_empty())
}

fn compile_cached(selector: &str) -> Result<selectors::CompiledSelector, ()> {
    let cache = SELECTOR_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut cache) = cache.lock() else {
        return selectors::compile(selector);
    };
    if let Some(compiled) = cache.get(selector).cloned() {
        return Ok(compiled);
    }

    let compiled = selectors::compile(selector)?;
    if cache.len() >= SELECTOR_CACHE_MAX_ENTRIES {
        cache.clear();
    }
    cache.insert(selector.to_string(), compiled.clone());
    Ok(compiled)
}

fn extract_has_filter(selector: &str) -> Result<Option<HasFilter>, ()> {
    let mut output = String::with_capacity(selector.len());
    let mut inner_selector = None;
    let mut i = 0;
    let mut quote = None;
    let mut bracket_depth = 0usize;

    while i < selector.len() {
        let rest = &selector[i..];
        let ch = rest.chars().next().ok_or(())?;

        if let Some(quote_char) = quote {
            output.push(ch);
            i += ch.len_utf8();
            if ch == quote_char {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                output.push(ch);
                i += ch.len_utf8();
            }
            '[' => {
                bracket_depth += 1;
                output.push(ch);
                i += ch.len_utf8();
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                output.push(ch);
                i += ch.len_utf8();
            }
            _ if bracket_depth == 0 && rest.starts_with(":has(") => {
                if inner_selector.is_some() {
                    return Err(());
                }
                if !current_compound_has_selector(&output) {
                    output.push('*');
                }
                let (next, parsed) = parse_has_call(selector, i)?;
                inner_selector = Some(parsed);
                i = next;
            }
            _ => {
                output.push(ch);
                i += ch.len_utf8();
            }
        }
    }

    if quote.is_some() || bracket_depth != 0 {
        return Err(());
    }
    let Some(inner_selector) = inner_selector else {
        return Ok(None);
    };
    let selector = if output.trim().is_empty() {
        "*".to_string()
    } else {
        output
    };
    Ok(Some(HasFilter {
        selector,
        inner_selector,
    }))
}

fn extract_not_has_filter(selector: &str) -> Result<Option<NotHasFilter>, ()> {
    let mut output = String::with_capacity(selector.len());
    let mut inner_selector = None;
    let mut i = 0;
    let mut quote = None;
    let mut bracket_depth = 0usize;

    while i < selector.len() {
        let rest = &selector[i..];
        let ch = rest.chars().next().ok_or(())?;

        if let Some(quote_char) = quote {
            output.push(ch);
            i += ch.len_utf8();
            if ch == quote_char {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                output.push(ch);
                i += ch.len_utf8();
            }
            '[' => {
                bracket_depth += 1;
                output.push(ch);
                i += ch.len_utf8();
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                output.push(ch);
                i += ch.len_utf8();
            }
            _ if bracket_depth == 0 && rest.starts_with(":not(") => {
                if inner_selector.is_some() {
                    return Err(());
                }
                let (next, parsed_not) = parse_pseudo_call(selector, i, ":not(")?;
                let inner = parsed_not.trim();
                if !inner.starts_with(":has(") {
                    output.push_str(":not(");
                    output.push_str(&parsed_not);
                    output.push(')');
                    i = next;
                    continue;
                }
                let (has_end, parsed_has) = parse_has_call(inner, 0)?;
                if has_end != inner.len() {
                    output.push_str(":not(");
                    output.push_str(&parsed_not);
                    output.push(')');
                    i = next;
                    continue;
                }
                if !current_compound_has_selector(&output) {
                    output.push('*');
                }
                inner_selector = Some(parsed_has);
                i = next;
            }
            _ => {
                output.push(ch);
                i += ch.len_utf8();
            }
        }
    }

    if quote.is_some() || bracket_depth != 0 {
        return Err(());
    }
    let Some(inner_selector) = inner_selector else {
        return Ok(None);
    };
    let selector = if output.trim().is_empty() {
        "*".to_string()
    } else {
        output
    };
    Ok(Some(NotHasFilter {
        selector,
        inner_selector,
    }))
}

fn extract_contains_filter(selector: &str) -> Result<Option<ContainsFilter>, ()> {
    let mut output = String::with_capacity(selector.len());
    let mut needles = Vec::new();
    let mut found = false;
    let mut i = 0;
    let mut quote = None;
    let mut bracket_depth = 0usize;

    while i < selector.len() {
        let rest = &selector[i..];
        let ch = rest.chars().next().ok_or(())?;

        if let Some(quote_char) = quote {
            output.push(ch);
            i += ch.len_utf8();
            if ch == quote_char {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                output.push(ch);
                i += ch.len_utf8();
            }
            '[' => {
                bracket_depth += 1;
                output.push(ch);
                i += ch.len_utf8();
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                output.push(ch);
                i += ch.len_utf8();
            }
            _ if bracket_depth == 0 && rest.starts_with(":-soup-contains(") => {
                found = true;
                if !current_compound_has_selector(&output) {
                    output.push('*');
                }
                let (next, mut parsed) = parse_contains_call(selector, i, ":-soup-contains(")?;
                needles.append(&mut parsed);
                i = next;
            }
            _ if bracket_depth == 0 && rest.starts_with(":contains(") => {
                found = true;
                if !current_compound_has_selector(&output) {
                    output.push('*');
                }
                let (next, mut parsed) = parse_contains_call(selector, i, ":contains(")?;
                needles.append(&mut parsed);
                i = next;
            }
            _ => {
                output.push(ch);
                i += ch.len_utf8();
            }
        }
    }

    if quote.is_some() || bracket_depth != 0 {
        return Err(());
    }
    if !found {
        return Ok(None);
    }
    if needles.is_empty() {
        return Err(());
    }
    let selector = if output.trim().is_empty() {
        "*".to_string()
    } else {
        output
    };
    Ok(Some(ContainsFilter { selector, needles }))
}

fn parse_has_call(selector: &str, start: usize) -> Result<(usize, String), ()> {
    parse_pseudo_call(selector, start, ":has(")
}

fn parse_pseudo_call(selector: &str, start: usize, prefix: &str) -> Result<(usize, String), ()> {
    if !selector[start..].starts_with(prefix) {
        return Err(());
    }
    let args_start = start + prefix.len();
    let mut quote = None;
    let mut escape = false;
    let mut depth = 1usize;
    let mut i = args_start;

    while i < selector.len() {
        let ch = selector[i..].chars().next().ok_or(())?;
        i += ch.len_utf8();
        if escape {
            escape = false;
            continue;
        }
        if ch == '\\' {
            escape = true;
            continue;
        }
        if let Some(quote_char) = quote {
            if ch == quote_char {
                quote = None;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }
        if ch == '(' {
            depth += 1;
            continue;
        }
        if ch == ')' {
            depth -= 1;
            if depth == 0 {
                let end = i - ch.len_utf8();
                let inner = selector[args_start..end].trim();
                if inner.is_empty() {
                    return Err(());
                }
                return Ok((i, inner.to_string()));
            }
        }
    }
    Err(())
}

fn current_compound_has_selector(output: &str) -> bool {
    if let Some(ch) = output.chars().next_back() {
        if ch.is_whitespace() || matches!(ch, '>' | '+' | '~' | ',') {
            return false;
        }
        return true;
    }
    false
}

fn parse_contains_call(
    selector: &str,
    start: usize,
    prefix: &str,
) -> Result<(usize, Vec<String>), ()> {
    let args_start = start + prefix.len();
    let mut quote = None;
    let mut escape = false;
    let mut end = None;
    let mut i = args_start;

    while i < selector.len() {
        let ch = selector[i..].chars().next().ok_or(())?;
        i += ch.len_utf8();
        if escape {
            escape = false;
            continue;
        }
        if ch == '\\' {
            escape = true;
            continue;
        }
        if let Some(quote_char) = quote {
            if ch == quote_char {
                quote = None;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }
        if ch == ')' {
            end = Some(i - ch.len_utf8());
            break;
        }
    }

    let end = end.ok_or(())?;
    let args = parse_contains_args(&selector[args_start..end])?;
    Ok((i, args))
}

fn parse_contains_args(input: &str) -> Result<Vec<String>, ()> {
    let mut values = Vec::new();
    let mut i = 0;

    while i < input.len() {
        skip_css_separators(input, &mut i);
        if i >= input.len() {
            break;
        }

        let ch = input[i..].chars().next().ok_or(())?;
        if ch == '"' || ch == '\'' {
            let (next, value) = parse_css_string(input, i, ch)?;
            values.push(value);
            i = next;
        } else {
            let start = i;
            while i < input.len() {
                let ch = input[i..].chars().next().ok_or(())?;
                if ch == ',' {
                    break;
                }
                i += ch.len_utf8();
            }
            let value = input[start..i].trim();
            if value.is_empty() {
                return Err(());
            }
            values.push(value.to_string());
        }
        skip_css_separators(input, &mut i);
    }

    if values.is_empty() {
        Err(())
    } else {
        Ok(values)
    }
}

fn skip_css_separators(input: &str, i: &mut usize) {
    while *i < input.len() {
        let Some(ch) = input[*i..].chars().next() else {
            break;
        };
        if ch == ',' || ch.is_whitespace() {
            *i += ch.len_utf8();
        } else {
            break;
        }
    }
}

fn parse_css_string(input: &str, start: usize, quote: char) -> Result<(usize, String), ()> {
    let mut value = String::new();
    let mut i = start + quote.len_utf8();
    let mut escape = false;

    while i < input.len() {
        let ch = input[i..].chars().next().ok_or(())?;
        i += ch.len_utf8();
        if escape {
            value.push(ch);
            escape = false;
            continue;
        }
        if ch == '\\' {
            escape = true;
            continue;
        }
        if ch == quote {
            return Ok((i, value));
        }
        value.push(ch);
    }
    Err(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_selector_cache() {
        SELECTOR_CACHE
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .clear();
    }

    fn selector_cache_len() -> usize {
        SELECTOR_CACHE
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .len()
    }

    #[test]
    fn selector_cache_is_bounded() {
        clear_selector_cache();

        for index in 0..SELECTOR_CACHE_MAX_ENTRIES {
            compile_cached(&format!("#cache-{index}")).unwrap();
        }
        assert_eq!(selector_cache_len(), SELECTOR_CACHE_MAX_ENTRIES);

        compile_cached("#cache-overflow").unwrap();
        assert_eq!(selector_cache_len(), 1);
    }
}
