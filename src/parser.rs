use crate::attrs::{Attr, dedupe_attrs_last_wins};
use crate::dom::{
    DoctypeData, Document, ElementData, Node, NodeId, NodeType, ProcessingInstructionData,
    html_namespace, is_html_namespace, is_raw_text_element, is_void_element,
};
use compact_str::CompactString;
use html5ever::interface::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::tree_builder::TreeBuilderOpts;
use html5ever::{Attribute, ParseOpts, QualName, parse_document, parse_fragment};
use markup5ever::{LocalName, Namespace, local_name, ns};
use memchr::memchr_iter;
use std::borrow::Cow;
use std::cell::{Cell, UnsafeCell};
use std::collections::{HashMap, HashSet};

pub fn parse_html(html: &str) -> Document {
    if let Some(document) = try_parse_fast_full_document(html) {
        return document;
    }
    if let Some(document) = try_parse_fast_fragment(html) {
        return document;
    }

    let hints = inspect_source(html);
    let html = if hints.preserve_newline_candidate {
        preserve_bs4_leading_newlines(html)
    } else {
        Cow::Borrowed(html)
    };
    let html = html.as_ref();
    if hints.looks_like_full_document {
        let mut document = parse_document(Sink::new(html), parse_opts()).one(html);
        if !hints.has_head {
            document.remove_first_empty_head();
        }
        document.promote_bogus_comments_to_processing_instructions(&hints.processing_instructions);
        document
    } else {
        let mut document = parse_fragment(
            Sink::new(html),
            parse_opts(),
            QualName::new(None, ns!(html), local_name!("body")),
            Vec::new(),
            false,
        )
        .one(html);
        document.unwrap_single_child_element_named("html");
        document.promote_bogus_comments_to_processing_instructions(&hints.processing_instructions);
        document
    }
}

pub fn parse_html_document(html: &str) -> Document {
    if let Some(document) = try_parse_fast_full_document(html) {
        return document;
    }

    let hints = inspect_source(html);
    let html = if hints.preserve_newline_candidate {
        preserve_bs4_leading_newlines(html)
    } else {
        Cow::Borrowed(html)
    };
    let html = html.as_ref();
    let mut document = parse_document(Sink::new(html), parse_opts()).one(html);
    if !hints.has_head {
        document.remove_first_empty_head();
    }
    document.promote_bogus_comments_to_processing_instructions(&hints.processing_instructions);
    document
}

fn starts_with_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .get(..needle.len())
        .is_some_and(|window| window.eq_ignore_ascii_case(needle))
}

struct SourceHints {
    looks_like_full_document: bool,
    has_head: bool,
    preserve_newline_candidate: bool,
    processing_instructions: Vec<String>,
}

fn inspect_source(html: &str) -> SourceHints {
    let bytes = html.as_bytes();
    let trimmed = html.trim_start();
    let has_processing_instructions = memchr::memmem::find(bytes, b"<?").is_some();
    let mut hints = SourceHints {
        looks_like_full_document: starts_with_ascii_case_insensitive(trimmed.as_bytes(), b"<html")
            || starts_with_ascii_case_insensitive(trimmed.as_bytes(), b"<!doctype"),
        has_head: false,
        preserve_newline_candidate: false,
        processing_instructions: Vec::new(),
    };

    let mut next_processing_instruction_start = 0;
    for open in memchr_iter(b'<', bytes) {
        let rest = &bytes[open..];
        if !hints.looks_like_full_document
            && (starts_with_ascii_case_insensitive(rest, b"<html")
                || starts_with_ascii_case_insensitive(rest, b"<!doctype"))
        {
            hints.looks_like_full_document = true;
        }
        if !hints.has_head && starts_with_ascii_case_insensitive(rest, b"<head") {
            hints.has_head = true;
        }
        if !hints.preserve_newline_candidate {
            let after_lt = &bytes[open + 1..];
            match after_lt.first().map(u8::to_ascii_lowercase) {
                Some(b'p') if starts_with_ascii_case_insensitive(after_lt, b"pre") => {
                    hints.preserve_newline_candidate = true;
                }
                Some(b't') if starts_with_ascii_case_insensitive(after_lt, b"textarea") => {
                    hints.preserve_newline_candidate = true;
                }
                _ => {}
            }
        }
        if has_processing_instructions
            && open >= next_processing_instruction_start
            && bytes.get(open + 1) == Some(&b'?')
        {
            let value_start = open + 2;
            let Some(close) = bytes[value_start..]
                .iter()
                .position(|byte| *byte == b'>')
                .map(|position| value_start + position)
            else {
                next_processing_instruction_start = usize::MAX;
                continue;
            };
            hints
                .processing_instructions
                .push(html[value_start..close].to_string());
            next_processing_instruction_start = close + 1;
        }
        if !has_processing_instructions
            && hints.looks_like_full_document
            && hints.has_head
            && hints.preserve_newline_candidate
        {
            break;
        }
    }

    hints
}

fn parse_opts() -> ParseOpts {
    ParseOpts {
        tree_builder: TreeBuilderOpts {
            scripting_enabled: false,
            ..TreeBuilderOpts::default()
        },
        ..ParseOpts::default()
    }
}

fn preserve_bs4_leading_newlines(html: &str) -> Cow<'_, str> {
    let bytes = html.as_bytes();
    let mut index = 0;
    let mut copied_until = 0;
    let mut out: Option<String> = None;

    while let Some(open_offset) = memchr::memchr(b'<', &bytes[index..]) {
        let open = index + open_offset;
        let Some(close_offset) = memchr::memchr(b'>', &bytes[open..]) else {
            break;
        };
        let close = open + close_offset;
        let tag = &html[open..=close];
        let after_tag = close + 1;

        if is_preserve_newline_start_tag(tag) && bytes.get(after_tag) == Some(&b'\n') {
            if let Some(out) = out.as_mut() {
                out.push_str(&html[copied_until..after_tag]);
            } else {
                let mut value = String::with_capacity(html.len() + 1);
                value.push_str(&html[..after_tag]);
                out = Some(value);
            }
            if let Some(out) = out.as_mut() {
                out.push('\n');
            }
            copied_until = after_tag;
        }
        index = after_tag;
    }

    if let Some(mut out) = out {
        out.push_str(&html[copied_until..]);
        Cow::Owned(out)
    } else {
        Cow::Borrowed(html)
    }
}

