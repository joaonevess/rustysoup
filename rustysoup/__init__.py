"""Fast HTML parsing with BeautifulSoup-style ergonomics."""

from abc import ABCMeta
from collections import Counter
from html import escape as _escape_html
from importlib.metadata import PackageNotFoundError, version as _metadata_version

from ._rustysoup import (
    Soup,
    Tag,
    _NavigableString,
    _new_cdata_node,
    _new_comment_node,
    _new_declaration_node,
    _new_doctype_node,
    _new_processing_instruction_node,
    _new_template_string_node,
    _new_text_node,
)

try:
    __version__ = _metadata_version("rustysoup")
except PackageNotFoundError:
    __version__ = "0.0.0"


class FeatureNotFound(ValueError):
    """Raised when the requested BeautifulSoup parser feature is unavailable."""


class ParserRejectedMarkup(Exception):
    """Raised when a parser rejects markup before a tree can be built."""


class StopParsing(Exception):
    """Signal used by parser integrations to stop parsing early."""


class UnusualUsageWarning(UserWarning):
    """Base class for BeautifulSoup-compatible usage warnings."""


class GuessedAtParserWarning(UnusualUsageWarning):
    """Warning used when a parser is guessed rather than explicit."""


class MarkupResemblesLocatorWarning(UnusualUsageWarning):
    """Warning used when markup looks like a filename or URL."""


class AttributeResemblesVariableWarning(UnusualUsageWarning):
    """Warning used when an attribute name looks like Python syntax."""


class XMLParsedAsHTMLWarning(UnusualUsageWarning):
    """Warning used when XML-like markup is parsed as HTML."""


class PageElement(metaclass=ABCMeta):
    """Virtual base class for bs4-style page elements."""


class ResultSet(list):
    """List subclass returned by find_all-style APIs, matching bs4's shape."""

    def __init__(self, source=None, result=(), source_factory=None):
        super().__init__(result)
        self._source = source
        self._source_factory = source_factory

    @property
    def source(self):
        if self._source is None and self._source_factory is not None:
            self._source = self._source_factory()
            self._source_factory = None
        return self._source

    @source.setter
    def source(self, value):
        self._source = value
        self._source_factory = None

    def __getattr__(self, key):
        raise AttributeError(
            f'ResultSet object has no attribute "{key}". You\'re probably treating a list of elements like a single element. Did you call find_all() when you meant to call find()?'
        )


class AttributeValueList(list):
    """List-like multi-valued attribute value that writes mutations back."""

    def __init__(self, values=(), owner=None, key=None):
        super().__init__(values)
        self._owner = owner
        self._key = key

    def _sync(self):
        if self._owner is not None and self._key is not None:
            self._owner[self._key] = [str(value) for value in self]

    def append(self, value):
        super().append(value)
        self._sync()

    def extend(self, values):
        super().extend(values)
        self._sync()

    def insert(self, index, value):
        super().insert(index, value)
        self._sync()

    def remove(self, value):
        super().remove(value)
        self._sync()

    def pop(self, index=-1):
        value = super().pop(index)
        self._sync()
        return value

    def clear(self):
        super().clear()
        self._sync()

    def reverse(self):
        super().reverse()
        self._sync()

    def sort(self, *args, **kwargs):
        super().sort(*args, **kwargs)
        self._sync()

    def __setitem__(self, key, value):
        super().__setitem__(key, value)
        self._sync()

    def __delitem__(self, key):
        super().__delitem__(key)
        self._sync()

    def __iadd__(self, values):
        result = super().__iadd__(values)
        self._sync()
        return result


