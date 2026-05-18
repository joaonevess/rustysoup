from bs4 import BeautifulSoup as Bs4BeautifulSoup

from rustysoup import BeautifulSoup


HTML = "<div><p>One <b>bold</b></p><p>Two</p><!-- note --><span>Three</span></div>"


def shape(nodes):
    return [(getattr(node, "name", None), str(node)) for node in nodes]


def test_contents_children_and_descendants_match_bs4_values():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert shape(rusty.div.contents) == shape(bs4.div.contents)
    assert shape(list(rusty.div.children)) == shape(list(bs4.div.children))
    assert shape(list(rusty.div.descendants)) == shape(list(bs4.div.descendants))


def test_tag_and_document_iteration_and_len_match_bs4():
    html = "<div><p>one</p><span>two</span></div><footer>tail</footer>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert shape(list(rusty)) == shape(list(bs4))
    assert len(rusty) == len(bs4)
    assert shape(list(rusty.div)) == shape(list(bs4.div))
    assert len(rusty.div) == len(bs4.div)
    assert shape(list(rusty.footer)) == shape(list(bs4.footer))
    assert len(rusty.footer) == len(bs4.footer)


def test_tag_and_document_equality_and_membership_match_bs4():
    html = "<div><p>one</p><span>two</span></div>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert (rusty.p == rusty.find("p")) == (bs4.p == bs4.find("p"))
    assert (rusty.p == BeautifulSoup(str(rusty.p), "html.parser").p) == (
        bs4.p == Bs4BeautifulSoup(str(bs4.p), "html.parser").p
    )
    assert (rusty == BeautifulSoup(str(rusty), "html.parser")) == (
        bs4 == Bs4BeautifulSoup(str(bs4), "html.parser")
    )
    assert (rusty.p != rusty.span) == (bs4.p != bs4.span)
    assert (rusty.p == "<p>one</p>") == (bs4.p == "<p>one</p>")
    assert (rusty.p in rusty.div) == (bs4.p in bs4.div)

    try:
        bs4.p < bs4.span
    except Exception as exc:
        expected_type = type(exc)
    else:
        expected_type = None
    if expected_type is not None:
        import pytest

        with pytest.raises(expected_type):
            rusty.p < rusty.span


def test_string_strings_and_stripped_strings_match_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert str(rusty.b.string) == str(bs4.b.string)
    assert rusty.div.string == bs4.div.string
    assert list(rusty.div.strings) == list(bs4.div.strings)
    assert list(rusty.div.stripped_strings) == list(bs4.div.stripped_strings)


def test_sibling_parent_and_element_traversal_match_bs4_values():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert shape(rusty.p.next_siblings) == shape(bs4.p.next_siblings)
    assert shape(rusty.span.previous_siblings) == shape(bs4.span.previous_siblings)
    assert [node.name for node in rusty.b.parents] == [node.name for node in bs4.b.parents]
    assert str(rusty.p.next_sibling) == str(bs4.p.next_sibling)
    assert str(rusty.span.previous_sibling) == str(bs4.span.previous_sibling)
    assert str(rusty.p.next_element) == str(bs4.p.next_element)
    assert str(rusty.b.previous_element) == str(bs4.b.previous_element)


def test_deprecated_navigation_alias_properties_match_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert str(rusty.p.next) == str(bs4.p.next)
    assert str(rusty.p.previous) == str(bs4.p.previous)
    assert str(rusty.p.nextSibling) == str(bs4.p.nextSibling)
    assert str(rusty.span.previousSibling) == str(bs4.span.previousSibling)
    assert str(rusty.p.contents[0].next) == str(bs4.p.contents[0].next)
    assert str(rusty.p.contents[0].previous) == str(bs4.p.contents[0].previous)
    assert str(rusty.p.contents[0].nextSibling) == str(bs4.p.contents[0].nextSibling)
    assert shape(rusty.p.contents[0].next_siblings) == shape(bs4.p.contents[0].next_siblings)
    assert shape(rusty.p.contents[0].previous_siblings) == shape(
        bs4.p.contents[0].previous_siblings
    )


def test_deprecated_generator_methods_match_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert shape(rusty.div.childGenerator()) == shape(bs4.div.childGenerator())
    assert shape(rusty.p.recursiveChildGenerator()) == shape(bs4.p.recursiveChildGenerator())
    assert shape(rusty.p.nextGenerator()) == shape(bs4.p.nextGenerator())
    assert shape(rusty.span.previousGenerator()) == shape(bs4.span.previousGenerator())
    assert [node.name for node in rusty.b.parentGenerator()] == [
        node.name for node in bs4.b.parentGenerator()
    ]
    assert shape(rusty.p.contents[0].nextGenerator()) == shape(
        bs4.p.contents[0].nextGenerator()
    )
    assert [node.name for node in rusty.p.contents[0].parentGenerator()] == [
        node.name for node in bs4.p.contents[0].parentGenerator()
    ]


