use crate::attrs::{Attr, attrs_to_map};
use compact_str::CompactString;
use markup5ever::{LocalName, Namespace, ns};
use smallvec::SmallVec;
use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::Infallible;
use std::num::NonZeroU32;
use std::sync::LazyLock;

pub const HTML_NAMESPACE_URL: &str = "http://www.w3.org/1999/xhtml";

static HTML_NAMESPACE: LazyLock<Namespace> = LazyLock::new(|| ns!(html));

#[inline(always)]
pub fn html_namespace() -> &'static Namespace {
    &HTML_NAMESPACE
}

#[inline(always)]
pub fn is_html_namespace(namespace: &Namespace) -> bool {
    namespace.as_ref() == HTML_NAMESPACE_URL
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeId(NonZeroU32);

impl NodeId {
    #[inline(always)]
    pub fn new(index: usize) -> Self {
        assert!(
            index < u32::MAX as usize,
            "rustysoup DOM exceeded u32 node IDs"
        );
        Self(NonZeroU32::new(index as u32 + 1).unwrap_or(NonZeroU32::MIN))
    }

    #[inline(always)]
    pub fn index(self) -> usize {
        self.0.get() as usize - 1
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    pub parent: Option<NodeId>,
    pub first_child: Option<NodeId>,
    pub last_child: Option<NodeId>,
    pub prev_sibling: Option<NodeId>,
    pub next_sibling: Option<NodeId>,
    pub node_type: NodeType,
}

impl Node {
    pub fn new(node_type: NodeType) -> Self {
        Self {
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            node_type,
        }
    }
}

#[derive(Clone, Debug)]
pub enum NodeType {
    Document,
    Element(ElementData),
    Text(CompactString),
    CData(CompactString),
    Declaration(CompactString),
    TemplateString(CompactString),
    Comment(CompactString),
    Doctype(Box<DoctypeData>),
    ProcessingInstruction(Box<ProcessingInstructionData>),
}

#[derive(Clone, Debug)]
pub struct DoctypeData {
    pub name: CompactString,
    pub public_id: CompactString,
    pub system_id: CompactString,
}

#[derive(Clone, Debug)]
pub struct ProcessingInstructionData {
    pub target: CompactString,
    pub data: CompactString,
}

#[derive(Clone, Debug)]
pub struct ElementData {
    pub local_name: LocalName,
    pub attrs: Box<[Attr]>,
}

impl ElementData {
    #[inline(always)]
    pub fn tag_name(&self) -> &str {
        self.local_name.as_ref()
    }
}

#[derive(Clone, Debug)]
pub struct Document {
    pub nodes: Vec<Node>,
    pub namespaces: Option<HashMap<NodeId, Namespace>>,
    pub root: NodeId,
    pub root_decomposed: bool,
}

#[derive(Clone, Copy)]
enum SerializeFrame {
    Node(NodeId),
    CloseElement(NodeId),
    DocumentDoctypeNewline,
}

#[derive(Clone, Copy)]
enum PrettifyFrame {
    Node(NodeId, usize),
    CloseElement(NodeId, usize),
}

impl Document {
    pub fn empty() -> Self {
        let root = NodeId::new(0);
        Self {
            nodes: vec![Node::new(NodeType::Document)],
            namespaces: None,
            root,
            root_decomposed: false,
        }
    }

    pub fn detached_text(text: String) -> (Self, NodeId) {
        let mut document = Self::empty();
        let id = document.push_node(Node::new(NodeType::Text(CompactString::from(text))));
        (document, id)
    }

    pub fn detached_comment(text: String) -> (Self, NodeId) {
        let mut document = Self::empty();
        let id = document.push_node(Node::new(NodeType::Comment(CompactString::from(text))));
        (document, id)
    }

    pub fn detached_cdata(text: String) -> (Self, NodeId) {
        let mut document = Self::empty();
        let id = document.push_node(Node::new(NodeType::CData(CompactString::from(text))));
        (document, id)
    }

    pub fn detached_declaration(text: String) -> (Self, NodeId) {
        let mut document = Self::empty();
        let id = document.push_node(Node::new(NodeType::Declaration(CompactString::from(text))));
        (document, id)
    }

    pub fn detached_template_string(text: String) -> (Self, NodeId) {
        let mut document = Self::empty();
        let id = document.push_node(Node::new(NodeType::TemplateString(CompactString::from(
            text,
        ))));
        (document, id)
    }

    pub fn detached_doctype(text: String) -> (Self, NodeId) {
        let mut document = Self::empty();
        let id = document.push_node(Node::new(NodeType::Doctype(Box::new(DoctypeData {
            name: CompactString::from(text),
            public_id: CompactString::new(""),
            system_id: CompactString::new(""),
        }))));
        (document, id)
    }

    pub fn detached_processing_instruction(text: String) -> (Self, NodeId) {
        let mut document = Self::empty();
        let id = document.push_node(Node::new(NodeType::ProcessingInstruction(Box::new(
            ProcessingInstructionData {
                target: CompactString::from(text),
                data: CompactString::new(""),
            },
        ))));
        (document, id)
    }

    #[inline(always)]
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }

    pub fn append_text(&mut self, parent: NodeId, text: String) -> NodeId {
        let id = self.push_node(Node::new(NodeType::Text(CompactString::from(text))));
        self.append_existing(parent, id);
        id
    }

    pub fn insert_text(&mut self, parent: NodeId, index: usize, text: String) -> NodeId {
        let id = self.push_node(Node::new(NodeType::Text(CompactString::from(text))));
        self.insert_existing(parent, index, id);
        id
    }

    pub fn insert_text_before(&mut self, sibling: NodeId, text: String) -> NodeId {
        let id = self.push_node(Node::new(NodeType::Text(CompactString::from(text))));
        self.insert_before_existing(sibling, id);
        id
    }

    pub fn insert_text_after(&mut self, sibling: NodeId, text: String) -> NodeId {
        let id = self.push_node(Node::new(NodeType::Text(CompactString::from(text))));
        self.insert_after_existing(sibling, id);
        id
    }

    pub fn append_clone_from(
        &mut self,
        parent: NodeId,
        source: &Document,
        source_id: NodeId,
    ) -> NodeId {
        let id = self.clone_detached_subtree(source, source_id);
        self.append_existing(parent, id);
        id
    }

    pub fn clone_detached_from(&mut self, source: &Document, source_id: NodeId) -> NodeId {
        self.clone_detached_subtree(source, source_id)
    }

    pub fn insert_clone_from(
        &mut self,
        parent: NodeId,
        index: usize,
        source: &Document,
        source_id: NodeId,
    ) -> NodeId {
        let id = self.clone_detached_subtree(source, source_id);
        self.insert_existing(parent, index, id);
        id
    }

    pub fn insert_clone_before_from(
        &mut self,
        sibling: NodeId,
        source: &Document,
        source_id: NodeId,
    ) -> NodeId {
        let id = self.clone_detached_subtree(source, source_id);
        self.insert_before_existing(sibling, id);
        id
    }

    pub fn insert_clone_after_from(
        &mut self,
        sibling: NodeId,
        source: &Document,
        source_id: NodeId,
    ) -> NodeId {
        let id = self.clone_detached_subtree(source, source_id);
        self.insert_after_existing(sibling, id);
        id
    }

    pub fn wrap_with_clone_from(
        &mut self,
        target: NodeId,
        source: &Document,
        wrapper_id: NodeId,
    ) -> NodeId {
        let wrapper = self.clone_detached_subtree(source, wrapper_id);
        self.insert_before_existing(target, wrapper);
        self.append_existing(wrapper, target);
        wrapper
    }

    fn clone_detached_subtree(&mut self, source: &Document, source_id: NodeId) -> NodeId {
        let id = self.clone_detached_node(source, source_id);
        let mut stack = Vec::with_capacity(16);
        stack.push((source_id, id));

        while let Some((source_parent, target_parent)) = stack.pop() {
            let mut child = source.node(source_parent).first_child;
            while let Some(current) = child {
                let child_id = self.clone_detached_node(source, current);
                self.append_existing(target_parent, child_id);
                stack.push((current, child_id));
                child = source.node(current).next_sibling;
            }
        }

        id
    }

    fn clone_detached_node(&mut self, source: &Document, source_id: NodeId) -> NodeId {
        let id = self.push_node(Node::new(source.node(source_id).node_type.clone()));
        if source.element(source_id).is_some()
            && let Some(namespace) = source.namespace_override(source_id)
        {
            self.set_element_namespace(id, namespace.clone());
        }
        id
    }

    fn push_node(&mut self, node: Node) -> NodeId {
        let id = NodeId::new(self.nodes.len());
        self.nodes.push(node);
        id
    }

    pub fn detach(&mut self, target: NodeId) {
        let parent = self.nodes[target.index()].parent;
        let previous = self.nodes[target.index()].prev_sibling;
        let next = self.nodes[target.index()].next_sibling;

        if let Some(previous_id) = previous {
            self.nodes[previous_id.index()].next_sibling = next;
        } else if let Some(parent_id) = parent {
            self.nodes[parent_id.index()].first_child = next;
        }

        if let Some(next_id) = next {
            self.nodes[next_id.index()].prev_sibling = previous;
        } else if let Some(parent_id) = parent {
            self.nodes[parent_id.index()].last_child = previous;
        }

        self.nodes[target.index()].parent = None;
        self.nodes[target.index()].prev_sibling = None;
        self.nodes[target.index()].next_sibling = None;
    }

    pub fn clear_children(&mut self, parent: NodeId) {
        let children = self.child_nodes(parent);
        for child in children {
            self.detach(child);
        }
    }

    pub fn decompose_root(&mut self) {
        self.clear_children(self.root);
        self.root_decomposed = true;
    }

    pub fn decompose_node(&mut self, target: NodeId) {
        self.clear_children(target);
        self.detach(target);
        if let Some(element) = self.element_mut(target) {
            element.local_name = LocalName::from(Cow::Borrowed(""));
            element.attrs = Vec::new().into_boxed_slice();
        }
        if let Some(namespaces) = self.namespaces.as_mut() {
            namespaces.remove(&target);
            if namespaces.is_empty() {
                self.namespaces = None;
            }
        }
    }

    pub fn remove_first_empty_head(&mut self) {
        let Some(head) = self
            .descendant_elements(self.root, false)
            .into_iter()
            .find(|id| {
                self.element(*id)
                    .is_some_and(|element| element.tag_name() == "head")
                    && self.node(*id).first_child.is_none()
            })
        else {
            return;
        };
        self.detach(head);
    }

    pub fn promote_bogus_comments_to_processing_instructions(&mut self, values: &[String]) {
        if values.is_empty() {
            return;
        }

        let mut value_index = 0;
        for id in self.descendant_nodes(self.root, false) {
            if value_index >= values.len() {
                break;
            }
            let expected_comment = format!("?{}", values[value_index]);
            let NodeType::Comment(comment) = &self.node(id).node_type else {
                continue;
            };
            if comment.as_str() != expected_comment {
                continue;
            }
            self.nodes[id.index()].node_type =
                NodeType::ProcessingInstruction(Box::new(ProcessingInstructionData {
                    target: CompactString::from(values[value_index].as_str()),
                    data: CompactString::from(""),
                }));
            value_index += 1;
        }
    }

    pub fn unwrap_node(&mut self, target: NodeId) {
        let children = self.child_nodes(target);
        for child in children {
            self.insert_before_existing(target, child);
        }
        self.detach(target);
    }

    pub fn append_existing(&mut self, parent: NodeId, child: NodeId) {
        self.detach(child);
        self.nodes[child.index()].parent = Some(parent);

        if let Some(last_child) = self.nodes[parent.index()].last_child {
            self.nodes[last_child.index()].next_sibling = Some(child);
            self.nodes[child.index()].prev_sibling = Some(last_child);
        } else {
            self.nodes[parent.index()].first_child = Some(child);
        }

        self.nodes[parent.index()].last_child = Some(child);
    }

    pub fn insert_existing(&mut self, parent: NodeId, index: usize, child: NodeId) {
        let children = self.child_nodes(parent);
        if let Some(sibling) = children.get(index).copied() {
            self.insert_before_existing(sibling, child);
        } else {
            self.append_existing(parent, child);
        }
    }

    pub fn child_index(&self, parent: NodeId, target: NodeId) -> Option<usize> {
        let mut index = 0;
        let mut child = self.node(parent).first_child;
        while let Some(current) = child {
            if current == target {
                return Some(index);
            }
            index += 1;
            child = self.node(current).next_sibling;
        }
        None
    }

    pub fn insert_before_existing(&mut self, sibling: NodeId, child: NodeId) {
        self.detach(child);
        let parent = self.nodes[sibling.index()].parent;
        let previous = self.nodes[sibling.index()].prev_sibling;

        self.nodes[child.index()].parent = parent;
        self.nodes[child.index()].prev_sibling = previous;
        self.nodes[child.index()].next_sibling = Some(sibling);

        if let Some(previous_id) = previous {
            self.nodes[previous_id.index()].next_sibling = Some(child);
        } else if let Some(parent_id) = parent {
            self.nodes[parent_id.index()].first_child = Some(child);
        }

        self.nodes[sibling.index()].prev_sibling = Some(child);
    }

    pub fn insert_after_existing(&mut self, sibling: NodeId, child: NodeId) {
        self.detach(child);
        let parent = self.nodes[sibling.index()].parent;
        let next = self.nodes[sibling.index()].next_sibling;

        self.nodes[child.index()].parent = parent;
        self.nodes[child.index()].prev_sibling = Some(sibling);
        self.nodes[child.index()].next_sibling = next;

        if let Some(next_id) = next {
            self.nodes[next_id.index()].prev_sibling = Some(child);
        } else if let Some(parent_id) = parent {
            self.nodes[parent_id.index()].last_child = Some(child);
        }

        self.nodes[sibling.index()].next_sibling = Some(child);
    }

    pub fn unwrap_single_child_element_named(&mut self, tag_name: &str) {
        let Some(wrapper) = self.nodes[self.root.index()].first_child else {
            return;
        };
        if self.nodes[self.root.index()].last_child != Some(wrapper) {
            return;
        }
        if !matches!(
            &self.nodes[wrapper.index()].node_type,
            NodeType::Element(element) if element.tag_name() == tag_name
        ) {
            return;
        }

        let first = self.nodes[wrapper.index()].first_child;
        let last = self.nodes[wrapper.index()].last_child;
        self.nodes[self.root.index()].first_child = first;
        self.nodes[self.root.index()].last_child = last;

        let mut child = first;
        while let Some(current) = child {
            self.nodes[current.index()].parent = Some(self.root);
            child = self.nodes[current.index()].next_sibling;
        }

        self.nodes[wrapper.index()].parent = None;
        self.nodes[wrapper.index()].first_child = None;
        self.nodes[wrapper.index()].last_child = None;
    }

    #[inline]
    pub fn element(&self, id: NodeId) -> Option<&ElementData> {
        match &self.node(id).node_type {
            NodeType::Element(element) => Some(element),
            _ => None,
        }
    }

    #[inline]
    pub fn element_mut(&mut self, id: NodeId) -> Option<&mut ElementData> {
        match &mut self.nodes[id.index()].node_type {
            NodeType::Element(element) => Some(element),
            _ => None,
        }
    }

    #[inline(always)]
    pub fn element_namespace(&self, id: NodeId) -> &Namespace {
        match self.namespace_override(id) {
            Some(namespace) => namespace,
            None => html_namespace(),
        }
    }

    #[inline]
    fn namespace_override(&self, id: NodeId) -> Option<&Namespace> {
        self.namespaces
            .as_ref()
            .and_then(|namespaces| namespaces.get(&id))
    }

    fn set_element_namespace(&mut self, id: NodeId, namespace: Namespace) {
        if is_html_namespace(&namespace) {
            return;
        }
        self.namespaces
            .get_or_insert_with(HashMap::new)
            .insert(id, namespace);
    }

    pub fn set_tag_name(&mut self, id: NodeId, name: String) {
        if let Some(element) = self.element_mut(id) {
            element.local_name = LocalName::from(Cow::Owned(name));
        }
    }

    #[inline]
    pub fn is_element(&self, id: NodeId) -> bool {
        matches!(self.node(id).node_type, NodeType::Element(_))
    }

    pub fn is_text_like(&self, id: NodeId) -> bool {
        matches!(
            self.node(id).node_type,
            NodeType::Text(_)
                | NodeType::CData(_)
                | NodeType::Declaration(_)
                | NodeType::TemplateString(_)
                | NodeType::Comment(_)
                | NodeType::Doctype(_)
                | NodeType::ProcessingInstruction(_)
        )
    }

    pub fn node_string(&self, id: NodeId) -> Option<&str> {
        match &self.node(id).node_type {
            NodeType::Text(text)
            | NodeType::CData(text)
            | NodeType::Declaration(text)
            | NodeType::TemplateString(text)
            | NodeType::Comment(text) => Some(text.as_str()),
            NodeType::Doctype(data) => Some(data.name.as_str()),
            NodeType::ProcessingInstruction(data) if data.data.is_empty() => {
                Some(data.target.as_str())
            }
            _ => None,
        }
    }

    pub fn parent_node(&self, id: NodeId) -> Option<NodeId> {
        self.node(id).parent
    }

    pub fn parent_element(&self, id: NodeId) -> Option<NodeId> {
        let parent = self.node(id).parent?;
        self.is_element(parent).then_some(parent)
    }

    pub fn first_element_child(&self, id: NodeId) -> Option<NodeId> {
        let mut child = self.node(id).first_child;
        while let Some(current) = child {
            if self.is_element(current) {
                return Some(current);
            }
            child = self.node(current).next_sibling;
        }
        None
    }

    pub fn prev_sibling_element(&self, id: NodeId) -> Option<NodeId> {
        let mut sibling = self.node(id).prev_sibling;
        while let Some(current) = sibling {
            if self.is_element(current) {
                return Some(current);
            }
            sibling = self.node(current).prev_sibling;
        }
        None
    }

    pub fn next_sibling_element(&self, id: NodeId) -> Option<NodeId> {
        let mut sibling = self.node(id).next_sibling;
        while let Some(current) = sibling {
            if self.is_element(current) {
                return Some(current);
            }
            sibling = self.node(current).next_sibling;
        }
        None
    }

    #[inline]
    pub fn attr(&self, id: NodeId, name: &str) -> Option<&str> {
        self.element(id)?
            .attrs
            .iter()
            .rev()
            .find(|attr| attr.name() == name)
            .and_then(|attr| attr.value.as_deref())
    }

    #[inline]
    pub fn attr_present(&self, id: NodeId, name: &str) -> bool {
        self.element(id)
            .is_some_and(|element| element.attrs.iter().any(|attr| attr.name() == name))
    }

    #[inline]
    pub fn attr_value(&self, id: NodeId, name: &str) -> Option<Option<&str>> {
        self.element(id)?
            .attrs
            .iter()
            .rev()
            .find(|attr| attr.name() == name)
            .map(|attr| attr.value.as_deref())
    }

    pub fn set_attr_value(&mut self, id: NodeId, name: String, value: Option<String>) {
        let Some(element) = self.element_mut(id) else {
            return;
        };
        if let Some(attr) = element.attrs.iter_mut().find(|attr| attr.name() == name) {
            attr.value = value.map(CompactString::from);
        } else {
            let mut attrs = element.attrs.to_vec();
            attrs.push(Attr::with_optional_dynamic_name(
                &name,
                value.map(CompactString::from),
            ));
            element.attrs = attrs.into_boxed_slice();
        }
    }

    pub fn delete_attr(&mut self, id: NodeId, name: &str) {
        if let Some(element) = self.element_mut(id) {
            let mut attrs = element.attrs.to_vec();
            attrs.retain(|attr| attr.name() != name);
            element.attrs = attrs.into_boxed_slice();
        }
    }

    pub fn clear_attrs(&mut self, id: NodeId) {
        if let Some(element) = self.element_mut(id) {
            element.attrs = Vec::new().into_boxed_slice();
        }
    }

    pub fn pop_attr(&mut self, id: NodeId) -> Option<Attr> {
        let element = self.element_mut(id)?;
        let mut attrs = element.attrs.to_vec();
        let popped = attrs.pop()?;
        element.attrs = attrs.into_boxed_slice();
        Some(popped)
    }

    pub fn attrs_map(&self, id: NodeId) -> HashMap<String, Option<String>> {
        self.element(id)
            .map(|element| attrs_to_map(&element.attrs))
            .unwrap_or_default()
    }

    #[inline]
    pub fn element_attrs(&self, id: NodeId) -> &[Attr] {
        self.element(id)
            .map(|element| element.attrs.as_ref())
            .unwrap_or(&[])
    }

    pub fn child_nodes(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut child = self.node(id).first_child;
        while let Some(current) = child {
            out.push(current);
            child = self.node(current).next_sibling;
        }
        out
    }

    pub fn child_count(&self, id: NodeId) -> usize {
        let mut count = 0;
        let mut child = self.node(id).first_child;
        while let Some(current) = child {
            count += 1;
            child = self.node(current).next_sibling;
        }
        count
    }

    pub fn descendant_elements(&self, root: NodeId, include_self: bool) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut current = if include_self {
            Some(root)
        } else {
            self.node(root).first_child
        };

        while let Some(id) = current {
            if self.is_element(id) {
                out.push(id);
            }
            current = self.next_in_subtree(root, id);
        }
        out
    }

    pub fn find_descendant_element(
        &self,
        root: NodeId,
        include_self: bool,
        mut predicate: impl FnMut(NodeId) -> bool,
    ) -> Option<NodeId> {
        let mut current = if include_self {
            Some(root)
        } else {
            self.node(root).first_child
        };

        while let Some(id) = current {
            if self.is_element(id) && predicate(id) {
                return Some(id);
            }
            current = self.next_in_subtree(root, id);
        }
        None
    }

    pub fn descendant_nodes(&self, root: NodeId, include_self: bool) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut current = if include_self {
            Some(root)
        } else {
            self.node(root).first_child
        };

        while let Some(id) = current {
            out.push(id);
            current = self.next_in_subtree(root, id);
        }
        out
    }

    pub fn sibling_nodes_after(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut sibling = self.node(id).next_sibling;
        while let Some(current) = sibling {
            out.push(current);
            sibling = self.node(current).next_sibling;
        }
        out
    }

    pub fn sibling_nodes_before(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut sibling = self.node(id).prev_sibling;
        while let Some(current) = sibling {
            out.push(current);
            sibling = self.node(current).prev_sibling;
        }
        out
    }

    pub fn parent_nodes(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut parent = self.node(id).parent;
        while let Some(current) = parent {
            out.push(current);
            parent = self.node(current).parent;
        }
        out
    }

    pub fn next_element_node(&self, id: NodeId) -> Option<NodeId> {
        if let Some(child) = self.node(id).first_child {
            return Some(child);
        }
        let mut current = id;
        loop {
            if let Some(sibling) = self.node(current).next_sibling {
                return Some(sibling);
            }
            current = self.node(current).parent?;
        }
    }

    #[inline]
    pub fn next_in_subtree(&self, root: NodeId, id: NodeId) -> Option<NodeId> {
        if let Some(child) = self.node(id).first_child {
            return Some(child);
        }

        let mut current = id;
        while current != root {
            if let Some(sibling) = self.node(current).next_sibling {
                return Some(sibling);
            }
            current = self.node(current).parent?;
        }
        None
    }

    fn next_after_subtree(&self, root: NodeId, id: NodeId) -> Option<NodeId> {
        let mut current = id;
        while current != root {
            if let Some(sibling) = self.node(current).next_sibling {
                return Some(sibling);
            }
            current = self.node(current).parent?;
        }
        None
    }

    pub fn next_element_nodes(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut current = self.next_element_node(id);
        while let Some(id) = current {
            out.push(id);
            current = self.next_element_node(id);
        }
        out
    }

    pub fn previous_element_node(&self, id: NodeId) -> Option<NodeId> {
        if let Some(previous) = self.node(id).prev_sibling {
            return Some(self.deepest_last_child(previous));
        }
        self.node(id)
            .parent
            .filter(|id| !matches!(self.node(*id).node_type, NodeType::Document))
    }

    pub fn previous_element_nodes(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut current = self.previous_element_node(id);
        while let Some(id) = current {
            out.push(id);
            current = self.previous_element_node(id);
        }
        out
    }

    fn deepest_last_child(&self, mut id: NodeId) -> NodeId {
        while let Some(child) = self.node(id).last_child {
            id = child;
        }
        id
    }

    pub fn tag_string_node(&self, id: NodeId) -> Option<NodeId> {
        let mut current = id;
        loop {
            let first = self.node(current).first_child?;
            if self.node(first).next_sibling.is_some() {
                return None;
            }
            if self.is_text_like(first) {
                return Some(first);
            }
            if !self.is_element(first) {
                return None;
            }
            current = first;
        }
    }

    pub fn smooth_text_nodes(&mut self, root: NodeId) {
        let nodes = self.descendant_nodes(root, true);
        for id in nodes.into_iter().rev() {
            if id == root || self.node(id).parent.is_some() {
                self.merge_adjacent_text_children(id);
            }
        }
    }

    fn merge_adjacent_text_children(&mut self, parent: NodeId) {
        let mut current = self.node(parent).first_child;
        while let Some(id) = current {
            if matches!(self.node(id).node_type, NodeType::Text(_)) {
                let mut next = self.node(id).next_sibling;
                while let Some(next_id) = next {
                    let extra = match &self.node(next_id).node_type {
                        NodeType::Text(text) => text.clone(),
                        _ => break,
                    };
                    next = self.node(next_id).next_sibling;
                    if let NodeType::Text(text) = &mut self.nodes[id.index()].node_type {
                        text.push_str(extra.as_str());
                    }
                    self.detach(next_id);
                }
            }
            current = self.node(id).next_sibling;
        }
    }

    pub fn text(&self, id: NodeId, separator: &str, strip: bool) -> String {
        let include_template = self.is_template_element(id);
        self.text_with_options(
            id,
            separator,
            strip,
            true,
            true,
            false,
            include_template,
            false,
            false,
            false,
            false,
            false,
            false,
            true,
        )
    }

    #[inline]
    pub fn is_template_element(&self, id: NodeId) -> bool {
        self.element(id)
            .is_some_and(|element| element.tag_name() == "template")
    }

    #[allow(clippy::too_many_arguments)]
    pub fn text_with_options(
        &self,
        id: NodeId,
        separator: &str,
        strip: bool,
        include_text: bool,
        include_cdata: bool,
        include_declaration: bool,
        include_template: bool,
        include_comments: bool,
        include_script: bool,
        include_stylesheet: bool,
        include_raw_text: bool,
        include_doctype: bool,
        include_processing_instruction: bool,
        include_root_raw_text: bool,
    ) -> String {
        let mut out = String::new();
        let mut first = true;
        let mut current = Some(id);

        while let Some(current_id) = current {
            if current_id != id
                && let Some(name) = self.raw_text_element_name(current_id)
            {
                let include = include_raw_text
                    || (name == "script" && include_script)
                    || (name == "style" && include_stylesheet);
                if !include {
                    current = self.next_after_subtree(id, current_id);
                    continue;
                }
            }

            match &self.node(current_id).node_type {
                NodeType::Text(text) => {
                    let include = match self.raw_text_parent_name(current_id) {
                        Some("script") => {
                            include_script
                                || include_raw_text
                                || (include_root_raw_text
                                    && include_text
                                    && self.node(current_id).parent == Some(id))
                        }
                        Some("style") => {
                            include_stylesheet
                                || include_raw_text
                                || (include_root_raw_text
                                    && include_text
                                    && self.node(current_id).parent == Some(id))
                        }
                        _ => include_text,
                    };
                    if include {
                        push_text_value(text, separator, strip, &mut first, &mut out);
                    }
                }
                NodeType::CData(text) if include_cdata => {
                    push_text_value(text, separator, strip, &mut first, &mut out);
                }
                NodeType::Declaration(text) if include_declaration => {
                    push_text_value(text, separator, strip, &mut first, &mut out);
                }
                NodeType::TemplateString(text) if include_template => {
                    push_text_value(text, separator, strip, &mut first, &mut out);
                }
                NodeType::Comment(text) if include_comments => {
                    push_text_value(text, separator, strip, &mut first, &mut out);
                }
                NodeType::Doctype(data) if include_doctype => {
                    push_text_value(&data.name, separator, strip, &mut first, &mut out);
                }
                NodeType::ProcessingInstruction(data)
                    if include_processing_instruction && data.data.is_empty() =>
                {
                    push_text_value(&data.target, separator, strip, &mut first, &mut out);
                }
                NodeType::ProcessingInstruction(data) if include_processing_instruction => {
                    let mut value = data.target.to_string();
                    value.push(' ');
                    value.push_str(&data.data);
                    push_string_value(&value, separator, strip, &mut first, &mut out);
                }
                _ => {}
            }

            current = self.next_in_subtree(id, current_id);
        }
        out
    }

    pub fn outer_html(&self, id: NodeId) -> String {
        self.outer_html_with_options(id, true)
    }

    pub fn outer_html_with_options(&self, id: NodeId, escape: bool) -> String {
        self.outer_html_with_encoding_options(id, escape, "utf-8")
    }

    pub fn outer_html_with_encoding_options(
        &self,
        id: NodeId,
        escape: bool,
        eventual_encoding: &str,
    ) -> String {
        let mut formatter = SerializationFormatter::infallible(escape, eventual_encoding);
        self.outer_html_with_formatter_state(id, &mut formatter)
            .unwrap_or_else(|never| match never {})
    }

    pub fn outer_html_with_callback_formatter_and_encoding<E>(
        &self,
        id: NodeId,
        callback: &mut dyn FnMut(&str) -> Result<String, E>,
        eventual_encoding: &str,
    ) -> Result<String, E> {
        let mut formatter = SerializationFormatter::Callback {
            callback,
            eventual_encoding,
        };
        self.outer_html_with_formatter_state(id, &mut formatter)
    }

    fn outer_html_with_formatter_state<E>(
        &self,
        id: NodeId,
        formatter: &mut SerializationFormatter<'_, E>,
    ) -> Result<String, E> {
        let mut out = String::new();
        self.serialize_node(id, formatter, &mut out)?;
        Ok(out)
    }

    pub fn inner_html_with_encoding_options(
        &self,
        id: NodeId,
        escape: bool,
        eventual_encoding: &str,
    ) -> String {
        let mut formatter = SerializationFormatter::infallible(escape, eventual_encoding);
        self.inner_html_with_formatter_state(id, &mut formatter)
            .unwrap_or_else(|never| match never {})
    }

    pub fn inner_html_with_callback_formatter_and_encoding<E>(
        &self,
        id: NodeId,
        callback: &mut dyn FnMut(&str) -> Result<String, E>,
        eventual_encoding: &str,
    ) -> Result<String, E> {
        let mut formatter = SerializationFormatter::Callback {
            callback,
            eventual_encoding,
        };
        self.inner_html_with_formatter_state(id, &mut formatter)
    }

    fn inner_html_with_formatter_state<E>(
        &self,
        id: NodeId,
        formatter: &mut SerializationFormatter<'_, E>,
    ) -> Result<String, E> {
        let mut out = String::new();
        self.serialize_children(id, formatter, &mut out)?;
        Ok(out)
    }

    fn serialize_children<E>(
        &self,
        id: NodeId,
        formatter: &mut SerializationFormatter<'_, E>,
        out: &mut String,
    ) -> Result<(), E> {
        let mut stack = SmallVec::<[SerializeFrame; 64]>::new();
        self.push_serialization_children(id, false, &mut stack);
        self.serialize_frames(stack, formatter, out)
    }

    fn serialize_node<E>(
        &self,
        id: NodeId,
        formatter: &mut SerializationFormatter<'_, E>,
        out: &mut String,
    ) -> Result<(), E> {
        let mut stack = SmallVec::<[SerializeFrame; 64]>::new();
        stack.push(SerializeFrame::Node(id));
        self.serialize_frames(stack, formatter, out)
    }

    fn serialize_frames<E>(
        &self,
        mut stack: SmallVec<[SerializeFrame; 64]>,
        formatter: &mut SerializationFormatter<'_, E>,
        out: &mut String,
    ) -> Result<(), E> {
        while let Some(frame) = stack.pop() {
            match frame {
                SerializeFrame::Node(id) => match &self.node(id).node_type {
                    NodeType::Document => self.push_serialization_children(id, true, &mut stack),
                    NodeType::Element(element) => {
                        out.push('<');
                        out.push_str(element.tag_name());
                        let mut attrs = self.element_attrs(id).iter().collect::<Vec<_>>();
                        attrs.sort_by(|left, right| left.name.cmp(&right.name));
                        let is_content_type_meta = element.tag_name() == "meta"
                            && attrs.iter().any(|attr| {
                                attr.name() == "http-equiv"
                                    && attr.value.as_ref().is_some_and(|value| {
                                        value.eq_ignore_ascii_case("content-type")
                                    })
                            });
                        for attr in attrs {
                            out.push(' ');
                            out.push_str(attr.name());
                            if let Some(value) = &attr.value {
                                out.push_str("=\"");
                                if let Some(substituted) = substitute_meta_charset_attr(
                                    element.tag_name(),
                                    attr.name(),
                                    value,
                                    is_content_type_meta,
                                    formatter.eventual_encoding(),
                                ) {
                                    formatter.write_attr(&substituted, out)?;
                                } else {
                                    formatter.write_attr(value, out)?;
                                }
                                out.push('"');
                            }
                        }
                        if is_void_element(element.tag_name())
                            && self.node(id).first_child.is_none()
                        {
                            out.push_str("/>");
                        } else {
                            out.push('>');
                            stack.push(SerializeFrame::CloseElement(id));
                            self.push_serialization_children(id, false, &mut stack);
                        }
                    }
                    NodeType::Text(text) => {
                        if self.text_has_raw_text_parent(id) {
                            out.push_str(text);
                        } else {
                            formatter.write_text(text, out)?;
                        }
                    }
                    NodeType::CData(text) => {
                        out.push_str("<![CDATA[");
                        out.push_str(text);
                        out.push_str("]]>");
                    }
                    NodeType::Declaration(text) => {
                        out.push_str("<?");
                        out.push_str(text);
                        out.push_str("?>");
                    }
                    NodeType::TemplateString(text) => {
                        formatter.write_text(text, out)?;
                    }
                    NodeType::Comment(text) => {
                        out.push_str("<!--");
                        out.push_str(text);
                        out.push_str("-->");
                    }
                    NodeType::Doctype(data) => {
                        out.push_str("<!DOCTYPE ");
                        out.push_str(&data.name);
                        if !data.public_id.is_empty() {
                            out.push_str(" PUBLIC \"");
                            formatter.write_attr(&data.public_id, out)?;
                            out.push('"');
                            if !data.system_id.is_empty() {
                                out.push_str(" \"");
                                formatter.write_attr(&data.system_id, out)?;
                                out.push('"');
                            }
                        } else if !data.system_id.is_empty() {
                            out.push_str(" SYSTEM \"");
                            formatter.write_attr(&data.system_id, out)?;
                            out.push('"');
                        }
                        out.push('>');
                    }
                    NodeType::ProcessingInstruction(data) => {
                        out.push_str("<?");
                        out.push_str(&data.target);
                        if !data.data.is_empty() {
                            out.push(' ');
                            out.push_str(&data.data);
                        }
                        out.push('>');
                    }
                },
                SerializeFrame::CloseElement(id) => {
                    if let Some(element) = self.element(id) {
                        out.push_str("</");
                        out.push_str(element.tag_name());
                        out.push('>');
                    }
                }
                SerializeFrame::DocumentDoctypeNewline => out.push('\n'),
            }
        }
        Ok(())
    }

    fn push_serialization_children(
        &self,
        id: NodeId,
        doctype_newline: bool,
        stack: &mut SmallVec<[SerializeFrame; 64]>,
    ) {
        let parent_adds_doctype_newline =
            doctype_newline && matches!(self.node(id).node_type, NodeType::Document);
        let mut child = self.node(id).last_child;
        while let Some(current) = child {
            if parent_adds_doctype_newline
                && matches!(self.node(current).node_type, NodeType::Doctype(_))
            {
                stack.push(SerializeFrame::DocumentDoctypeNewline);
            }
            stack.push(SerializeFrame::Node(current));
            child = self.node(current).prev_sibling;
        }
    }

    pub fn prettify_with_options(&self, id: NodeId, escape: bool) -> String {
        let mut formatter = SerializationFormatter::infallible(escape, "utf-8");
        self.prettify_with_formatter_state(id, &mut formatter)
            .unwrap_or_else(|never| match never {})
    }

    pub fn prettify_with_callback_formatter<E>(
        &self,
        id: NodeId,
        callback: &mut dyn FnMut(&str) -> Result<String, E>,
    ) -> Result<String, E> {
        let mut formatter = SerializationFormatter::Callback {
            callback,
            eventual_encoding: "utf-8",
        };
        self.prettify_with_formatter_state(id, &mut formatter)
    }

    fn prettify_with_formatter_state<E>(
        &self,
        id: NodeId,
        formatter: &mut SerializationFormatter<'_, E>,
    ) -> Result<String, E> {
        let mut out = String::new();
        let mut stack = SmallVec::<[PrettifyFrame; 64]>::new();
        stack.push(PrettifyFrame::Node(id, 0));
        while let Some(frame) = stack.pop() {
            match frame {
                PrettifyFrame::Node(id, depth) => {
                    self.prettify_node(id, depth, formatter, &mut out, &mut stack)?;
                }
                PrettifyFrame::CloseElement(id, depth) => {
                    let indent = " ".repeat(depth);
                    if let Some(element) = self.element(id) {
                        out.push_str(&indent);
                        out.push_str("</");
                        out.push_str(element.tag_name());
                        out.push_str(">\n");
                    }
                }
            }
        }
        Ok(out)
    }

    fn prettify_node<E>(
        &self,
        id: NodeId,
        depth: usize,
        formatter: &mut SerializationFormatter<'_, E>,
        out: &mut String,
        stack: &mut SmallVec<[PrettifyFrame; 64]>,
    ) -> Result<(), E> {
        match &self.node(id).node_type {
            NodeType::Document => {
                let mut child = self.node(id).last_child;
                while let Some(current) = child {
                    stack.push(PrettifyFrame::Node(current, depth));
                    child = self.node(current).prev_sibling;
                }
            }
            NodeType::Element(element) => {
                let indent = " ".repeat(depth);
                out.push_str(&indent);
                out.push('<');
                out.push_str(element.tag_name());
                let mut attrs = self.element_attrs(id).iter().collect::<Vec<_>>();
                attrs.sort_by(|left, right| left.name.cmp(&right.name));
                for attr in attrs {
                    out.push(' ');
                    out.push_str(attr.name());
                    if let Some(value) = &attr.value {
                        out.push_str("=\"");
                        formatter.write_attr(value, out)?;
                        out.push('"');
                    }
                }
                if is_void_element(element.tag_name()) && self.node(id).first_child.is_none() {
                    out.push_str("/>\n");
                    return Ok(());
                }
                out.push_str(">\n");
                stack.push(PrettifyFrame::CloseElement(id, depth));
                let mut child = self.node(id).last_child;
                while let Some(current) = child {
                    stack.push(PrettifyFrame::Node(current, depth + 1));
                    child = self.node(current).prev_sibling;
                }
            }
            NodeType::Text(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    out.push_str(&" ".repeat(depth));
                    if self.text_has_raw_text_parent(id) {
                        out.push_str(trimmed);
                    } else {
                        formatter.write_text(trimmed, out)?;
                    }
                    out.push('\n');
                }
            }
            NodeType::CData(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    out.push_str(&" ".repeat(depth));
                    out.push_str("<![CDATA[");
                    out.push_str(trimmed);
                    out.push_str("]]>\n");
                }
            }
            NodeType::Declaration(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    out.push_str(&" ".repeat(depth));
                    out.push_str("<?");
                    out.push_str(trimmed);
                    out.push_str("?>\n");
                }
            }
            NodeType::TemplateString(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    out.push_str(&" ".repeat(depth));
                    formatter.write_text(trimmed, out)?;
                    out.push('\n');
                }
            }
            NodeType::Comment(text) => {
                out.push_str(&" ".repeat(depth));
                out.push_str("<!--");
                out.push_str(text);
                out.push_str("-->\n");
            }
            _ => {
                out.push_str(&" ".repeat(depth));
                self.serialize_node(id, formatter, out)?;
                out.push('\n');
            }
        }
        Ok(())
    }

    #[inline]
    pub fn is_raw_text_element_node(&self, id: NodeId) -> bool {
        self.element(id)
            .is_some_and(|element| is_raw_text_element(element.tag_name()))
    }

    pub fn raw_text_element_name(&self, id: NodeId) -> Option<&str> {
        self.element(id)
            .map(ElementData::tag_name)
            .filter(|name| is_raw_text_element(name))
    }

    pub fn raw_text_parent_name(&self, id: NodeId) -> Option<&str> {
        self.node(id)
            .parent
            .and_then(|parent| self.raw_text_element_name(parent))
    }

    pub fn is_inside_skipped_raw_text_element(&self, root: NodeId, id: NodeId) -> bool {
        let mut current = self.node(id).parent;
        while let Some(parent) = current {
            if parent == root {
                return false;
            }
            if self.is_raw_text_element_node(parent) {
                return true;
            }
            current = self.node(parent).parent;
        }
        false
    }

    #[inline]
    fn text_has_raw_text_parent(&self, id: NodeId) -> bool {
        self.node(id)
            .parent
            .is_some_and(|parent| self.is_raw_text_element_node(parent))
    }
}