class NavigableString(str):
    """A small bs4-compatible string wrapper backed by a Rust text node."""

    _node_factory = staticmethod(_new_text_node)

    def __new__(cls, inner=""):
        if isinstance(inner, _NavigableString):
            backing = inner
        else:
            backing = cls._node_factory(str(inner))
        obj = str.__new__(cls, str(backing))
        obj._inner = backing
        return obj

    @property
    def name(self):
        return None

    @property
    def parent(self):
        return self._inner.parent

    @property
    def parents(self):
        return self._inner.parents

    @property
    def hidden(self):
        return self._inner.hidden

    @property
    def decomposed(self):
        return self._inner.decomposed

    @property
    def known_xml(self):
        return self._inner.known_xml

    @property
    def string(self):
        return self

    @property
    def text(self):
        return str(self)

    @property
    def next(self):
        return self._inner.next

    @property
    def previous(self):
        return self._inner.previous

    @property
    def next_element(self):
        return self._inner.next_element

    @property
    def next_elements(self):
        return self._inner.next_elements

    @property
    def previous_element(self):
        return self._inner.previous_element

    @property
    def previous_elements(self):
        return self._inner.previous_elements

    @property
    def self_and_next_elements(self):
        return self._with_self(self._inner.next_elements)

    @property
    def self_and_previous_elements(self):
        return self._with_self(self._inner.previous_elements)

    @property
    def self_and_parents(self):
        return self._with_self(self._inner.parents)

    @property
    def next_sibling(self):
        return self._inner.next_sibling

    @property
    def previous_sibling(self):
        return self._inner.previous_sibling

    @property
    def next_siblings(self):
        return self._inner.next_siblings

    @property
    def previous_siblings(self):
        return self._inner.previous_siblings

    @property
    def self_and_next_siblings(self):
        return self._with_self(self._inner.next_siblings)

    @property
    def self_and_previous_siblings(self):
        return self._with_self(self._inner.previous_siblings)

    @property
    def nextSibling(self):
        return self._inner.nextSibling

    @property
    def previousSibling(self):
        return self._inner.previousSibling

    def nextGenerator(self):
        return self._inner.nextGenerator()

    def previousGenerator(self):
        return self._inner.previousGenerator()

    def parentGenerator(self):
        return self._inner.parentGenerator()

    def nextSiblingGenerator(self):
        return self._inner.nextSiblingGenerator()

    def previousSiblingGenerator(self):
        return self._inner.previousSiblingGenerator()

    @property
    def strings(self):
        return [self] if type(self) in (NavigableString, CData) else []

    @property
    def stripped_strings(self):
        if type(self) not in (NavigableString, CData):
            return []
        stripped = str(self).strip()
        return [stripped] if stripped else []

    def get_text(self, separator="", strip=False, *args, **kwargs):
        del separator
        types = _text_types_from_args(args, kwargs)
        if not _string_matches_text_type(self, types):
            return ""
        return str(self).strip() if strip else str(self)

    def getText(self, separator="", strip=False, *args, **kwargs):
        return self.get_text(separator, strip, *args, **kwargs)

    def _with_self(self, nodes):
        return [self, *nodes]

    def find_next(self, name=None, attrs=None, string=None, **kwargs):
        return self._inner.find_next(name, attrs, string, **kwargs)

    def findNext(self, name=None, attrs=None, string=None, **kwargs):
        return self.find_next(name, attrs, string, **kwargs)

    def find_all_next(self, name=None, attrs=None, string=None, limit=None, **kwargs):
        return self._inner.find_all_next(name, attrs, string, limit, **kwargs)

    def findAllNext(self, name=None, attrs=None, string=None, limit=None, **kwargs):
        return self.find_all_next(name, attrs, string, limit, **kwargs)

    def find_previous(self, name=None, attrs=None, string=None, **kwargs):
        return self._inner.find_previous(name, attrs, string, **kwargs)

    def findPrevious(self, name=None, attrs=None, string=None, **kwargs):
        return self.find_previous(name, attrs, string, **kwargs)

    def find_all_previous(self, name=None, attrs=None, string=None, limit=None, **kwargs):
        return self._inner.find_all_previous(name, attrs, string, limit, **kwargs)

    def findAllPrevious(self, name=None, attrs=None, string=None, limit=None, **kwargs):
        return self.find_all_previous(name, attrs, string, limit, **kwargs)

    def fetchAllPrevious(self, name=None, attrs=None, string=None, limit=None, **kwargs):
        return self.find_all_previous(name, attrs, string, limit, **kwargs)

    def find_parent(self, name=None, attrs=None, string=None, **kwargs):
        return self._inner.find_parent(name, attrs, string, **kwargs)

    def findParent(self, name=None, attrs=None, string=None, **kwargs):
        return self.find_parent(name, attrs, string, **kwargs)

    def find_parents(self, name=None, attrs=None, string=None, limit=None, **kwargs):
        return self._inner.find_parents(name, attrs, string, limit, **kwargs)

    def findParents(self, name=None, attrs=None, string=None, limit=None, **kwargs):
        return self.find_parents(name, attrs, string, limit, **kwargs)

    def fetchParents(self, name=None, attrs=None, string=None, limit=None, **kwargs):
        return self.find_parents(name, attrs, string, limit, **kwargs)

    def find_next_sibling(self, name=None, attrs=None, string=None, **kwargs):
        return self._inner.find_next_sibling(name, attrs, string, **kwargs)

    def findNextSibling(self, name=None, attrs=None, string=None, **kwargs):
        return self.find_next_sibling(name, attrs, string, **kwargs)

    def find_next_siblings(self, name=None, attrs=None, string=None, limit=None, **kwargs):
        return self._inner.find_next_siblings(name, attrs, string, limit, **kwargs)

    def findNextSiblings(self, name=None, attrs=None, string=None, limit=None, **kwargs):
        return self.find_next_siblings(name, attrs, string, limit, **kwargs)

    def fetchNextSiblings(self, name=None, attrs=None, string=None, limit=None, **kwargs):
        return self.find_next_siblings(name, attrs, string, limit, **kwargs)

    def find_previous_sibling(self, name=None, attrs=None, string=None, **kwargs):
        return self._inner.find_previous_sibling(name, attrs, string, **kwargs)

    def findPreviousSibling(self, name=None, attrs=None, string=None, **kwargs):
        return self.find_previous_sibling(name, attrs, string, **kwargs)

    def find_previous_siblings(self, name=None, attrs=None, string=None, limit=None, **kwargs):
        return self._inner.find_previous_siblings(name, attrs, string, limit, **kwargs)

    def findPreviousSiblings(self, name=None, attrs=None, string=None, limit=None, **kwargs):
        return self.find_previous_siblings(name, attrs, string, limit, **kwargs)

    def fetchPreviousSiblings(self, name=None, attrs=None, string=None, limit=None, **kwargs):
        return self.find_previous_siblings(name, attrs, string, limit, **kwargs)

    def wrap(self, wrapper):
        return self._inner.wrap(wrapper)

    def replace_with(self, item):
        self._inner.replace_with(item)
        return self

    def replaceWith(self, item):
        return self.replace_with(item)

    def extract(self):
        self._inner.extract()
        return self

    def decompose(self):
        self._inner.decompose()

    def insert_before(self, item):
        return [self._inner.insert_before(item)]

    def insert_after(self, item):
        return [self._inner.insert_after(item)]


class Comment(NavigableString):
    """BeautifulSoup-compatible comment string wrapper."""

    _node_factory = staticmethod(_new_comment_node)


class PreformattedString(NavigableString):
    """Base class for string nodes serialized with explicit wrappers."""


class CData(PreformattedString):
    """BeautifulSoup-compatible CDATA string wrapper."""

    _node_factory = staticmethod(_new_cdata_node)


class Declaration(PreformattedString):
    """BeautifulSoup-compatible declaration string wrapper."""

    _node_factory = staticmethod(_new_declaration_node)


class Doctype(NavigableString):
    """BeautifulSoup-compatible doctype string wrapper."""

    _node_factory = staticmethod(_new_doctype_node)


class ProcessingInstruction(NavigableString):
    """BeautifulSoup-compatible processing instruction string wrapper."""

    _node_factory = staticmethod(_new_processing_instruction_node)


class TemplateString(NavigableString):
    """BeautifulSoup-compatible template string wrapper."""

    _node_factory = staticmethod(_new_template_string_node)


class RubyTextString(TemplateString):
    """BeautifulSoup-compatible ruby text string wrapper."""


class RubyParenthesisString(TemplateString):
    """BeautifulSoup-compatible ruby parenthesis string wrapper."""


class Script(NavigableString):
    """BeautifulSoup-compatible script string wrapper."""


class Stylesheet(NavigableString):
    """BeautifulSoup-compatible stylesheet string wrapper."""


class ElementFilter:
    """An efficient BeautifulSoup-compatible element filter used by high-performance search APIs."""

    def __init__(self, match_function=None):
        self.match_function = match_function

    @property
    def includes_everything(self):
        return not self.match_function

    @property
    def excludes_everything(self):
        return False

    def match(self, element, _known_rules=False):
        if not _known_rules and self.includes_everything:
            return True
        if not self.match_function:
            return True
        return self.match_function(element)

    def filter(self, generator):
        if self.includes_everything:
            yield from generator
            return
        for element in generator:
            if element and self.match(element, _known_rules=True):
                yield element

    def find(self, generator):
        for match in self.filter(generator):
            return match
        return None

    def find_all(self, generator, limit=None):
        results = []
        for match in self.filter(generator):
            results.append(match)
            if limit is not None and len(results) >= limit:
                break
        return ResultSet(self, results)

    def allow_tag_creation(self, nsprefix, name, attrs):
        del nsprefix, name, attrs
        return True

    def allow_string_creation(self, string):
        del string
        return True


