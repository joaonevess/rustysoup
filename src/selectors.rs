use crate::dom::{Document, HTML_NAMESPACE_URL, NodeId, NodeType};
use cssparser::{CssStringWriter, Parser as CssParser, ParserInput, ToCss, serialize_identifier};
use precomputed_hash::PrecomputedHash;
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::context::{
    MatchingContext, MatchingForInvalidation, MatchingMode, NeedsSelectorFlags, QuirksMode,
    SelectorCaches,
};
use selectors::matching::{ElementSelectorFlags, matches_selector_list};
use selectors::parser::{
    NonTSPseudoClass, ParseRelative, Parser, PseudoElement, SelectorImpl, SelectorList,
    SelectorParseErrorKind,
};
use selectors::{Element, OpaqueElement};
use std::borrow::Borrow;
use std::collections::hash_map::DefaultHasher;
use std::fmt::{self, Write};
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct Atom(String);

impl Atom {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Atom {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for Atom {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for Atom {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl ToCss for Atom {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        serialize_identifier(self.as_str(), dest)
    }
}

impl PrecomputedHash for Atom {
    fn precomputed_hash(&self) -> u32 {
        let mut hasher = DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish() as u32
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AttrValue(String);

impl AsRef<str> for AttrValue {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for AttrValue {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl ToCss for AttrValue {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        dest.write_str("\"")?;
        write!(CssStringWriter::new(dest), "{}", self.0)?;
        dest.write_str("\"")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedPseudoClass {}

impl NonTSPseudoClass for UnsupportedPseudoClass {
    type Impl = RustySelectorImpl;

    fn is_active_or_hover(&self) -> bool {
        match *self {}
    }

    fn is_user_action_state(&self) -> bool {
        match *self {}
    }
}

impl ToCss for UnsupportedPseudoClass {
    fn to_css<W>(&self, _dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        match *self {}
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedPseudoElement {}

impl PseudoElement for UnsupportedPseudoElement {
    type Impl = RustySelectorImpl;
}

impl ToCss for UnsupportedPseudoElement {
    fn to_css<W>(&self, _dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        match *self {}
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustySelectorImpl;

impl SelectorImpl for RustySelectorImpl {
    type ExtraMatchingData<'a> = ();
    type AttrValue = AttrValue;
    type Identifier = Atom;
    type LocalName = Atom;
    type NamespaceUrl = Atom;
    type NamespacePrefix = Atom;
    type BorrowedLocalName = str;
    type BorrowedNamespaceUrl = str;
    type NonTSPseudoClass = UnsupportedPseudoClass;
    type PseudoElement = UnsupportedPseudoElement;
}

#[derive(Default)]
pub struct RustySelectorParser;

impl<'i> Parser<'i> for RustySelectorParser {
    type Impl = RustySelectorImpl;
    type Error = SelectorParseErrorKind<'i>;
}

pub type CompiledSelector = SelectorList<RustySelectorImpl>;

pub fn compile(selector: &str) -> Result<CompiledSelector, ()> {
    let mut input = ParserInput::new(selector);
    let mut parser_input = CssParser::new(&mut input);
    let parser = RustySelectorParser;
    let list =
        SelectorList::parse(&parser, &mut parser_input, ParseRelative::No).map_err(|_| ())?;
    parser_input.expect_exhausted().map_err(|_| ())?;
    Ok(list)
}

#[derive(Clone)]
pub struct DomElement<'a> {
    pub document: &'a Document,
    pub id: NodeId,
}

impl fmt::Debug for DomElement<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DomElement")
            .field("id", &self.id)
            .field(
                "name",
                &self
                    .document
                    .element(self.id)
                    .map(|element| element.tag_name()),
            )
            .finish()
    }
}

impl Element for DomElement<'_> {
    type Impl = RustySelectorImpl;

    fn opaque(&self) -> OpaqueElement {
        OpaqueElement::new(&self.document.nodes[self.id.index()])
    }

    fn parent_element(&self) -> Option<Self> {
        self.document.parent_element(self.id).map(|id| Self {
            document: self.document,
            id,
        })
    }

    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }

    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }

    fn is_pseudo_element(&self) -> bool {
        false
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.document.prev_sibling_element(self.id).map(|id| Self {
            document: self.document,
            id,
        })
    }

    fn next_sibling_element(&self) -> Option<Self> {
        self.document.next_sibling_element(self.id).map(|id| Self {
            document: self.document,
            id,
        })
    }

    fn first_element_child(&self) -> Option<Self> {
        self.document.first_element_child(self.id).map(|id| Self {
            document: self.document,
            id,
        })
    }

    fn is_html_element_in_html_document(&self) -> bool {
        self.document.is_element(self.id)
            && self.document.element_namespace(self.id).as_ref() == HTML_NAMESPACE_URL
    }

    fn has_local_name(&self, local_name: &str) -> bool {
        self.document
            .element(self.id)
            .is_some_and(|element| element.tag_name() == local_name)
    }

    fn has_namespace(&self, ns: &str) -> bool {
        self.document.is_element(self.id) && self.document.element_namespace(self.id).as_ref() == ns
    }

    fn is_same_type(&self, other: &Self) -> bool {
        let Some(left) = self.document.element(self.id) else {
            return false;
        };
        let Some(right) = other.document.element(other.id) else {
            return false;
        };
        left.tag_name() == right.tag_name()
            && self.document.element_namespace(self.id)
                == other.document.element_namespace(other.id)
    }

    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&Atom>,
        local_name: &Atom,
        operation: &AttrSelectorOperation<&AttrValue>,
    ) -> bool {
        if !self.document.is_element(self.id) {
            return false;
        }
        self.document.element_attrs(self.id).iter().any(|attr| {
            let namespace_matches = match ns {
                NamespaceConstraint::Any => true,
                NamespaceConstraint::Specific(namespace) => attr.namespace() == namespace.as_str(),
            };
            namespace_matches
                && attr.name() == local_name.as_str()
                && operation.eval_str(attr.value.as_deref().unwrap_or(""))
        })
    }

    fn match_non_ts_pseudo_class(
        &self,
        _pc: &UnsupportedPseudoClass,
        _context: &mut MatchingContext<RustySelectorImpl>,
    ) -> bool {
        false
    }

    fn match_pseudo_element(
        &self,
        _pe: &UnsupportedPseudoElement,
        _context: &mut MatchingContext<RustySelectorImpl>,
    ) -> bool {
        false
    }

    fn apply_selector_flags(&self, _flags: ElementSelectorFlags) {}

    fn is_link(&self) -> bool {
        self.document.element(self.id).is_some_and(|element| {
            element.tag_name() == "a" && self.document.attr(self.id, "href").is_some()
        })
    }

    fn is_html_slot_element(&self) -> bool {
        self.document
            .element(self.id)
            .is_some_and(|element| element.tag_name() == "slot")
    }

    fn has_id(&self, id: &Atom, case_sensitivity: CaseSensitivity) -> bool {
        self.document
            .attr(self.id, "id")
            .is_some_and(|value| case_sensitivity.eq(value.as_bytes(), id.as_str().as_bytes()))
    }

    fn has_class(&self, name: &Atom, case_sensitivity: CaseSensitivity) -> bool {
        self.document.attr(self.id, "class").is_some_and(|value| {
            value
                .split_ascii_whitespace()
                .any(|class| case_sensitivity.eq(class.as_bytes(), name.as_str().as_bytes()))
        })
    }

    fn has_custom_state(&self, _name: &Atom) -> bool {
        false
    }

    fn imported_part(&self, _name: &Atom) -> Option<Atom> {
        None
    }

    fn is_part(&self, _name: &Atom) -> bool {
        false
    }

    fn is_empty(&self) -> bool {
        let mut child = self.document.node(self.id).first_child;
        while let Some(current) = child {
            match &self.document.node(current).node_type {
                NodeType::Element(_) => return false,
                NodeType::Text(text)
                | NodeType::CData(text)
                | NodeType::Declaration(text)
                | NodeType::TemplateString(text)
                    if !text.is_empty() =>
                {
                    return false;
                }
                _ => {}
            }
            child = self.document.node(current).next_sibling;
        }
        true
    }

    fn is_root(&self) -> bool {
        self.document.node(self.id).parent.is_some_and(|parent| {
            matches!(self.document.node(parent).node_type, NodeType::Document)
        })
    }

    fn add_element_unique_hashes(&self, _filter: &mut selectors::bloom::BloomFilter) -> bool {
        false
    }
}

pub fn matches(document: &Document, id: NodeId, selector: &CompiledSelector) -> bool {
    let element = DomElement { document, id };
    let mut caches = SelectorCaches::default();
    let mut context = MatchingContext::new(
        MatchingMode::Normal,
        None,
        &mut caches,
        QuirksMode::NoQuirks,
        NeedsSelectorFlags::No,
        MatchingForInvalidation::No,
    );
    matches_selector_list(selector, &element, &mut context)
}