fn is_preserve_newline_start_tag(tag: &str) -> bool {
    let inner = tag
        .strip_prefix('<')
        .and_then(|value| value.strip_suffix('>'))
        .unwrap_or_default()
        .trim_start();
    if inner.starts_with('/') || inner.starts_with('!') || inner.starts_with('?') {
        return false;
    }
    let name_end = inner
        .find(|ch: char| ch.is_whitespace() || ch == '/' || ch == '>')
        .unwrap_or(inner.len());
    matches!(
        &inner[..name_end].to_ascii_lowercase()[..],
        "pre" | "textarea"
    )
}

fn normalize_text_value(text: &str, preserve_whitespace: bool) -> Cow<'_, str> {
    if preserve_whitespace {
        return Cow::Borrowed(text);
    }
    match normalized_whitespace(text) {
        Some(value) => Cow::Borrowed(value),
        None => Cow::Borrowed(text),
    }
}

fn normalized_whitespace(text: &str) -> Option<&'static str> {
    let mut saw_newline = false;
    for byte in text.as_bytes() {
        match *byte {
            b'\n' => saw_newline = true,
            b' ' | b'\t' | b'\r' | 0x0c => {}
            0x00..=0x7f => return None,
            _ => return normalized_unicode_whitespace(text),
        }
    }

    Some(if saw_newline { "\n" } else { " " })
}

fn normalized_unicode_whitespace(text: &str) -> Option<&'static str> {
    let mut saw_newline = false;
    for ch in text.chars() {
        if ch == '\n' {
            saw_newline = true;
        } else if !ch.is_whitespace() {
            return None;
        }
    }
    Some(if saw_newline { "\n" } else { " " })
}

struct FastParser<'a> {
    html: &'a str,
    bytes: &'a [u8],
    pos: usize,
    nodes: Vec<Node>,
    stack: Vec<NodeId>,
    preserve_stack: Vec<bool>,
    saw_html: bool,
    saw_body: bool,
    require_full_document: bool,
}

impl<'a> FastParser<'a> {
    fn new(html: &'a str, require_full_document: bool) -> Self {
        let estimated_nodes = (html.len() / 384).clamp(64, 16_384);
        let mut nodes = Vec::with_capacity(estimated_nodes);
        nodes.push(Node::new(NodeType::Document));
        Self {
            html,
            bytes: html.as_bytes(),
            pos: 0,
            nodes,
            stack: vec![NodeId::new(0)],
            preserve_stack: vec![false],
            saw_html: false,
            saw_body: false,
            require_full_document,
        }
    }

    fn parse(mut self) -> Option<Document> {
        while self.pos < self.bytes.len() {
            if let Some(raw_text_name) = self.current_raw_text_name() {
                self.parse_raw_text(raw_text_name)?;
            } else if self.bytes.get(self.pos) == Some(&b'<') {
                self.parse_markup()?;
            } else {
                let next = memchr::memchr(b'<', &self.bytes[self.pos..])
                    .map(|offset| self.pos + offset)
                    .unwrap_or(self.bytes.len());
                self.append_text(self.pos, next)?;
                self.pos = next;
            }
        }

        if self.stack.len() == 1
            && (!self.require_full_document || (self.saw_html && self.saw_body))
        {
            mark_template_content_strings(&mut self.nodes);
            shrink_sparse_nodes(&mut self.nodes);
            Some(Document {
                nodes: self.nodes,
                namespaces: None,
                root: NodeId::new(0),
                root_decomposed: false,
            })
        } else {
            None
        }
    }