class SoupStrainer(ElementFilter):
    """Small parse-only filter compatible with common bs4 SoupStrainer usage."""

    def __init__(self, name=None, attrs=None, string=None, **kwargs):
        super().__init__(None)
        if string is None and "text" in kwargs:
            string = kwargs.pop("text")

        if attrs is None:
            attrs = {}
        elif isinstance(attrs, dict):
            attrs = dict(attrs)
        else:
            attrs = {"class": attrs}

        for key, value in kwargs.items():
            attrs["class" if key == "class_" else key] = value

        attrs = {
            key: (False if value is None else value) for key, value in attrs.items()
        }

        self.name = name
        self.attrs = attrs
        self.string = string
        self.text = string
        self._present_name_rule = name is None and not attrs and not string
        self._has_name_rule = self._present_name_rule or name is not None
        self._has_string_rule = string is not None
        self._rustysoup_parse_only = (name, attrs, string)

    @property
    def includes_everything(self):
        return not self._has_name_rule and not self._has_string_rule and not self.attrs

    @property
    def excludes_everything(self):
        if self._has_string_rule and (self._has_name_rule or self.attrs):
            return True
        if self._has_name_rule and _rule_excludes_everything(self.name):
            return True
        if self._has_string_rule and _rule_excludes_everything(self.string):
            return True
        return any(_rule_excludes_everything(value) for value in self.attrs.values())

    def matches_any_string_rule(self, string):
        if not self._has_string_rule:
            return True
        return _matches_value_rule(self.string, string)

    def matches_tag(self, tag):
        if not _is_tag_like(tag):
            return False
        if not self._has_name_rule and not self.attrs:
            return False
        if not self._matches_tag_name(tag):
            return False
        for attr, rule in self.attrs.items():
            if not _attribute_match(_tag_attr_value(tag, attr), rule):
                return False
        if self._has_string_rule:
            value = getattr(tag, "string", None)
            if value is None or not self.matches_any_string_rule(value):
                return False
        return True

    def match(self, element, _known_rules=False):
        if not _known_rules and self.includes_everything:
            return True
        if _is_tag_like(element):
            return self.matches_tag(element)
        if self._has_name_rule or self.attrs:
            return False
        return self.matches_any_string_rule(element)

    def search(self, element):
        return element if self.match(element) else None

    def allow_tag_creation(self, nsprefix, name, attrs):
        if self._has_string_rule:
            return False
        if not self._matches_tag_name_value(nsprefix, name):
            return False
        attrs = attrs or {}
        for attr, rule in self.attrs.items():
            if not _attribute_match(_mapping_get(attrs, attr), rule):
                return False
        return True

    def allow_string_creation(self, string):
        if self._has_name_rule or self.attrs:
            return False
        return self.matches_any_string_rule(string)

    def _matches_tag_name(self, tag):
        if self._present_name_rule:
            return getattr(tag, "name", None) is not None
        if self.name is None:
            return True
        return _matches_name_rule(self.name, tag)

    def _matches_tag_name_value(self, nsprefix, name):
        if self._present_name_rule:
            return name is not None
        if self.name is None:
            return True
        prefixed = f"{nsprefix}:{name}" if nsprefix else None
        return _matches_name_value_rule(self.name, name, prefixed)

    def __repr__(self):
        return f"<SoupStrainer name={self.name!r} attrs={self.attrs!r} string={self.string!r}>"


def _is_regex_like(value):
    return hasattr(value, "search")


def _is_tag_like(value):
    return getattr(value, "name", None) is not None


def _is_iterable_rule(value):
    return not isinstance(value, (str, bytes)) and hasattr(value, "__iter__")


def _iterable_is_empty(value):
    try:
        return len(value) == 0
    except TypeError:
        iterator = iter(value)
        try:
            next(iterator)
        except StopIteration:
            return True
        return False


def _rule_excludes_everything(rule):
    if rule is None or isinstance(rule, (str, bytes, bool)) or callable(rule) or _is_regex_like(rule):
        return False
    if not _is_iterable_rule(rule):
        return False
    if _iterable_is_empty(rule):
        return True
    for value in rule:
        if _is_iterable_rule(value):
            return True
    return False


def _matches_value_rule(rule, value):
    if isinstance(rule, bytes):
        rule = rule.decode("utf8")
    if isinstance(rule, str):
        return value is not None and rule == str(value)
    if isinstance(rule, bool):
        return value is not None if rule else value is None
    if rule is None:
        return value is None
    if _is_regex_like(rule):
        return value is not None and rule.search(str(value)) is not None
    if callable(rule):
        return bool(rule(value))
    if _is_iterable_rule(rule):
        if _iterable_is_empty(rule):
            return False
        for item in rule:
            if _is_iterable_rule(item):
                return False
            if _matches_value_rule(item, value):
                return True
        return False
    return value is not None and str(rule) == str(value)


def _matches_name_rule(rule, tag):
    if callable(rule) and not _is_regex_like(rule):
        return bool(rule(tag))
    if _is_iterable_rule(rule):
        if _iterable_is_empty(rule):
            return False
        for item in rule:
            if _is_iterable_rule(item):
                return False
            if _matches_name_rule(item, tag):
                return True
        return False
    return _matches_value_rule(rule, getattr(tag, "name", None))


def _matches_name_value_rule(rule, name, prefixed_name=None):
    if _is_iterable_rule(rule):
        if _iterable_is_empty(rule):
            return False
        for item in rule:
            if _is_iterable_rule(item):
                return False
            if _matches_name_value_rule(item, name, prefixed_name):
                return True
        return False
    return _matches_value_rule(rule, name) or (
        prefixed_name is not None and _matches_value_rule(rule, prefixed_name)
    )


def _attribute_values(value):
    if isinstance(value, (list, tuple, AttributeValueList)):
        return list(value)
    return [value]


def _attribute_match(value, rule):
    values = _attribute_values(value)
    for item in values:
        if _matches_value_rule(rule, item):
            return True
    if len(values) != 1 and all(item is not None for item in values):
        return _matches_value_rule(rule, " ".join(str(item) for item in values))
    return False


def _mapping_get(mapping, key):
    if hasattr(mapping, "get"):
        return mapping.get(key)
    try:
        return dict(mapping).get(key)
    except (TypeError, ValueError):
        return None


def _tag_attr_value(tag, attr):
    getter = getattr(tag, "get", None)
    if getter is not None:
        return getter(attr)
    attrs = getattr(tag, "attrs", {})
    return _mapping_get(attrs, attr)


