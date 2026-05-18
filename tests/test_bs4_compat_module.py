from bs4 import BeautifulSoup as Bs4BeautifulSoup
from bs4 import CData as Bs4CData
from bs4 import Comment as Bs4Comment
from bs4 import Declaration as Bs4Declaration
from bs4 import ElementFilter as Bs4ElementFilter
from bs4 import TemplateString as Bs4TemplateString
from bs4.element import PageElement as Bs4PageElement

import rustysoup
from rustysoup import (
    BeautifulSoup,
    CData,
    Comment,
    Declaration,
    ElementFilter,
    PageElement,
    TemplateString,
    UnicodeDammit,
)
from rustysoup.element import CData as ElementCData
from rustysoup.exceptions import GuessedAtParserWarning, ParserRejectedMarkup
from rustysoup.filter import ElementFilter as FilterElementFilter


def test_common_module_level_exports_match_bs4_shape():
    names = [
        "AttributeResemblesVariableWarning",
        "BeautifulSoup",
        "BeautifulStoneSoup",
        "CData",
        "Comment",
        "Declaration",
        "ElementFilter",
        "Formatter",
        "GuessedAtParserWarning",
        "MarkupResemblesLocatorWarning",
        "PageElement",
        "ParserRejectedMarkup",
        "StopParsing",
        "TemplateString",
        "TreeBuilder",
        "UnicodeDammit",
        "UnusualUsageWarning",
        "XMLParsedAsHTMLWarning",
        "builder",
        "css",
        "dammit",
        "element",
        "exceptions",
        "filter",
        "formatter",
    ]

    for name in names:
        assert hasattr(rustysoup, name)

    assert ElementCData is CData
    assert FilterElementFilter is ElementFilter
    assert issubclass(GuessedAtParserWarning, rustysoup.UnusualUsageWarning)
    assert issubclass(ParserRejectedMarkup, Exception)

    soup = BeautifulSoup("<p>x</p>", "html.parser")
    assert isinstance(soup, PageElement)
    assert isinstance(soup.p, PageElement)
    assert isinstance(soup.p.string, PageElement)


def test_element_filter_lower_level_api_matches_bs4():
    rusty = BeautifulSoup("<main><p>one</p><span>two</span></main>", "html.parser")
    bs4 = Bs4BeautifulSoup("<main><p>one</p><span>two</span></main>", "html.parser")

    rusty_filter = ElementFilter(lambda node: getattr(node, "name", None) == "p")
    bs4_filter = Bs4ElementFilter(lambda node: getattr(node, "name", None) == "p")

    assert str(rusty_filter.find(iter(rusty.descendants))) == str(
        bs4_filter.find(iter(bs4.descendants))
    )
    assert [str(node) for node in rusty_filter.find_all(iter(rusty.descendants))] == [
        str(node) for node in bs4_filter.find_all(iter(bs4.descendants))
    ]


def test_special_string_classes_new_string_and_serialization_match_bs4():
    cases = [
        (CData, Bs4CData),
        (Declaration, Bs4Declaration),
        (TemplateString, Bs4TemplateString),
        (Comment, Bs4Comment),
    ]

    for rusty_cls, bs4_cls in cases:
        rusty_direct = rusty_cls("a & b")
        bs4_direct = bs4_cls("a & b")
        assert rusty_direct.output_ready() == bs4_direct.output_ready()
        assert list(rusty_direct.strings) == list(bs4_direct.strings)
        assert list(rusty_direct.stripped_strings) == list(bs4_direct.stripped_strings)

        rusty = BeautifulSoup("<div></div>", "html.parser")
        bs4 = Bs4BeautifulSoup("<div></div>", "html.parser")
        rusty_node = rusty.new_string("a & b", rusty_cls)
        bs4_node = bs4.new_string("a & b", bs4_cls)

        assert type(rusty_node).__name__ == type(bs4_node).__name__
        assert rusty_node.output_ready() == bs4_node.output_ready()

        rusty.div.append(rusty_node)
        bs4.div.append(bs4_node)
        assert str(rusty.div) == str(bs4.div)
        assert list(rusty.div.strings) == list(bs4.div.strings)
        assert list(rusty.div.stripped_strings) == list(bs4.div.stripped_strings)
        assert rusty.div.get_text("|") == bs4.div.get_text("|")
        assert rusty.div.get_text("|", types=(rusty_cls,)) == bs4.div.get_text(
            "|", types=(bs4_cls,)
        )
        assert type(rusty.div.find(string=True)).__name__ == type(
            bs4.div.find(string=True)
        ).__name__


def test_unicode_dammit_common_decode_shape():
    rusty = UnicodeDammit("caf\xc3\xa9".encode("latin-1"))
    assert isinstance(rusty.unicode_markup, str)
    assert rusty.original_encoding is not None
    assert rusty.contains_replacement_characters is False
    assert UnicodeDammit.detwingle(b"abc") == b"abc"