    fn current_raw_text_name(&self) -> Option<&'static [u8]> {
        let current = *self.stack.last()?;
        match node_tag_name(&self.nodes, current)? {
            "script" => Some(b"script"),
            "style" => Some(b"style"),
            "textarea" => Some(b"textarea"),
            _ => None,
        }
    }

    fn parse_raw_text(&mut self, name: &[u8]) -> Option<()> {
        let close = find_raw_text_end(self.bytes, self.pos, name)?;
        self.append_text(self.pos, close)?;
        self.pos = close;
        self.parse_end_tag()
    }

    fn parse_markup(&mut self) -> Option<()> {
        if self.starts_with_at(self.pos, b"<!--") {
            return self.parse_comment();
        }
        if self.starts_with_at_case_insensitive(self.pos, b"<!doctype") {
            return self.parse_doctype();
        }
        if self.starts_with_at(self.pos, b"</") {
            return self.parse_end_tag();
        }
        if self.starts_with_at(self.pos, b"<?") {
            return self.parse_processing_instruction();
        }
        if self.starts_with_at(self.pos, b"<!") {
            return None;
        }
        self.parse_start_tag()
    }

    fn parse_comment(&mut self) -> Option<()> {
        let value_start = self.pos + 4;
        let close = find_bytes(&self.bytes[value_start..], b"-->")? + value_start;
        let text = &self.html[value_start..close];
        let id = self.push_node(Node::new(NodeType::Comment(CompactString::from(text))));
        let parent = *self.stack.last()?;
        append_new(&mut self.nodes, parent, id);
        self.pos = close + 3;
        Some(())
    }

    fn parse_processing_instruction(&mut self) -> Option<()> {
        let value_start = self.pos + 2;
        let close = memchr::memchr(b'>', &self.bytes[value_start..])? + value_start;
        let target = &self.html[value_start..close];
        let id = self.push_node(Node::new(NodeType::ProcessingInstruction(Box::new(
            ProcessingInstructionData {
                target: CompactString::from(target),
                data: CompactString::from(""),
            },
        ))));
        let parent = *self.stack.last()?;
        append_new(&mut self.nodes, parent, id);
        self.pos = close + 1;
        Some(())
    }

    fn parse_doctype(&mut self) -> Option<()> {
        if self.stack.len() != 1 {
            return None;
        }
        let close = self.find_tag_close(self.pos + 2)?;
        let inner = trim_ascii(&self.html[self.pos + 2..close]);
        if !starts_with_ascii_case_insensitive(inner.as_bytes(), b"doctype") {
            return None;
        }
        let mut parts = trim_ascii(&inner["doctype".len()..]).split_ascii_whitespace();
        let name = parts.next().unwrap_or("html");
        if parts.next().is_some() {
            return None;
        }
        let id = self.push_node(Node::new(NodeType::Doctype(Box::new(DoctypeData {
            name: CompactString::from(name.to_ascii_lowercase()),
            public_id: CompactString::from(""),
            system_id: CompactString::from(""),
        }))));
        append_new(&mut self.nodes, NodeId::new(0), id);
        self.pos = close + 1;
        Some(())
    }

    fn parse_start_tag(&mut self) -> Option<()> {
        let close = self.find_tag_close(self.pos + 1)?;
        let mut inner = trim_ascii(&self.html[self.pos + 1..close]);
        let self_closing = inner.ends_with('/');
        if self_closing {
            inner = trim_ascii_end(&inner[..inner.len() - 1]);
        }

        let name_end = fast_name_end(inner.as_bytes(), true);
        if name_end == 0 {
            return None;
        }
        let name = normalize_fast_tag_name(&inner[..name_end])?;
        let tag_name = name.as_ref();
        if is_fast_unsupported_tag(tag_name) {
            return None;
        }
        match tag_name {
            "html" => self.saw_html = true,
            "body" => self.saw_body = true,
            _ => {}
        }

        let attrs = parse_fast_attrs(&inner[name_end..])?;
        if tag_name == "p"
            && self
                .stack
                .last()
                .copied()
                .and_then(|id| node_tag_name(&self.nodes, id))
                == Some("p")
        {
            self.stack.pop();
            self.preserve_stack.pop();
        }
        let id = self.push_element(tag_name, attrs);
        let parent = *self.stack.last()?;
        append_new(&mut self.nodes, parent, id);
        if !self_closing && !is_void_element(tag_name) {
            let preserve_whitespace = self.preserve_stack.last().copied().unwrap_or(false)
                || is_preserve_whitespace_tag(tag_name);
            self.stack.push(id);
            self.preserve_stack.push(preserve_whitespace);
        }
        self.pos = close + 1;
        Some(())
    }

    fn parse_end_tag(&mut self) -> Option<()> {
        let close = self.find_tag_close(self.pos + 2)?;
        let raw_name = trim_ascii(&self.html[self.pos + 2..close]);
        let name_end = fast_name_end(raw_name.as_bytes(), false);
        if name_end == 0 || self.stack.len() <= 1 {
            return None;
        }
        let name = normalize_fast_tag_name(&raw_name[..name_end])?;
        let Some(position) = self
            .stack
            .iter()
            .rposition(|id| node_tag_name(&self.nodes, *id) == Some(name.as_ref()))
        else {
            self.pos = close + 1;
            return Some(());
        };
        if position == 0 {
            return None;
        }
        self.stack.truncate(position);
        self.preserve_stack.truncate(position);
        self.pos = close + 1;
        Some(())
    }

    fn append_text(&mut self, start: usize, end: usize) -> Option<()> {
        if start >= end {
            return Some(());
        }
        let parent = *self.stack.last()?;
        let preserve_whitespace = self.preserve_stack.last().copied().unwrap_or(false);
        let raw_text = &self.html[start..end];
        let decoded;
        let text =
            if !preserve_whitespace && memchr::memchr(b'&', &self.bytes[start..end]).is_some() {
                decoded = decode_fast_entities(raw_text)?;
                decoded.as_ref()
            } else {
                raw_text
            };
        if self.require_full_document
            && parent == NodeId::new(0)
            && self.nodes[parent.index()].first_child.is_none()
            && normalized_whitespace(text).is_some()
        {
            return Some(());
        }
        append_fast_text(&mut self.nodes, parent, text, preserve_whitespace);
        Some(())
    }

    fn push_node(&mut self, node: Node) -> NodeId {
        let id = NodeId::new(self.nodes.len());
        self.nodes.push(node);
        id
    }

    fn push_element(&mut self, tag_name: &str, attrs: Box<[Attr]>) -> NodeId {
        let id = NodeId::new(self.nodes.len());
        self.nodes.push(Node::new(NodeType::Element(ElementData {
            local_name: fast_local_name(tag_name),
            attrs,
        })));
        id
    }

    fn find_tag_close(&self, mut index: usize) -> Option<usize> {
        let close = memchr::memchr(b'>', &self.bytes[index..]).map(|offset| index + offset)?;
        if memchr::memchr2(b'"', b'\'', &self.bytes[index..close]).is_none() {
            return Some(close);
        }

        let mut quote = None;
        while index < self.bytes.len() {
            let byte = self.bytes[index];
            if let Some(active) = quote {
                if byte == active {
                    quote = None;
                }
            } else if byte == b'\'' || byte == b'"' {
                quote = Some(byte);
            } else if byte == b'>' {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn starts_with_at(&self, index: usize, needle: &[u8]) -> bool {
        self.bytes
            .get(index..index + needle.len())
            .is_some_and(|value| value == needle)
    }

    fn starts_with_at_case_insensitive(&self, index: usize, needle: &[u8]) -> bool {
        self.bytes
            .get(index..index + needle.len())
            .is_some_and(|value| value.eq_ignore_ascii_case(needle))
    }
}

fn try_parse_fast_full_document(html: &str) -> Option<Document> {
    let bytes = html.as_bytes();
    let trimmed = html.trim_start().as_bytes();
    if !(starts_with_ascii_case_insensitive(trimmed, b"<!doctype")
        || starts_with_ascii_case_insensitive(trimmed, b"<html"))
        || (memchr::memchr(b'&', bytes).is_some() && source_has_fast_unsupported_tag(bytes))
    {
        return None;
    }
    FastParser::new(html, true).parse()
}

fn try_parse_fast_fragment(html: &str) -> Option<Document> {
    let trimmed = html.trim_start().as_bytes();
    if starts_with_ascii_case_insensitive(trimmed, b"<!doctype")
        || starts_with_ascii_case_insensitive(trimmed, b"<html")
    {
        return None;
    }
    FastParser::new(html, false).parse()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    memchr::memmem::find(haystack, needle)
}

fn find_raw_text_end(bytes: &[u8], start: usize, name: &[u8]) -> Option<usize> {
    memchr_iter(b'<', &bytes[start..])
        .map(|offset| start + offset)
        .find(|open| {
            bytes.get(open + 1) == Some(&b'/')
                && bytes
                    .get(open + 2..open + 2 + name.len())
                    .is_some_and(|value| value.eq_ignore_ascii_case(name))
                && bytes
                    .get(open + 2 + name.len())
                    .is_some_and(|byte| byte.is_ascii_whitespace() || matches!(byte, b'/' | b'>'))
        })
}

fn node_tag_name(nodes: &[Node], id: NodeId) -> Option<&str> {
    match &nodes[id.index()].node_type {
        NodeType::Element(element) => Some(element.tag_name()),
        _ => None,
    }
}

fn shrink_sparse_nodes(nodes: &mut Vec<Node>) {
    if nodes.capacity() > nodes.len().saturating_add(nodes.len() / 8) {
        nodes.shrink_to_fit();
    }
}

fn mark_template_content_strings(nodes: &mut [Node]) {
    let template_ids = nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| match &node.node_type {
            NodeType::Element(element) if element.tag_name() == "template" => {
                Some(NodeId::new(index))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    for id in template_ids {
        mark_template_strings(nodes, id, false);
    }
}

fn append_fast_text(nodes: &mut Vec<Node>, parent: NodeId, text: &str, preserve_whitespace: bool) {
    let text = normalize_text_value(text, preserve_whitespace);
    if let Some(last_child) = nodes[parent.index()].last_child
        && let NodeType::Text(existing) = &mut nodes[last_child.index()].node_type
    {
        append_normalized_text(existing, text.as_ref(), preserve_whitespace);
        return;
    }
    let child = NodeId::new(nodes.len());
    nodes.push(Node {
        parent: Some(parent),
        first_child: None,
        last_child: None,
        prev_sibling: None,
        next_sibling: None,
        node_type: NodeType::Text(CompactString::from(text.as_ref())),
    });
    append_new_attached_leaf(nodes, parent, child);
}

fn parse_fast_attrs(input: &str) -> Option<Box<[Attr]>> {
    let mut first_attr = None;
    let mut many_attrs: Option<Vec<Attr>> = None;
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        if bytes[index] == b'/' {
            break;
        }
        let name_start = index;
        while index < bytes.len()
            && !bytes[index].is_ascii_whitespace()
            && bytes[index] != b'='
            && bytes[index] != b'/'
        {
            index += 1;
        }
        if index == name_start {
            return None;
        }
        let name = normalize_fast_attr_name(&input[name_start..index])?;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let value = if bytes.get(index) == Some(&b'=') {
            index += 1;
            while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            if index >= bytes.len() {
                return None;
            }
            if bytes[index] == b'\'' || bytes[index] == b'"' {
                let quote = bytes[index];
                index += 1;
                let value_start = index;
                while index < bytes.len() && bytes[index] != quote {
                    index += 1;
                }
                if index >= bytes.len() {
                    return None;
                }
                let value = &input[value_start..index];
                index += 1;
                value
            } else {
                let value_start = index;
                while index < bytes.len()
                    && !bytes[index].is_ascii_whitespace()
                    && bytes[index] != b'/'
                {
                    index += 1;
                }
                &input[value_start..index]
            }
        } else {
            ""
        };
        let value = if memchr::memchr(b'&', value.as_bytes()).is_some() {
            decode_fast_entities(value)?
        } else {
            Cow::Borrowed(value)
        };
        let attr = Attr::new(
            fast_local_name(name.as_ref()),
            Namespace::default(),
            value.as_ref(),
        );
        if let Some(attrs) = many_attrs.as_mut() {
            attrs.push(attr);
        } else if let Some(first) = first_attr.take() {
            let attrs = Vec::from([first, attr]);
            many_attrs = Some(attrs);
        } else {
            first_attr = Some(attr);
        }
    }

    if let Some(attrs) = many_attrs {
        Some(dedupe_attrs_last_wins(attrs))
    } else if let Some(attr) = first_attr {
        let attrs: Box<[Attr]> = Box::new([attr]);
        Some(attrs)
    } else {
        Some(Box::default())
    }
}

fn decode_fast_entities(value: &str) -> Option<Cow<'_, str>> {
    let bytes = value.as_bytes();
    let first = memchr::memchr(b'&', bytes)?;
    let mut decoded = String::with_capacity(value.len());
    decoded.push_str(&value[..first]);
    let mut index = first;

    while index < bytes.len() {
        if bytes[index] != b'&' {
            let next = memchr::memchr(b'&', &bytes[index..])
                .map(|offset| index + offset)
                .unwrap_or(bytes.len());
            decoded.push_str(&value[index..next]);
            index = next;
            continue;
        }

        let entity_start = index + 1;
        let Some(semicolon) =
            memchr::memchr(b';', &bytes[entity_start..]).map(|offset| entity_start + offset)
        else {
            decoded.push_str(&value[index..]);
            break;
        };
        let entity = &value[entity_start..semicolon];
        if !looks_like_entity_reference(entity.as_bytes()) {
            decoded.push('&');
            index = entity_start;
            continue;
        }
        push_decoded_entity(entity, &mut decoded)?;
        index = semicolon + 1;
    }

    Some(Cow::Owned(decoded))
}

fn looks_like_entity_reference(entity: &[u8]) -> bool {
    !entity.is_empty()
        && entity
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'#')
}

fn push_decoded_entity(entity: &str, out: &mut String) -> Option<()> {
    if let Some(number) = entity
        .strip_prefix("#x")
        .or_else(|| entity.strip_prefix("#X"))
    {
        let value = u32::from_str_radix(number, 16).ok()?;
        out.push(char::from_u32(value)?);
        return Some(());
    }
    if let Some(number) = entity.strip_prefix('#') {
        let value = number.parse::<u32>().ok()?;
        out.push(char::from_u32(value)?);
        return Some(());
    }

    let decoded = match entity {
        "amp" => "&",
        "apos" => "'",
        "bull" => "\u{2022}",
        "copy" => "\u{00a9}",
        "equiv" => "\u{2261}",
        "fjlig" => "fj",
        "gt" => ">",
        "hellip" => "\u{2026}",
        "le" => "\u{2264}",
        "ldquo" => "\u{201c}",
        "lsquo" => "\u{2018}",
        "lt" => "<",
        "mdash" => "\u{2014}",
        "middot" => "\u{00b7}",
        "nbsp" => "\u{00a0}",
        "ndash" => "\u{2013}",
        "nvgt" => ">\u{20d2}",
        "nvlt" => "<\u{20d2}",
        "para" => "\u{00b6}",
        "quot" => "\"",
        "raquo" => "\u{00bb}",
        "rdquo" => "\u{201d}",
        "rsquo" => "\u{2019}",
        "trade" => "\u{2122}",
        _ => return None,
    };
    out.push_str(decoded);
    Some(())
}

#[inline(always)]
fn normalize_fast_tag_name(value: &str) -> Option<Cow<'_, str>> {
    normalize_fast_name(value, |byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

#[inline(always)]
fn normalize_fast_attr_name(value: &str) -> Option<Cow<'_, str>> {
    normalize_fast_name(value, |byte| {
        byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'_' | b':' | b'.' | b',' | b'#' | b'@' | b'[' | b']' | b'(' | b')'
            )
    })
}