_SEARCH_CONTROL_KWARGS = {
    "recursive",
    "limit",
    "namespaces",
    "flags",
    "kwargs",
}


def _search_result_source(args, kwargs):
    kwargs = dict(kwargs or {})
    name = kwargs.pop("name", args[0] if len(args) >= 1 else None)
    attrs = kwargs.pop("attrs", args[1] if len(args) >= 2 else None)
    string = kwargs.pop("string", kwargs.pop("text", None))
    for key in _SEARCH_CONTROL_KWARGS:
        kwargs.pop(key, None)
    return SoupStrainer(name, attrs, string, **kwargs)


def _result_set_from_search(result, args, kwargs):
    return ResultSet(
        None, result, source_factory=lambda: _search_result_source(args, kwargs)
    )


def _wrap_result_set_method(method):
    def wrapped(self, *args, **kwargs):
        return _result_set_from_search(method(self, *args, **kwargs), args, kwargs)

    wrapped.__name__ = getattr(method, "__name__", "wrapped")
    wrapped.__doc__ = getattr(method, "__doc__", None)
    return wrapped


def _wrap_result_set_into_method(method):
    def wrapped(self, *args, **kwargs):
        result = ResultSet(
            None, source_factory=lambda: _search_result_source(args, kwargs)
        )
        method(self, result, *args, **kwargs)
        return result

    wrapped.__name__ = getattr(method, "__name__", "wrapped")
    wrapped.__doc__ = getattr(method, "__doc__", None)
    return wrapped


def _wrap_selector_result_set_method(method):
    def wrapped(self, *args, **kwargs):
        return ResultSet(None, method(self, *args, **kwargs))

    wrapped.__name__ = getattr(method, "__name__", "wrapped")
    wrapped.__doc__ = getattr(method, "__doc__", None)
    return wrapped


def _wrap_selector_result_set_into_method(method):
    def wrapped(self, *args, **kwargs):
        result = ResultSet(None)
        method(self, result, *args, **kwargs)
        return result

    wrapped.__name__ = getattr(method, "__name__", "wrapped")
    wrapped.__doc__ = getattr(method, "__doc__", None)
    return wrapped


class CSS:
    """Small bs4-compatible CSS proxy for Soup and Tag objects."""

    def __init__(self, owner):
        self.api = owner

    def select(self, select, namespaces=None, limit=0, flags=0, **kwargs):
        del flags
        return self.api.select(select, namespaces=namespaces, limit=limit, **kwargs)

    def select_one(self, select, namespaces=None, flags=0, **kwargs):
        del flags
        return self.api.select_one(select, namespaces=namespaces, **kwargs)

    def iselect(self, select, namespaces=None, limit=0, flags=0, **kwargs):
        return iter(self.select(select, namespaces, limit, flags, **kwargs))

    def filter(self, select, namespaces=None, flags=0, **kwargs):
        del namespaces, flags, kwargs
        return ResultSet(
            None,
            [
                node
                for node in getattr(self.api, "contents", [])
                if hasattr(node, "_matches_selector") and node._matches_selector(select)
            ],
        )

    def match(self, select, namespaces=None, flags=0, **kwargs):
        del namespaces, flags, kwargs
        matcher = getattr(self.api, "_matches_selector", None)
        return False if matcher is None else matcher(select)

    def closest(self, select, namespaces=None, flags=0, **kwargs):
        del namespaces, flags, kwargs
        node = self.api
        while node is not None and getattr(node, "name", None) != "[document]":
            matcher = getattr(node, "_matches_selector", None)
            if matcher is not None and matcher(select):
                return node
            node = getattr(node, "parent", None)
        return None

    def escape(self, ident):
        out = []
        for char in str(ident):
            if char.isalnum() or char in ("_", "-"):
                out.append(char)
            else:
                out.append("\\" + char)
        return "".join(out)


class HTMLFormatter:
    """Minimal formatter object compatible with bs4's common formatter API."""

    def __init__(self, name="minimal"):
        self.name = name

    def substitute(self, value):
        if self.name is None:
            return str(value)
        return _escape_html(str(value), quote=False)


class UnicodeDammit:
    """Small UnicodeDammit-compatible decoder for common bytes/str inputs."""

    def __init__(
        self,
        markup,
        known_definite_encodings=None,
        smart_quotes_to=None,
        is_html=False,
        exclude_encodings=None,
        user_encodings=None,
        override_encodings=None,
    ):
        del smart_quotes_to, is_html
        self.markup = markup
        self.contains_replacement_characters = False
        self.original_encoding = None

        if isinstance(markup, str):
            self.unicode_markup = markup
            return

        encodings = []
        for values in (known_definite_encodings, user_encodings, override_encodings):
            if values:
                encodings.extend(values)
        encodings.extend(("utf-8", "windows-1252", "latin-1"))
        excluded = {encoding.lower() for encoding in (exclude_encodings or ())}

        data = bytes(markup)
        for encoding in encodings:
            if encoding.lower() in excluded:
                continue
            try:
                self.unicode_markup = data.decode(encoding)
                self.original_encoding = encoding
                return
            except UnicodeDecodeError:
                pass

        self.unicode_markup = data.decode("utf-8", "replace")
        self.original_encoding = "utf-8"
        self.contains_replacement_characters = True

    @classmethod
    def detwingle(cls, in_bytes, main_encoding="utf8", embedded_encoding="windows-1252"):
        del cls, main_encoding, embedded_encoding
        return bytes(in_bytes)


class _TreeTraversalEvent:
    pass


class HTMLParserTreeBuilder:
    pass


class TreeBuilder:
    pass


class _BuilderRegistry:
    def lookup(self, *features):
        del features
        return HTMLParserTreeBuilder

    def register(self, treebuilder_class):
        del treebuilder_class
        return None


_SOUP_STATE = {}


def _soup_state(soup):
    return _SOUP_STATE.setdefault(
        id(soup),
        {
            "current_data": [],
            "tag_stack": [soup],
            "open_tag_counter": Counter({tag.name: 0 for tag in soup.find_all(True)}),
        },
    )


def _setup(self, parent=None, previous_element=None, next_element=None, previous_sibling=None, next_sibling=None):
    del self, parent, previous_element, next_element, previous_sibling, next_sibling
    return None


def _formatter_for_name(self, formatter):
    del self
    if formatter is None or formatter == "minimal":
        return HTMLFormatter(formatter)
    if hasattr(formatter, "substitute"):
        return formatter
    return HTMLFormatter(formatter)


def _format_string(self, s, formatter="minimal"):
    if formatter is None:
        return str(s)
    if callable(formatter):
        return formatter(str(s))
    return self.formatter_for_name(formatter).substitute(s)