enum SerializationFormatter<'a, E> {
    Escaped {
        eventual_encoding: &'a str,
    },
    Raw {
        eventual_encoding: &'a str,
    },
    Callback {
        callback: &'a mut dyn FnMut(&str) -> Result<String, E>,
        eventual_encoding: &'a str,
    },
}

impl<'a> SerializationFormatter<'a, Infallible> {
    fn infallible(escape: bool, eventual_encoding: &'a str) -> Self {
        if escape {
            Self::Escaped { eventual_encoding }
        } else {
            Self::Raw { eventual_encoding }
        }
    }
}

impl<E> SerializationFormatter<'_, E> {
    fn eventual_encoding(&self) -> &str {
        match self {
            Self::Escaped { eventual_encoding }
            | Self::Raw { eventual_encoding }
            | Self::Callback {
                eventual_encoding, ..
            } => eventual_encoding,
        }
    }

    fn write_attr(&mut self, value: &str, out: &mut String) -> Result<(), E> {
        match self {
            Self::Escaped { .. } => escape_attr(value, out),
            Self::Raw { .. } => out.push_str(value),
            Self::Callback { callback, .. } => out.push_str(&callback(value)?),
        }
        Ok(())
    }

    fn write_text(&mut self, value: &str, out: &mut String) -> Result<(), E> {
        match self {
            Self::Escaped { .. } => escape_text(value, out),
            Self::Raw { .. } => out.push_str(value),
            Self::Callback { callback, .. } => out.push_str(&callback(value)?),
        }
        Ok(())
    }
}