#[inline(always)]
fn normalize_fast_name(value: &str, is_allowed: impl Fn(u8) -> bool) -> Option<Cow<'_, str>> {
    if value.is_empty() {
        return None;
    }
    let mut has_uppercase = false;
    for byte in value.bytes() {
        if byte.is_ascii_uppercase() {
            has_uppercase = true;
        } else if !is_allowed(byte) {
            return None;
        }
    }
    Some(if has_uppercase {
        Cow::Owned(value.to_ascii_lowercase())
    } else {
        Cow::Borrowed(value)
    })
}

#[inline(always)]
fn trim_ascii(value: &str) -> &str {
    let bytes = value.as_bytes();
    let mut start = 0;
    let mut end = bytes.len();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &value[start..end]
}

#[inline(always)]
fn trim_ascii_end(value: &str) -> &str {
    let bytes = value.as_bytes();
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &value[..end]
}

#[inline(always)]
fn fast_name_end(bytes: &[u8], slash_terminates: bool) -> usize {
    bytes
        .iter()
        .position(|byte| byte.is_ascii_whitespace() || (slash_terminates && *byte == b'/'))
        .unwrap_or(bytes.len())
}

fn is_fast_unsupported_tag(name: &str) -> bool {
    matches!(name, "base" | "frameset" | "math" | "plaintext" | "xmp")
}