def _string_output_ready(self, formatter="minimal"):
    return f"{self.PREFIX}{self.format_string(str(self), formatter)}{self.SUFFIX}"


def _raw_string_output_ready(self, formatter="minimal"):
    del formatter
    return f"{self.PREFIX}{str(self)}{self.SUFFIX}"


def _text_types_from_args(args, kwargs):
    kwargs = dict(kwargs)
    if len(args) > 1:
        raise TypeError("get_text() takes from 1 to 4 positional arguments")
    if args and "types" in kwargs:
        raise TypeError("get_text() got multiple values for argument 'types'")
    if args:
        return args[0]
    return kwargs.pop("types", (NavigableString, CData))


def _string_matches_text_type(node, types):
    if types is None:
        return True
    if not isinstance(types, (list, tuple, set, frozenset)):
        types = (types,)
    return any(type(node) is candidate for candidate in types)


def _copy_tag_self(self):
    soup = Soup(str(self))
    copy = soup.find(self.name)
    if copy is not None:
        copy.clear()
    return copy


def _copy_soup_self(self):
    return Soup("")


def _soup_extract(self):
    return self


def _soup_insert_before_not_supported(self, *args, **kwargs):
    del self, args, kwargs
    raise NotImplementedError("BeautifulSoup objects don't support insert_before().")


def _soup_insert_after_not_supported(self, *args, **kwargs):
    del self, args, kwargs
    raise NotImplementedError("BeautifulSoup objects don't support insert_after().")


def _soup_replace_without_parent(self, *args, **kwargs):
    del self, args, kwargs
    raise ValueError(
        "Cannot replace one element with another when the element to be replaced is not part of a tree."
    )


def _soup_unwrap_without_parent(self, *args, **kwargs):
    del self, args, kwargs
    raise ValueError(
        "Cannot replace an element with its contents when that element is not part of a tree."
    )


def _soup_current_tag(self):
    stack = _soup_state(self)["tag_stack"]
    return stack[-1] if stack else self


def _soup_tag_stack(self):
    return _soup_state(self)["tag_stack"]


def _soup_current_data(self):
    return _soup_state(self)["current_data"]


def _soup_open_tag_counter(self):
    return _soup_state(self)["open_tag_counter"]


def _soup_handle_data(self, data):
    _soup_state(self)["current_data"].append(str(data))
    return None


def _soup_end_data(self, container_class=None):
    del container_class
    state = _soup_state(self)
    if state["current_data"]:
        text = "".join(state["current_data"])
        state["current_data"].clear()
        state["tag_stack"][-1].append(text)
    return None


def _soup_handle_starttag(
    self,
    name,
    namespace=None,
    nsprefix=None,
    attrs=None,
    sourceline=None,
    sourcepos=None,
    namespaces=None,
):
    del namespace, nsprefix, sourceline, sourcepos, namespaces
    tag = self.new_tag(name, attrs=attrs or {})
    state = _soup_state(self)
    parent = state["tag_stack"][-1] if state["tag_stack"] else self
    parent.append(tag)
    state["tag_stack"].append(tag)
    state["open_tag_counter"][name] += 1
    return tag


def _soup_handle_endtag(self, name, nsprefix=None):
    del nsprefix
    state = _soup_state(self)
    self.endData()
    if len(state["tag_stack"]) > 1:
        state["tag_stack"].pop()
    state["open_tag_counter"][name] -= 1
    return None


def _soup_push_tag(self, tag):
    state = _soup_state(self)
    state["tag_stack"].append(tag)
    state["open_tag_counter"][tag.name] += 1
    return None


def _soup_pop_tag(self):
    state = _soup_state(self)
    if not state["tag_stack"]:
        return None
    tag = state["tag_stack"].pop()
    if tag is not self:
        state["open_tag_counter"][tag.name] -= 1
    return tag


def _soup_object_was_parsed(self, o, parent=None, most_recent_element=None):
    del most_recent_element
    (parent or _soup_current_tag(self)).append(o)
    return None


def _soup_reset(self):
    self.clear()
    _SOUP_STATE[id(self)] = {
        "current_data": [],
        "tag_stack": [],
        "open_tag_counter": Counter(),
    }
    return None


def _soup_string_container(self, base_class=None):
    return NavigableString if base_class is None else base_class


