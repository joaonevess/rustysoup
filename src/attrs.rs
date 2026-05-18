use compact_str::CompactString;
use markup5ever::{LocalName, Namespace};
use std::borrow::Cow;
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attr {
    pub name: LocalName,
    pub value: Option<CompactString>,
}

impl Attr {
    pub fn new(name: LocalName, _namespace: Namespace, value: impl Into<CompactString>) -> Self {
        Self {
            name,
            value: Some(value.into()),
        }
    }

    pub fn with_optional_dynamic_name(name: &str, value: Option<CompactString>) -> Self {
        Self {
            name: LocalName::from(Cow::Borrowed(name)),
            value,
        }
    }

    #[inline(always)]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    #[inline(always)]
    pub fn namespace(&self) -> &str {
        ""
    }
}

pub fn attrs_to_map(attrs: &[Attr]) -> HashMap<String, Option<String>> {
    attrs
        .iter()
        .map(|attr| {
            (
                attr.name().to_string(),
                attr.value.as_ref().map(ToString::to_string),
            )
        })
        .collect()
}

pub fn dedupe_attrs_last_wins(attrs: Vec<Attr>) -> Box<[Attr]> {
    if attrs.len() <= 1 {
        return attrs.into_boxed_slice();
    }

    let mut out = Vec::with_capacity(attrs.len());
    for attr in attrs.into_iter().rev() {
        if !out.iter().any(|existing: &Attr| existing.name == attr.name) {
            out.push(attr);
        }
    }
    out.reverse();
    out.into_boxed_slice()
}