fn source_has_fast_unsupported_tag(bytes: &[u8]) -> bool {
    memchr_iter(b'<', bytes).any(|open| {
        let rest = &bytes[open + 1..];
        !matches!(rest.first(), Some(b'/' | b'!' | b'?'))
            && (starts_with_fast_tag_name(rest, b"base")
                || starts_with_fast_tag_name(rest, b"frameset")
                || starts_with_fast_tag_name(rest, b"math")
                || starts_with_fast_tag_name(rest, b"plaintext")
                || starts_with_fast_tag_name(rest, b"xmp"))
    })
}

fn starts_with_fast_tag_name(value: &[u8], name: &[u8]) -> bool {
    starts_with_ascii_case_insensitive(value, name)
        && value
            .get(name.len())
            .is_none_or(|byte| byte.is_ascii_whitespace() || matches!(byte, b'/' | b'>'))
}

#[inline(always)]
fn fast_local_name(name: &str) -> LocalName {
    match name {
        "a" => local_name!("a"),
        "alt" => local_name!("alt"),
        "area" => local_name!("area"),
        "article" => local_name!("article"),
        "aside" => local_name!("aside"),
        "b" => local_name!("b"),
        "br" => local_name!("br"),
        "button" => local_name!("button"),
        "canvas" => local_name!("canvas"),
        "circle" => local_name!("circle"),
        "cite" => local_name!("cite"),
        "body" => local_name!("body"),
        "class" => local_name!("class"),
        "code" => local_name!("code"),
        "content" => local_name!("content"),
        "crossorigin" => local_name!("crossorigin"),
        "cx" => local_name!("cx"),
        "cy" => local_name!("cy"),
        "d" => local_name!("d"),
        "defer" => local_name!("defer"),
        "defs" => local_name!("defs"),
        "dir" => local_name!("dir"),
        "div" => local_name!("div"),
        "figcaption" => local_name!("figcaption"),
        "figure" => local_name!("figure"),
        "fill" => local_name!("fill"),
        "footer" => local_name!("footer"),
        "form" => local_name!("form"),
        "g" => local_name!("g"),
        "h1" => local_name!("h1"),
        "h2" => local_name!("h2"),
        "h3" => local_name!("h3"),
        "h4" => local_name!("h4"),
        "head" => local_name!("head"),
        "header" => local_name!("header"),
        "height" => local_name!("height"),
        "href" => local_name!("href"),
        "hreflang" => local_name!("hreflang"),
        "html" => local_name!("html"),
        "id" => local_name!("id"),
        "i" => local_name!("i"),
        "iframe" => local_name!("iframe"),
        "img" => local_name!("img"),
        "input" => local_name!("input"),
        "itemprop" => local_name!("itemprop"),
        "label" => local_name!("label"),
        "lang" => local_name!("lang"),
        "li" => local_name!("li"),
        "link" => local_name!("link"),
        "loading" => local_name!("loading"),
        "main" => local_name!("main"),
        "media" => local_name!("media"),
        "meta" => local_name!("meta"),
        "name" => local_name!("name"),
        "nav" => local_name!("nav"),
        "noscript" => local_name!("noscript"),
        "option" => local_name!("option"),
        "p" => local_name!("p"),
        "path" => local_name!("path"),
        "picture" => local_name!("picture"),
        "pre" => local_name!("pre"),
        "property" => local_name!("property"),
        "r" => local_name!("r"),
        "rect" => local_name!("rect"),
        "rel" => local_name!("rel"),
        "role" => local_name!("role"),
        "section" => local_name!("section"),
        "script" => local_name!("script"),
        "small" => local_name!("small"),
        "source" => local_name!("source"),
        "span" => local_name!("span"),
        "src" => local_name!("src"),
        "srcset" => local_name!("srcset"),
        "stop" => local_name!("stop"),
        "strong" => local_name!("strong"),
        "style" => local_name!("style"),
        "sup" => local_name!("sup"),
        "svg" => local_name!("svg"),
        "symbol" => local_name!("symbol"),
        "tabindex" => local_name!("tabindex"),
        "target" => local_name!("target"),
        "td" => local_name!("td"),
        "template" => local_name!("template"),
        "textarea" => local_name!("textarea"),
        "th" => local_name!("th"),
        "time" => local_name!("time"),
        "title" => local_name!("title"),
        "tr" => local_name!("tr"),
        "tspan" => local_name!("tspan"),
        "type" => local_name!("type"),
        "ul" => local_name!("ul"),
        "use" => local_name!("use"),
        "value" => local_name!("value"),
        "video" => local_name!("video"),
        "viewbox" => local_name!("viewbox"),
        "width" => local_name!("width"),
        "xmlns" => local_name!("xmlns"),
        _ => LocalName::from(Cow::Borrowed(name)),
    }
}