_SOUP_DECODE_CONTENTS = Soup.decode_contents
_SOUP_ENCODE_CONTENTS = Soup.encode_contents
_SOUP_DECODE = Soup.decode
_SOUP_ENCODE = Soup.encode
_SOUP_PRETTIFY = Soup.prettify
_SOUP_INSERT = Soup.insert
_SOUP_CALL = Soup.__call__
_SOUP_FIND_ALL = Soup.find_all
_SOUP_FIND_ALL_INTO = Soup._find_all_into_result_set
_SOUP_FIND_ALL_LEGACY = Soup.findAll
_SOUP_FIND_CHILDREN_LEGACY = Soup.findChildren
_SOUP_FIND_ALL_NEXT = Soup.find_all_next
_SOUP_FIND_ALL_NEXT_LEGACY = Soup.findAllNext
_SOUP_FIND_ALL_PREVIOUS = Soup.find_all_previous
_SOUP_FIND_ALL_PREVIOUS_LEGACY = Soup.findAllPrevious
_SOUP_FETCH_ALL_PREVIOUS_LEGACY = Soup.fetchAllPrevious
_SOUP_FIND_PARENTS = Soup.find_parents
_SOUP_FIND_PARENTS_LEGACY = Soup.findParents
_SOUP_FETCH_PARENTS_LEGACY = Soup.fetchParents
_SOUP_FIND_NEXT_SIBLINGS = Soup.find_next_siblings
_SOUP_FIND_NEXT_SIBLINGS_LEGACY = Soup.findNextSiblings
_SOUP_FETCH_NEXT_SIBLINGS_LEGACY = Soup.fetchNextSiblings
_SOUP_FIND_PREVIOUS_SIBLINGS = Soup.find_previous_siblings
_SOUP_FIND_PREVIOUS_SIBLINGS_LEGACY = Soup.findPreviousSiblings
_SOUP_FETCH_PREVIOUS_SIBLINGS_LEGACY = Soup.fetchPreviousSiblings
_SOUP_SELECT = Soup.select
_SOUP_SELECT_INTO = Soup._select_into_result_set
_TAG_DECODE_CONTENTS = Tag.decode_contents
_TAG_ENCODE_CONTENTS = Tag.encode_contents
_TAG_DECODE = Tag.decode
_TAG_ENCODE = Tag.encode
_TAG_PRETTIFY = Tag.prettify
_TAG_INSERT = Tag.insert
_TAG_INSERT_BEFORE = Tag.insert_before
_TAG_INSERT_AFTER = Tag.insert_after
_TAG_REPLACE_WITH = Tag.replace_with
_TAG_CALL = Tag.__call__
_TAG_FIND_ALL = Tag.find_all
_TAG_FIND_ALL_INTO = Tag._find_all_into_result_set
_TAG_FIND_ALL_LEGACY = Tag.findAll
_TAG_FIND_CHILDREN_LEGACY = Tag.findChildren
_TAG_FIND_ALL_NEXT = Tag.find_all_next
_TAG_FIND_ALL_NEXT_LEGACY = Tag.findAllNext
_TAG_FIND_ALL_PREVIOUS = Tag.find_all_previous
_TAG_FIND_ALL_PREVIOUS_LEGACY = Tag.findAllPrevious
_TAG_FIND_PARENTS = Tag.find_parents
_TAG_FIND_PARENTS_LEGACY = Tag.findParents
_TAG_FETCH_PARENTS_LEGACY = Tag.fetchParents
_TAG_FIND_NEXT_SIBLINGS = Tag.find_next_siblings
_TAG_FIND_NEXT_SIBLINGS_LEGACY = Tag.findNextSiblings
_TAG_FETCH_NEXT_SIBLINGS_LEGACY = Tag.fetchNextSiblings
_TAG_FIND_PREVIOUS_SIBLINGS = Tag.find_previous_siblings
_TAG_FIND_PREVIOUS_SIBLINGS_LEGACY = Tag.findPreviousSiblings
_TAG_FETCH_PREVIOUS_SIBLINGS_LEGACY = Tag.fetchPreviousSiblings
_TAG_SELECT = Tag.select
_TAG_SELECT_INTO = Tag._select_into_result_set
_NAV_FIND_ALL_NEXT = NavigableString.find_all_next
_NAV_FIND_ALL_NEXT_LEGACY = NavigableString.findAllNext
_NAV_FIND_ALL_PREVIOUS = NavigableString.find_all_previous
_NAV_FIND_ALL_PREVIOUS_LEGACY = NavigableString.findAllPrevious
_NAV_FIND_PARENTS = NavigableString.find_parents
_NAV_FIND_PARENTS_LEGACY = NavigableString.findParents
_NAV_FETCH_PARENTS_LEGACY = NavigableString.fetchParents
_NAV_FIND_NEXT_SIBLINGS = NavigableString.find_next_siblings
_NAV_FIND_NEXT_SIBLINGS_LEGACY = NavigableString.findNextSiblings
_NAV_FETCH_NEXT_SIBLINGS_LEGACY = NavigableString.fetchNextSiblings
_NAV_FIND_PREVIOUS_SIBLINGS = NavigableString.find_previous_siblings
_NAV_FIND_PREVIOUS_SIBLINGS_LEGACY = NavigableString.findPreviousSiblings
_NAV_FETCH_PREVIOUS_SIBLINGS_LEGACY = NavigableString.fetchPreviousSiblings


def _soup_decode_contents(
    self, indent_level=None, eventual_encoding="utf-8", formatter="minimal"
):
    return _SOUP_DECODE_CONTENTS(self, indent_level, eventual_encoding, formatter)


def _soup_encode_contents(
    self, indent_level=None, encoding="utf-8", formatter="minimal"
):
    return _SOUP_ENCODE_CONTENTS(self, indent_level, encoding, formatter)


def _soup_decode(
    self,
    indent_level=None,
    eventual_encoding="utf-8",
    formatter="minimal",
    iterator=None,
    **kwargs,
):
    return _SOUP_DECODE(self, indent_level, eventual_encoding, formatter, iterator, **kwargs)


def _soup_encode(
    self,
    encoding="utf-8",
    indent_level=None,
    formatter="minimal",
    errors="xmlcharrefreplace",
):
    return _SOUP_ENCODE(self, encoding, indent_level, formatter, errors)


def _soup_prettify(self, encoding=None, formatter="minimal"):
    return _SOUP_PRETTIFY(self, encoding, formatter)


def _tag_decode_contents(
    self, indent_level=None, eventual_encoding="utf-8", formatter="minimal"
):
    return _TAG_DECODE_CONTENTS(self, indent_level, eventual_encoding, formatter)


def _tag_encode_contents(
    self, indent_level=None, encoding="utf-8", formatter="minimal"
):
    return _TAG_ENCODE_CONTENTS(self, indent_level, encoding, formatter)


def _tag_decode(
    self,
    indent_level=None,
    eventual_encoding="utf-8",
    formatter="minimal",
    iterator=None,
):
    return _TAG_DECODE(self, indent_level, eventual_encoding, formatter, iterator)


def _tag_encode(
    self,
    encoding="utf-8",
    indent_level=None,
    formatter="minimal",
    errors="xmlcharrefreplace",
):
    return _TAG_ENCODE(self, encoding, indent_level, formatter, errors)


def _tag_prettify(self, encoding=None, formatter="minimal"):
    return _TAG_PRETTIFY(self, encoding, formatter)


def _insert_many(method, self, index, items):
    if index < 0:
        raise IndexError("list index out of range")
    inserted = []
    for offset, item in enumerate(items):
        inserted.append(method(self, index + offset, item))
    return inserted


def _insert_before_many(method, self, items):
    return [method(self, item) for item in items]


def _insert_after_many(method, self, items):
    inserted = []
    for item in reversed(items):
        inserted.append(method(self, item))
    inserted.reverse()
    return inserted


def _soup_insert(self, position, *new_children):
    return _insert_many(_SOUP_INSERT, self, position, new_children)


def _tag_insert(self, position, *new_children):
    return _insert_many(_TAG_INSERT, self, position, new_children)


def _tag_insert_before(self, *args):
    return _insert_before_many(_TAG_INSERT_BEFORE, self, args)


def _tag_insert_after(self, *args):
    return _insert_after_many(_TAG_INSERT_AFTER, self, args)


def _tag_replace_with(self, *args):
    if len(args) == 1:
        return _TAG_REPLACE_WITH(self, args[0])
    self.insert_before(*args)
    return self.extract()


def _nav_insert_before(self, *args):
    return _insert_before_many(lambda node, item: node._inner.insert_before(item), self, args)


def _nav_insert_after(self, *args):
    return _insert_after_many(lambda node, item: node._inner.insert_after(item), self, args)


def _nav_replace_with(self, *args):
    if len(args) == 1:
        self._inner.replace_with(args[0])
        return self
    self.insert_before(*args)
    return self.extract()