fn substitute_meta_charset_attr(
    tag_name: &str,
    attr_name: &str,
    value: &str,
    is_content_type_meta: bool,
    eventual_encoding: &str,
) -> Option<String> {
    if tag_name != "meta" {
        return None;
    }
    if attr_name == "charset" {
        return Some(eventual_encoding.to_string());
    }
    if attr_name == "content" && is_content_type_meta {
        return substitute_content_charset(value, eventual_encoding);
    }
    None
}

fn substitute_content_charset(value: &str, eventual_encoding: &str) -> Option<String> {
    let index = find_ascii_case_insensitive(value, "charset=")?;
    let value_start = index + "charset=".len();
    let mut value_end = value_start;
    for (offset, ch) in value[value_start..].char_indices() {
        if matches!(ch, ';' | '"' | '\'' | ' ' | '\t' | '\n' | '\r' | '\x0c') {
            break;
        }
        value_end = value_start + offset + ch.len_utf8();
    }
    let mut out = String::with_capacity(value.len() + eventual_encoding.len());
    out.push_str(&value[..value_start]);
    out.push_str(eventual_encoding);
    out.push_str(&value[value_end..]);
    Some(out)
}

fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn push_text_value(
    text: &CompactString,
    separator: &str,
    strip: bool,
    first: &mut bool,
    out: &mut String,
) {
    push_string_value(text.as_str(), separator, strip, first, out);
}

fn push_string_value(text: &str, separator: &str, strip: bool, first: &mut bool, out: &mut String) {
    let value = if strip { text.trim() } else { text };
    if !value.is_empty() {
        if *first {
            *first = false;
        } else {
            out.push_str(separator);
        }
        out.push_str(value);
    }
}

pub(crate) fn is_void_element(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

pub(crate) fn is_raw_text_element(name: &str) -> bool {
    matches!(name, "script" | "style")
}

fn escape_text(input: &str, out: &mut String) {
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

fn escape_attr(input: &str, out: &mut String) {
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