struct Sink {
    nodes: UnsafeCell<Vec<Node>>,
    namespaces: UnsafeCell<Option<HashMap<NodeId, Namespace>>>,
    template_contents: UnsafeCell<Option<HashMap<NodeId, NodeId>>>,
    mathml_annotation_xml_integration_points: UnsafeCell<Option<HashSet<NodeId>>>,
    fallback_local_name: LocalName,
    document: NodeId,
    quirks_mode: Cell<QuirksMode>,
}

impl Sink {
    fn new(html: &str) -> Self {
        let estimated_nodes = (html.len() / 15).clamp(16, 262_144);
        let mut nodes = Vec::with_capacity(estimated_nodes);
        nodes.push(Node::new(NodeType::Document));
        Self {
            nodes: UnsafeCell::new(nodes),
            namespaces: UnsafeCell::new(None),
            template_contents: UnsafeCell::new(None),
            mathml_annotation_xml_integration_points: UnsafeCell::new(None),
            fallback_local_name: LocalName::from(Cow::Borrowed("")),
            document: NodeId::new(0),
            quirks_mode: Cell::new(QuirksMode::NoQuirks),
        }
    }

    #[inline(always)]
    fn nodes(&self) -> &Vec<Node> {
        // SAFETY: html5ever drives TreeSink methods synchronously on one parser thread.
        // Shared references returned by elem_name are used immediately by the tree
        // builder; mutations go through separate callbacks after those borrows end.
        unsafe { &*self.nodes.get() }
    }

    #[inline(always)]
    #[allow(clippy::mut_from_ref)]
    fn nodes_mut(&self) -> &mut Vec<Node> {
        // SAFETY: the parser never calls into the same Sink concurrently. All DOM
        // mutations are serialized by html5ever's tree builder, so this interior
        // mutability avoids RefCell checks without changing aliasing behavior.
        unsafe { &mut *self.nodes.get() }
    }

    #[inline(always)]
    fn namespaces(&self) -> Option<&HashMap<NodeId, Namespace>> {
        // SAFETY: namespace reads follow the same single-threaded TreeSink access
        // pattern as node reads. Mutations happen only in create_element callbacks.
        unsafe { (&*self.namespaces.get()).as_ref() }
    }

    #[inline(always)]
    #[allow(clippy::mut_from_ref)]
    fn namespaces_mut(&self) -> &mut Option<HashMap<NodeId, Namespace>> {
        // SAFETY: html5ever drives this sink synchronously, so namespace mutations
        // are serialized with node mutations.
        unsafe { &mut *self.namespaces.get() }
    }

    #[inline(always)]
    fn namespace(&self, id: NodeId) -> &Namespace {
        match self.namespaces().and_then(|namespaces| namespaces.get(&id)) {
            Some(namespace) => namespace,
            None => html_namespace(),
        }
    }

    fn set_namespace(&self, id: NodeId, namespace: Namespace) {
        if is_html_namespace(&namespace) {
            return;
        }
        self.namespaces_mut()
            .get_or_insert_with(HashMap::new)
            .insert(id, namespace);
    }

    #[inline(always)]
    fn template_contents(&self) -> Option<&HashMap<NodeId, NodeId>> {
        // SAFETY: see nodes()/namespaces(); parser callbacks are serialized.
        unsafe { (&*self.template_contents.get()).as_ref() }
    }

    #[inline(always)]
    #[allow(clippy::mut_from_ref)]
    fn template_contents_mut(&self) -> &mut Option<HashMap<NodeId, NodeId>> {
        // SAFETY: see nodes_mut(); parser callbacks are serialized.
        unsafe { &mut *self.template_contents.get() }
    }

    fn set_template_contents(&self, id: NodeId, contents: NodeId) {
        self.template_contents_mut()
            .get_or_insert_with(HashMap::new)
            .insert(id, contents);
    }

    #[inline(always)]
    fn mathml_annotation_xml_integration_points(&self) -> Option<&HashSet<NodeId>> {
        // SAFETY: see nodes()/namespaces(); parser callbacks are serialized.
        unsafe { (&*self.mathml_annotation_xml_integration_points.get()).as_ref() }
    }

    #[inline(always)]
    #[allow(clippy::mut_from_ref)]
    fn mathml_annotation_xml_integration_points_mut(&self) -> &mut Option<HashSet<NodeId>> {
        // SAFETY: see nodes_mut(); parser callbacks are serialized.
        unsafe { &mut *self.mathml_annotation_xml_integration_points.get() }
    }

    fn add_mathml_annotation_xml_integration_point(&self, id: NodeId) {
        self.mathml_annotation_xml_integration_points_mut()
            .get_or_insert_with(HashSet::new)
            .insert(id);
    }

    fn new_node(&self, node_type: NodeType) -> NodeId {
        let nodes = self.nodes_mut();
        let id = NodeId::new(nodes.len());
        nodes.push(Node::new(node_type));
        id
    }

    fn append_common(&self, parent: NodeId, child: NodeOrText<NodeId>) {
        match child {
            NodeOrText::AppendText(text) => {
                let nodes = self.nodes_mut();
                let preserve_whitespace = preserves_whitespace_context(nodes, parent);
                let text = normalize_text_value(text.as_ref(), preserve_whitespace);
                if let Some(last_child) = nodes[parent.index()].last_child
                    && let NodeType::Text(existing) = &mut nodes[last_child.index()].node_type
                {
                    append_normalized_text(existing, text.as_ref(), preserve_whitespace);
                    return;
                }
                let child_id = NodeId::new(nodes.len());
                nodes.push(Node {
                    node_type: NodeType::Text(CompactString::from(text.as_ref())),
                    parent: Some(parent),
                    first_child: None,
                    last_child: None,
                    prev_sibling: None,
                    next_sibling: None,
                });
                append_new_attached_leaf(nodes, parent, child_id);
            }
            NodeOrText::AppendNode(child_id) => {
                let nodes = self.nodes_mut();
                append_new(nodes, parent, child_id);
            }
        }
    }

    fn insert_before_common(&self, sibling: NodeId, child: NodeOrText<NodeId>) {
        match child {
            NodeOrText::AppendText(text) => {
                let nodes = self.nodes_mut();
                let preserve_whitespace = nodes[sibling.index()]
                    .parent
                    .is_some_and(|parent| preserves_whitespace_context(nodes, parent));
                let text = normalize_text_value(text.as_ref(), preserve_whitespace);
                if let Some(previous) = nodes[sibling.index()].prev_sibling
                    && let NodeType::Text(existing) = &mut nodes[previous.index()].node_type
                {
                    append_normalized_text(existing, text.as_ref(), preserve_whitespace);
                    return;
                }
                let child_id = NodeId::new(nodes.len());
                insert_text_before_new(
                    nodes,
                    sibling,
                    child_id,
                    CompactString::from(text.as_ref()),
                );
            }
            NodeOrText::AppendNode(child_id) => {
                let nodes = self.nodes_mut();
                insert_before(nodes, sibling, child_id);
            }
        }
    }
}