_CDATA_LIST_ATTRIBUTES = {
    "*": {"class", "accesskey", "dropzone"},
    "a": {"rel", "rev"},
    "link": {"rel", "rev"},
    "td": {"headers"},
    "th": {"headers"},
    "form": {"accept-charset"},
    "object": {"archive"},
    "area": {"rel"},
    "icon": {"sizes"},
    "iframe": {"sandbox"},
    "output": {"for"},
}
_PRESERVE_WHITESPACE_TAGS = {"pre", "textarea"}
_MAIN_CONTENT_STRING_TYPES = {NavigableString, CData}
DEFAULT_OUTPUT_ENCODING = "utf-8"
PYTHON_SPECIFIC_ENCODINGS = {"idna", "mbcs", "oem", "palmos", "punycode", "raw_unicode_escape"}
Formatter = HTMLFormatter
builder_registry = _BuilderRegistry()


BeautifulSoup = Soup
BeautifulStoneSoup = Soup
Soup.css = property(lambda self: CSS(self))
Tag.css = property(lambda self: CSS(self))
Soup.ASCII_SPACES = " \n\t\f\r"
Soup.DEFAULT_BUILDER_FEATURES = ["html", "fast"]
Soup.ROOT_TAG_NAME = "[document]"
Soup.builder = HTMLParserTreeBuilder()
Soup.element_classes = {}
Soup.markup = None
Soup.parse_only = None
Soup.currentTag = property(_soup_current_tag)
Soup.current_data = property(_soup_current_data)
Soup.open_tag_counter = property(_soup_open_tag_counter)
Soup.preserve_whitespace_tag_stack = []
Soup.string_container_stack = []
Soup.tagStack = property(_soup_tag_stack)
Soup.handle_data = _soup_handle_data
Soup.endData = _soup_end_data
Soup.handle_starttag = _soup_handle_starttag
Soup.handle_endtag = _soup_handle_endtag
Soup.object_was_parsed = _soup_object_was_parsed
Soup.pushTag = _soup_push_tag
Soup.popTag = _soup_pop_tag
Soup.reset = _soup_reset
Soup.string_container = _soup_string_container
Soup.decode_contents = _soup_decode_contents
Soup.encode_contents = _soup_encode_contents
Soup.decode = _soup_decode
Soup.encode = _soup_encode
Soup.prettify = _soup_prettify
Soup.insert = _soup_insert
Soup.__call__ = _wrap_result_set_into_method(_SOUP_FIND_ALL_INTO)
Soup.find_all = _wrap_result_set_into_method(_SOUP_FIND_ALL_INTO)
Soup.findAll = _wrap_result_set_into_method(_SOUP_FIND_ALL_INTO)
Soup.findChildren = _wrap_result_set_into_method(_SOUP_FIND_ALL_INTO)
Soup.find_all_next = _wrap_result_set_method(_SOUP_FIND_ALL_NEXT)
Soup.findAllNext = _wrap_result_set_method(_SOUP_FIND_ALL_NEXT_LEGACY)
Soup.find_all_previous = _wrap_result_set_method(_SOUP_FIND_ALL_PREVIOUS)
Soup.findAllPrevious = _wrap_result_set_method(_SOUP_FIND_ALL_PREVIOUS_LEGACY)
Soup.fetchAllPrevious = _wrap_result_set_method(_SOUP_FETCH_ALL_PREVIOUS_LEGACY)
Soup.find_parents = _wrap_result_set_method(_SOUP_FIND_PARENTS)
Soup.findParents = _wrap_result_set_method(_SOUP_FIND_PARENTS_LEGACY)
Soup.fetchParents = _wrap_result_set_method(_SOUP_FETCH_PARENTS_LEGACY)
Soup.find_next_siblings = _wrap_result_set_method(_SOUP_FIND_NEXT_SIBLINGS)
Soup.findNextSiblings = _wrap_result_set_method(_SOUP_FIND_NEXT_SIBLINGS_LEGACY)
Soup.fetchNextSiblings = _wrap_result_set_method(_SOUP_FETCH_NEXT_SIBLINGS_LEGACY)
Soup.find_previous_siblings = _wrap_result_set_method(_SOUP_FIND_PREVIOUS_SIBLINGS)
Soup.findPreviousSiblings = _wrap_result_set_method(_SOUP_FIND_PREVIOUS_SIBLINGS_LEGACY)
Soup.fetchPreviousSiblings = _wrap_result_set_method(_SOUP_FETCH_PREVIOUS_SIBLINGS_LEGACY)
Soup.select = _wrap_selector_result_set_into_method(_SOUP_SELECT_INTO)
Tag.decode_contents = _tag_decode_contents
Tag.encode_contents = _tag_encode_contents
Tag.decode = _tag_decode
Tag.encode = _tag_encode
Tag.prettify = _tag_prettify
Tag.insert = _tag_insert
Tag.insert_before = _tag_insert_before
Tag.insert_after = _tag_insert_after
Tag.replace_with = _tag_replace_with
Tag.replaceWith = _tag_replace_with
Tag.__call__ = _wrap_result_set_into_method(_TAG_FIND_ALL_INTO)
Tag.find_all = _wrap_result_set_into_method(_TAG_FIND_ALL_INTO)
Tag.findAll = _wrap_result_set_into_method(_TAG_FIND_ALL_INTO)
Tag.findChildren = _wrap_result_set_into_method(_TAG_FIND_ALL_INTO)
Tag.find_all_next = _wrap_result_set_method(_TAG_FIND_ALL_NEXT)
Tag.findAllNext = _wrap_result_set_method(_TAG_FIND_ALL_NEXT_LEGACY)
Tag.find_all_previous = _wrap_result_set_method(_TAG_FIND_ALL_PREVIOUS)
Tag.findAllPrevious = _wrap_result_set_method(_TAG_FIND_ALL_PREVIOUS_LEGACY)
Tag.find_parents = _wrap_result_set_method(_TAG_FIND_PARENTS)
Tag.findParents = _wrap_result_set_method(_TAG_FIND_PARENTS_LEGACY)
Tag.fetchParents = _wrap_result_set_method(_TAG_FETCH_PARENTS_LEGACY)
Tag.find_next_siblings = _wrap_result_set_method(_TAG_FIND_NEXT_SIBLINGS)
Tag.findNextSiblings = _wrap_result_set_method(_TAG_FIND_NEXT_SIBLINGS_LEGACY)
Tag.fetchNextSiblings = _wrap_result_set_method(_TAG_FETCH_NEXT_SIBLINGS_LEGACY)
Tag.find_previous_siblings = _wrap_result_set_method(_TAG_FIND_PREVIOUS_SIBLINGS)
Tag.findPreviousSiblings = _wrap_result_set_method(_TAG_FIND_PREVIOUS_SIBLINGS_LEGACY)
Tag.fetchPreviousSiblings = _wrap_result_set_method(_TAG_FETCH_PREVIOUS_SIBLINGS_LEGACY)
Tag.select = _wrap_selector_result_set_into_method(_TAG_SELECT_INTO)
NavigableString.find_all_next = _wrap_result_set_method(_NAV_FIND_ALL_NEXT)
NavigableString.findAllNext = _wrap_result_set_method(_NAV_FIND_ALL_NEXT_LEGACY)
NavigableString.find_all_previous = _wrap_result_set_method(_NAV_FIND_ALL_PREVIOUS)
NavigableString.findAllPrevious = _wrap_result_set_method(_NAV_FIND_ALL_PREVIOUS_LEGACY)
NavigableString.find_parents = _wrap_result_set_method(_NAV_FIND_PARENTS)
NavigableString.findParents = _wrap_result_set_method(_NAV_FIND_PARENTS_LEGACY)
NavigableString.fetchParents = _wrap_result_set_method(_NAV_FETCH_PARENTS_LEGACY)
NavigableString.find_next_siblings = _wrap_result_set_method(_NAV_FIND_NEXT_SIBLINGS)
NavigableString.findNextSiblings = _wrap_result_set_method(_NAV_FIND_NEXT_SIBLINGS_LEGACY)
NavigableString.fetchNextSiblings = _wrap_result_set_method(_NAV_FETCH_NEXT_SIBLINGS_LEGACY)
NavigableString.find_previous_siblings = _wrap_result_set_method(_NAV_FIND_PREVIOUS_SIBLINGS)
NavigableString.findPreviousSiblings = _wrap_result_set_method(_NAV_FIND_PREVIOUS_SIBLINGS_LEGACY)
NavigableString.fetchPreviousSiblings = _wrap_result_set_method(_NAV_FETCH_PREVIOUS_SIBLINGS_LEGACY)
NavigableString.insert_before = _nav_insert_before
NavigableString.insert_after = _nav_insert_after
NavigableString.replace_with = _nav_replace_with
NavigableString.replaceWith = _nav_replace_with