def test_select_one_and_has_attr_match_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert str(rusty.select_one("div > p")) == str(bs4.select_one("div > p"))
    assert rusty.select_one("missing") is bs4.select_one("missing")
    assert rusty.p.has_attr("class") == bs4.p.has_attr("class")


def test_document_metadata_properties_match_bs4_defaults():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert rusty.name == bs4.name
    assert rusty.parent == bs4.parent
    assert rusty.is_xml == bs4.is_xml
    assert rusty.original_encoding == bs4.original_encoding


def test_document_tag_like_metadata_matches_bs4_defaults():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert rusty.hidden == bs4.hidden
    assert rusty.attrs == bs4.attrs
    assert rusty.can_be_empty_element == bs4.can_be_empty_element
    assert rusty.is_empty_element == bs4.is_empty_element
    assert rusty.known_xml == bs4.known_xml
    assert rusty.namespace == bs4.namespace
    assert rusty.prefix == bs4.prefix
    assert rusty.sourceline == bs4.sourceline
    assert rusty.sourcepos == bs4.sourcepos
    assert rusty.decomposed == bs4.decomposed
    assert rusty.parser_class is BeautifulSoup
    assert rusty.parserClass is BeautifulSoup


def test_tag_metadata_properties_match_bs4_defaults():
    html = "<div><p>x</p><br><input disabled></div>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    for selector in ["div", "p", "br", "input"]:
        rusty_tag = rusty.select_one(selector)
        bs4_tag = bs4.select_one(selector)
        assert rusty_tag.hidden == bs4_tag.hidden
        assert rusty_tag.can_be_empty_element == bs4_tag.can_be_empty_element
        assert rusty_tag.is_empty_element == bs4_tag.is_empty_element


def test_tag_legacy_metadata_aliases_match_bs4():
    html = "<div><p id='x'>one</p><br></div>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert rusty.p.has_key("id") == bs4.p.has_key("id")
    assert rusty.p.has_key("missing") == bs4.p.has_key("missing")
    assert rusty.br.isSelfClosing() == bs4.br.isSelfClosing()
    assert rusty.p.parser_class is BeautifulSoup
    assert rusty.p.parserClass is BeautifulSoup
    assert rusty.p.namespace == bs4.p.namespace
    assert rusty.p.prefix == bs4.p.prefix
    assert rusty.p.decomposed == bs4.p.decomposed
    assert rusty.p.known_xml == bs4.p.known_xml


def test_self_and_traversal_generators_match_bs4():
    html = "<div><p>one</p> text <span>two</span></div>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    for attr in [
        "self_and_descendants",
        "self_and_next_elements",
        "self_and_next_siblings",
        "self_and_parents",
        "self_and_previous_elements",
        "self_and_previous_siblings",
    ]:
        assert shape(getattr(rusty.p, attr)) == shape(getattr(bs4.p, attr))

    assert shape(rusty.p.nextSiblingGenerator()) == shape(bs4.p.nextSiblingGenerator())
    assert shape(rusty.span.previousSiblingGenerator()) == shape(
        bs4.span.previousSiblingGenerator()
    )

    assert shape(rusty.p.string.self_and_next_elements) == shape(
        bs4.p.string.self_and_next_elements
    )
    assert shape(rusty.span.string.self_and_previous_elements) == shape(
        bs4.span.string.self_and_previous_elements
    )


def test_document_tag_like_traversal_matches_bs4():
    rusty = BeautifulSoup("<div><p>one</p></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>one</p></div>", "html.parser")

    assert shape(rusty.childGenerator()) == shape(bs4.childGenerator())
    assert shape(rusty.recursiveChildGenerator()) == shape(bs4.recursiveChildGenerator())
    assert shape(rusty.self_and_descendants) == shape(bs4.self_and_descendants)
    assert shape(rusty.self_and_next_elements) == shape(bs4.self_and_next_elements)
    assert shape(rusty.self_and_previous_elements) == shape(bs4.self_and_previous_elements)
    assert shape(rusty.self_and_next_siblings) == shape(bs4.self_and_next_siblings)
    assert shape(rusty.self_and_previous_siblings) == shape(bs4.self_and_previous_siblings)
    assert shape(rusty.self_and_parents) == shape(bs4.self_and_parents)
    assert rusty.next_element == bs4.next_element
    assert rusty.previous_element == bs4.previous_element
    assert shape(rusty.next_elements) == shape(bs4.next_elements)
    assert shape(rusty.previous_elements) == shape(bs4.previous_elements)
    assert shape(rusty.next_siblings) == shape(bs4.next_siblings)
    assert shape(rusty.previous_siblings) == shape(bs4.previous_siblings)