fn append_normalized_text(existing: &mut CompactString, text: &str, preserve_whitespace: bool) {
    if preserve_whitespace {
        existing.push_str(text);
        return;
    }
    let Some(existing_whitespace) = known_or_normalized_whitespace(existing.as_str()) else {
        existing.push_str(text);
        return;
    };
    let Some(text_whitespace) = known_or_normalized_whitespace(text) else {
        existing.push_str(text);
        return;
    };
    if existing_whitespace == "\n" || text_whitespace == "\n" {
        if existing_whitespace != "\n" {
            *existing = CompactString::const_new("\n");
        }
    } else if existing_whitespace != " " {
        *existing = CompactString::const_new(" ");
    }
}

#[inline(always)]
fn known_or_normalized_whitespace(text: &str) -> Option<&'static str> {
    match text.as_bytes() {
        b" " => Some(" "),
        b"\n" => Some("\n"),
        _ => normalized_whitespace(text),
    }
}

impl TreeSink for Sink {
    type Handle = NodeId;
    type Output = Document;
    type ElemName<'a>
        = markup5ever::ExpandedName<'a>
    where
        Self: 'a;

    fn finish(self) -> Document {
        let mut nodes = self.nodes.into_inner();
        if let Some(template_contents) = self.template_contents.into_inner() {
            splice_template_contents(&mut nodes, &template_contents);
        }
        shrink_sparse_nodes(&mut nodes);
        Document {
            nodes,
            namespaces: self.namespaces.into_inner(),
            root: self.document,
            root_decomposed: false,
        }
    }