for _cls in (Soup, Tag, NavigableString):
    _cls.default = ()
    _cls.setup = _setup
    _cls.formatter_for_name = _formatter_for_name
    _cls.format_string = _format_string

_START_ELEMENT_EVENT = _TreeTraversalEvent()
_END_ELEMENT_EVENT = _TreeTraversalEvent()
_EMPTY_ELEMENT_EVENT = _TreeTraversalEvent()
_STRING_ELEMENT_EVENT = _TreeTraversalEvent()

for _cls in (Soup, Tag):
    _cls.START_ELEMENT_EVENT = _START_ELEMENT_EVENT
    _cls.END_ELEMENT_EVENT = _END_ELEMENT_EVENT
    _cls.EMPTY_ELEMENT_EVENT = _EMPTY_ELEMENT_EVENT
    _cls.STRING_ELEMENT_EVENT = _STRING_ELEMENT_EVENT
    _cls.MAIN_CONTENT_STRING_TYPES = _MAIN_CONTENT_STRING_TYPES
    _cls.attribute_value_list_class = AttributeValueList
    _cls.cdata_list_attributes = _CDATA_LIST_ATTRIBUTES
    _cls.preserve_whitespace_tags = _PRESERVE_WHITESPACE_TAGS
    _cls.interesting_string_types = _MAIN_CONTENT_STRING_TYPES

Soup.copy_self = _copy_soup_self
Soup.extract = _soup_extract
Soup.insert_before = _soup_insert_before_not_supported
Soup.insert_after = _soup_insert_after_not_supported
Soup.replace_with = _soup_replace_without_parent
Soup.replaceWith = _soup_replace_without_parent
Soup.wrap = _soup_replace_without_parent
Soup.unwrap = _soup_unwrap_without_parent
Soup.replace_with_children = _soup_unwrap_without_parent
Soup.replaceWithChildren = _soup_unwrap_without_parent
Tag.copy_self = _copy_tag_self

PageElement.register(Soup)
PageElement.register(Tag)
PageElement.register(NavigableString)

NavigableString.PREFIX = ""
NavigableString.SUFFIX = ""
NavigableString.output_ready = _string_output_ready
Comment.PREFIX = "<!--"
Comment.SUFFIX = "-->"
Comment.output_ready = _raw_string_output_ready
CData.PREFIX = "<![CDATA["
CData.SUFFIX = "]]>"
CData.output_ready = _raw_string_output_ready
Declaration.PREFIX = "<?"
Declaration.SUFFIX = "?>"
Declaration.output_ready = _raw_string_output_ready
Doctype.PREFIX = "<!DOCTYPE "
Doctype.SUFFIX = ">\n"
Doctype.output_ready = _raw_string_output_ready
ProcessingInstruction.PREFIX = "<?"
ProcessingInstruction.SUFFIX = ">"
ProcessingInstruction.output_ready = _raw_string_output_ready
TemplateString.PREFIX = ""
TemplateString.SUFFIX = ""
TemplateString.output_ready = _string_output_ready
RubyTextString.PREFIX = ""
RubyTextString.SUFFIX = ""
RubyTextString.output_ready = _string_output_ready
RubyParenthesisString.PREFIX = ""
RubyParenthesisString.SUFFIX = ""
RubyParenthesisString.output_ready = _string_output_ready
Script.PREFIX = ""
Script.SUFFIX = ""
Script.output_ready = _raw_string_output_ready
Stylesheet.PREFIX = ""
Stylesheet.SUFFIX = ""
Stylesheet.output_ready = _raw_string_output_ready

__all__ = [
    "AttributeResemblesVariableWarning",
    "AttributeValueList",
    "BeautifulSoup",
    "BeautifulStoneSoup",
    "CData",
    "CSS",
    "Comment",
    "Declaration",
    "Doctype",
    "ElementFilter",
    "FeatureNotFound",
    "Formatter",
    "GuessedAtParserWarning",
    "HTMLFormatter",
    "MarkupResemblesLocatorWarning",
    "NavigableString",
    "PageElement",
    "ParserRejectedMarkup",
    "PreformattedString",
    "ProcessingInstruction",
    "ResultSet",
    "RubyParenthesisString",
    "RubyTextString",
    "Script",
    "Soup",
    "SoupStrainer",
    "StopParsing",
    "Stylesheet",
    "Tag",
    "TemplateString",
    "TreeBuilder",
    "UnicodeDammit",
    "UnusualUsageWarning",
    "XMLParsedAsHTMLWarning",
    "__version__",
]

from . import builder, css, dammit, element, exceptions, filter, formatter