    fn parse_error(&self, _msg: Cow<'static, str>) {}

    fn get_document(&self) -> NodeId {
        self.document
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.quirks_mode.set(mode);
    }

    fn same_node(&self, x: &NodeId, y: &NodeId) -> bool {
        x == y
    }

    fn elem_name<'a>(&'a self, target: &'a NodeId) -> Self::ElemName<'a> {
        match &self.nodes()[target.index()].node_type {
            NodeType::Element(element) => markup5ever::ExpandedName {
                ns: self.namespace(*target),
                local: &element.local_name,
            },
            _ => markup5ever::ExpandedName {
                ns: html_namespace(),
                local: &self.fallback_local_name,
            },
        }
    }

    fn get_template_contents(&self, target: &NodeId) -> NodeId {
        match &self.nodes()[target.index()].node_type {
            NodeType::Element(_) => self
                .template_contents()
                .and_then(|templates| templates.get(target))
                .copied()
                .unwrap_or(*target),
            _ => *target,
        }
    }

    fn is_mathml_annotation_xml_integration_point(&self, target: &NodeId) -> bool {
        match &self.nodes()[target.index()].node_type {
            NodeType::Element(_) => self
                .mathml_annotation_xml_integration_points()
                .is_some_and(|nodes| nodes.contains(target)),
            _ => false,
        }
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> NodeId {
        let QualName {
            ns,
            local,
            prefix: _,
        } = name;
        let attrs = if attrs.is_empty() {
            Box::default()
        } else {
            let mut out = Vec::with_capacity(attrs.len());
            for attr in attrs {
                let Attribute { name, value } = attr;
                out.push(Attr::new(name.local, name.ns, value.as_ref()));
            }
            dedupe_attrs_last_wins(out)
        };

        let nodes = self.nodes_mut();
        let template_contents = if flags.template {
            let id = NodeId::new(nodes.len());
            nodes.push(Node::new(NodeType::Document));
            Some(id)
        } else {
            None
        };
        let id = NodeId::new(nodes.len());
        nodes.push(Node::new(NodeType::Element(ElementData {
            local_name: local,
            attrs,
        })));
        self.set_namespace(id, ns);
        if let Some(template_contents) = template_contents {
            self.set_template_contents(id, template_contents);
        }
        if flags.mathml_annotation_xml_integration_point {
            self.add_mathml_annotation_xml_integration_point(id);
        }
        id
    }

    fn create_comment(&self, text: StrTendril) -> NodeId {
        self.new_node(NodeType::Comment(CompactString::from(text.as_ref())))
    }

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> NodeId {
        self.new_node(NodeType::ProcessingInstruction(Box::new(
            ProcessingInstructionData {
                target: CompactString::from(target.as_ref()),
                data: CompactString::from(data.as_ref()),
            },
        )))
    }

    fn append(&self, parent: &NodeId, child: NodeOrText<NodeId>) {
        self.append_common(*parent, child);
    }

    fn append_before_sibling(&self, sibling: &NodeId, new_node: NodeOrText<NodeId>) {
        self.insert_before_common(*sibling, new_node);
    }

    fn append_based_on_parent_node(
        &self,
        element: &NodeId,
        prev_element: &NodeId,
        child: NodeOrText<NodeId>,
    ) {
        if self.nodes()[element.index()].parent.is_some() {
            self.insert_before_common(*element, child);
        } else {
            self.append_common(*prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        let nodes = self.nodes_mut();
        let doctype = NodeId::new(nodes.len());
        nodes.push(Node::new(NodeType::Doctype(Box::new(DoctypeData {
            name: CompactString::from(name.as_ref()),
            public_id: CompactString::from(public_id.as_ref()),
            system_id: CompactString::from(system_id.as_ref()),
        }))));
        append_new(nodes, self.document, doctype);
    }

    fn add_attrs_if_missing(&self, target: &NodeId, attrs: Vec<Attribute>) {
        let nodes = self.nodes_mut();
        let NodeType::Element(element) = &mut nodes[target.index()].node_type else {
            return;
        };

        let additional = attrs
            .into_iter()
            .filter_map(|attr| {
                let Attribute { name, value } = attr;
                let exists = element
                    .attrs
                    .iter()
                    .any(|existing| existing.name == name.local);
                (!exists).then(|| Attr::new(name.local, name.ns, value.as_ref()))
            })
            .collect::<Vec<_>>();
        if !additional.is_empty() {
            let mut combined = Vec::with_capacity(element.attrs.len() + additional.len());
            combined.extend_from_slice(&element.attrs);
            combined.extend(additional);
            element.attrs = combined.into_boxed_slice();
        }
    }

    fn remove_from_parent(&self, target: &NodeId) {
        let nodes = self.nodes_mut();
        detach(nodes, *target);
    }

    fn reparent_children(&self, node: &NodeId, new_parent: &NodeId) {
        let nodes = self.nodes_mut();
        let mut child = nodes[node.index()].first_child;
        while let Some(current) = child {
            child = nodes[current.index()].next_sibling;
            append_existing(nodes, *new_parent, current);
        }
    }
}

fn splice_template_contents(nodes: &mut [Node], template_contents: &HashMap<NodeId, NodeId>) {
    for (&template, &contents) in template_contents {
        let mut child = nodes[contents.index()].first_child;
        while let Some(current) = child {
            child = nodes[current.index()].next_sibling;
            append_existing(nodes, template, current);
            mark_template_strings(nodes, current, false);
        }
        nodes[contents.index()].first_child = None;
        nodes[contents.index()].last_child = None;
    }
}

fn mark_template_strings(nodes: &mut [Node], id: NodeId, inside_raw_text: bool) {
    let in_raw_text = inside_raw_text
        || matches!(
            &nodes[id.index()].node_type,
            NodeType::Element(element) if is_raw_text_element(element.tag_name())
        );

    if !in_raw_text && let NodeType::Text(text) = &nodes[id.index()].node_type {
        nodes[id.index()].node_type = NodeType::TemplateString(text.clone());
    }

    let mut child = nodes[id.index()].first_child;
    while let Some(current) = child {
        child = nodes[current.index()].next_sibling;
        mark_template_strings(nodes, current, in_raw_text);
    }
}

#[inline(always)]
fn detach(nodes: &mut [Node], target: NodeId) {
    let parent = nodes[target.index()].parent;
    let previous = nodes[target.index()].prev_sibling;
    let next = nodes[target.index()].next_sibling;

    if let Some(previous_id) = previous {
        nodes[previous_id.index()].next_sibling = next;
    } else if let Some(parent_id) = parent {
        nodes[parent_id.index()].first_child = next;
    }

    if let Some(next_id) = next {
        nodes[next_id.index()].prev_sibling = previous;
    } else if let Some(parent_id) = parent {
        nodes[parent_id.index()].last_child = previous;
    }

    nodes[target.index()].parent = None;
    nodes[target.index()].prev_sibling = None;
    nodes[target.index()].next_sibling = None;
}

#[inline(always)]
fn append_existing(nodes: &mut [Node], parent: NodeId, child: NodeId) {
    detach(nodes, child);
    append_new(nodes, parent, child);
}

#[inline(always)]
fn append_new(nodes: &mut [Node], parent: NodeId, child: NodeId) {
    debug_assert!(nodes[child.index()].parent.is_none());
    debug_assert!(nodes[child.index()].prev_sibling.is_none());
    debug_assert!(nodes[child.index()].next_sibling.is_none());

    nodes[child.index()].parent = Some(parent);

    if let Some(last_child) = nodes[parent.index()].last_child {
        nodes[last_child.index()].next_sibling = Some(child);
        nodes[child.index()].prev_sibling = Some(last_child);
    } else {
        nodes[parent.index()].first_child = Some(child);
    }

    nodes[parent.index()].last_child = Some(child);
}

#[inline(always)]
fn append_new_attached_leaf(nodes: &mut [Node], parent: NodeId, child: NodeId) {
    debug_assert_eq!(nodes[child.index()].parent, Some(parent));
    debug_assert!(nodes[child.index()].first_child.is_none());
    debug_assert!(nodes[child.index()].last_child.is_none());
    debug_assert!(nodes[child.index()].prev_sibling.is_none());
    debug_assert!(nodes[child.index()].next_sibling.is_none());

    if let Some(last_child) = nodes[parent.index()].last_child {
        nodes[last_child.index()].next_sibling = Some(child);
        nodes[child.index()].prev_sibling = Some(last_child);
    } else {
        nodes[parent.index()].first_child = Some(child);
    }

    nodes[parent.index()].last_child = Some(child);
}

#[inline(always)]
fn insert_before(nodes: &mut [Node], sibling: NodeId, child: NodeId) {
    detach(nodes, child);
    insert_before_new(nodes, sibling, child);
}

#[inline(always)]
fn insert_before_new(nodes: &mut [Node], sibling: NodeId, child: NodeId) {
    debug_assert!(nodes[child.index()].parent.is_none());
    debug_assert!(nodes[child.index()].prev_sibling.is_none());
    debug_assert!(nodes[child.index()].next_sibling.is_none());

    let parent = nodes[sibling.index()].parent;
    let previous = nodes[sibling.index()].prev_sibling;

    nodes[child.index()].parent = parent;
    nodes[child.index()].prev_sibling = previous;
    nodes[child.index()].next_sibling = Some(sibling);

    if let Some(previous_id) = previous {
        nodes[previous_id.index()].next_sibling = Some(child);
    } else if let Some(parent_id) = parent {
        nodes[parent_id.index()].first_child = Some(child);
    }

    nodes[sibling.index()].prev_sibling = Some(child);
}

#[inline(always)]
fn insert_text_before_new(
    nodes: &mut Vec<Node>,
    sibling: NodeId,
    child: NodeId,
    text: CompactString,
) {
    let parent = nodes[sibling.index()].parent;
    let previous = nodes[sibling.index()].prev_sibling;

    nodes.push(Node {
        node_type: NodeType::Text(text),
        parent,
        first_child: None,
        last_child: None,
        prev_sibling: previous,
        next_sibling: Some(sibling),
    });

    if let Some(previous_id) = previous {
        nodes[previous_id.index()].next_sibling = Some(child);
    } else if let Some(parent_id) = parent {
        nodes[parent_id.index()].first_child = Some(child);
    }

    nodes[sibling.index()].prev_sibling = Some(child);
}

#[inline(always)]
fn preserves_whitespace_context(nodes: &[Node], id: NodeId) -> bool {
    let mut current = Some(id);
    while let Some(id) = current {
        if matches!(
            &nodes[id.index()].node_type,
            NodeType::Element(element) if is_preserve_whitespace_tag(element.tag_name())
        ) {
            return true;
        }
        current = nodes[id.index()].parent;
    }
    false
}

#[inline(always)]
fn is_preserve_whitespace_tag(name: &str) -> bool {
    matches!(name, "pre" | "textarea" | "script" | "style")
}
