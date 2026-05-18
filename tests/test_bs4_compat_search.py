import re
from io import BytesIO, StringIO

from bs4 import BeautifulSoup as Bs4BeautifulSoup
from bs4 import FeatureNotFound as Bs4FeatureNotFound
from bs4 import SoupStrainer as Bs4SoupStrainer
from bs4.element import ResultSet as Bs4ResultSet
import pytest

from rustysoup import BeautifulSoup, FeatureNotFound, ResultSet, SoupStrainer


HTML = """
<main>
  <section id="catalog">
    <article class="product featured" data-id="1"><a href="/one">One</a><p>Alpha</p></article>
    <article class="product" data-id="2"><a>Two</a><p>Beta</p></article>
    <aside data-id="side"><a href="/side">Side</a></aside>
  </section>
</main>
"""


def names(nodes):
    return [getattr(node, "name", None) for node in nodes]


def texts(nodes):
    return [str(node) for node in nodes]


def test_constructor_accepts_bs4_style_features_and_bytes():
    assert BeautifulSoup(HTML, "html.parser").find("main").name == "main"
    assert BeautifulSoup(HTML.encode(), "html.parser").find("article")["data-id"] == "1"
    assert BeautifulSoup(markup=HTML, features="html.parser").find("aside").text == "Side"


def test_constructor_decodes_bytes_from_encoding_like_bs4():
    markup = b"<p>caf\xe9</p>"
    rusty = BeautifulSoup(markup, "html.parser", from_encoding="latin-1")
    bs4 = Bs4BeautifulSoup(markup, "html.parser", from_encoding="latin-1")

    assert str(rusty) == str(bs4)
    assert rusty.get_text() == bs4.get_text()
    assert rusty.original_encoding == bs4.original_encoding
    assert rusty.declared_html_encoding == bs4.declared_html_encoding
    assert rusty.contains_replacement_characters == bs4.contains_replacement_characters


def test_constructor_accepts_file_like_markup_like_bs4():
    cases = [
        lambda: (StringIO("<p>x</p>"), StringIO("<p>x</p>")),
        lambda: (BytesIO("<p>é</p>".encode()), BytesIO("<p>é</p>".encode())),
        lambda: (BytesIO(b"<p>caf\xe9</p>"), BytesIO(b"<p>caf\xe9</p>")),
    ]

    for make_pair in cases:
        rusty_markup, bs4_markup = make_pair()
        rusty = BeautifulSoup(rusty_markup, "html.parser")
        bs4 = Bs4BeautifulSoup(bs4_markup, "html.parser")

        assert str(rusty) == str(bs4)
        assert rusty.get_text() == bs4.get_text()
        assert rusty.original_encoding == bs4.original_encoding


def test_constructor_rejects_invalid_markup_types_like_bs4():
    for markup in (["<p>x</p>"], object()):
        with pytest.raises(TypeError):
            Bs4BeautifulSoup(markup, "html.parser")
        with pytest.raises(TypeError):
            BeautifulSoup(markup, "html.parser")


def test_constructor_distinguishes_omitted_markup_from_explicit_none_like_bs4():
    assert str(BeautifulSoup()) == str(Bs4BeautifulSoup())
    assert str(BeautifulSoup(features="html.parser")) == str(
        Bs4BeautifulSoup(features="html.parser")
    )

    for kwargs in ({}, {"features": "html.parser"}):
        with pytest.raises(TypeError) as bs4_error:
            Bs4BeautifulSoup(None, **kwargs)
        with pytest.raises(TypeError) as rusty_error:
            BeautifulSoup(None, **kwargs)
        assert str(rusty_error.value) == str(bs4_error.value)

    with pytest.raises(TypeError) as bs4_error:
        Bs4BeautifulSoup(markup=None, features="html.parser")
    with pytest.raises(TypeError) as rusty_error:
        BeautifulSoup(markup=None, features="html.parser")
    assert str(rusty_error.value) == str(bs4_error.value)


def test_constructor_detects_common_byte_encodings_like_bs4():
    cases = [
        b"<p>caf\xc3\xa9</p>",
        b"<p>caf\xe9</p>",
        b'<meta charset="windows-1252"><p>\x93x\x94</p>',
        (
            b'<meta http-equiv="Content-Type" '
            b'content="text/html; charset=windows-1252"><p>\x93x\x94</p>'
        ),
    ]

    for markup in cases:
        rusty = BeautifulSoup(markup, "html.parser")
        bs4 = Bs4BeautifulSoup(markup, "html.parser")

        assert rusty.get_text() == bs4.get_text()
        assert rusty.original_encoding == bs4.original_encoding
        assert rusty.declared_html_encoding == bs4.declared_html_encoding
        assert rusty.contains_replacement_characters == bs4.contains_replacement_characters


def test_constructor_meta_charset_serialization_substitution_matches_bs4():
    cases = [
        b'<meta charset="windows-1252"><p>\x93x\x94</p>',
        (
            b'<meta http-equiv="Content-Type" '
            b'content="text/html; charset=windows-1252"><p>\x93x\x94</p>'
        ),
    ]

    for markup in cases:
        rusty = BeautifulSoup(markup, "html.parser")
        bs4 = Bs4BeautifulSoup(markup, "html.parser")

        assert str(rusty) == str(bs4)
        assert str(rusty.find("meta")) == str(bs4.find("meta"))
        assert rusty.find("meta").attrs == bs4.find("meta").attrs
        assert rusty.decode(eventual_encoding="latin-1") == bs4.decode(
            eventual_encoding="latin-1"
        )
        assert rusty.encode("latin-1") == bs4.encode("latin-1")


def test_constructor_exclude_encodings_replacement_metadata_matches_bs4():
    markup = b"<p>caf\xe9</p>"
    kwargs = {
        "features": "html.parser",
        "exclude_encodings": ["iso-8859-1", "latin-1", "windows-1252"],
    }

    rusty = BeautifulSoup(markup, **kwargs)
    bs4 = Bs4BeautifulSoup(markup, **kwargs)

    assert str(rusty) == str(bs4)
    assert rusty.original_encoding == bs4.original_encoding
    assert rusty.contains_replacement_characters == bs4.contains_replacement_characters


def test_constructor_accepts_html_parser_feature_sequences_like_bs4():
    for features in (["html.parser"], ("html.parser",)):
        rusty = BeautifulSoup("<p>x</p>", features)
        bs4 = Bs4BeautifulSoup("<p>x</p>", features)
        assert str(rusty) == str(bs4)


def test_constructor_lxml_html_features_use_full_document_tree_like_bs4():
    for features in ("lxml", "lxml-html"):
        try:
            bs4 = Bs4BeautifulSoup("<p>x</p>", features)
        except Bs4FeatureNotFound:
            pytest.skip(f"bs4 parser {features!r} is not available")

        rusty = BeautifulSoup("<p>x</p>", features)

        assert str(rusty) == str(bs4)
        assert [getattr(node, "name", None) for node in rusty.contents] == [
            getattr(node, "name", None) for node in bs4.contents
        ]
        assert rusty.html.name == bs4.html.name
        assert rusty.body.name == bs4.body.name
        assert rusty.p.text == bs4.p.text


def test_constructor_unsupported_features_raise_feature_not_found_like_bs4():
    for features in (
        "html5lib",
        "no-such-parser",
        ["no-such-parser"],
        ("html5lib", "no-such-parser"),
    ):
        with pytest.raises(Bs4FeatureNotFound) as bs4_error:
            Bs4BeautifulSoup("<p>x</p>", features)
        with pytest.raises(FeatureNotFound) as rusty_error:
            BeautifulSoup("<p>x</p>", features)
        assert str(rusty_error.value) == str(bs4_error.value)


def test_constructor_parse_only_soup_strainer_matches_bs4():
    html = """
    <html><body>
      <a href="/x">x</a>
      <a>y</a>
      <p class="keep">p <b>b</b></p>
      <p>z</p>
    </body></html>
    """
    cases = [
        (SoupStrainer("a"), Bs4SoupStrainer("a")),
        (SoupStrainer("a", href=True), Bs4SoupStrainer("a", href=True)),
        (SoupStrainer("p", class_="keep"), Bs4SoupStrainer("p", class_="keep")),
        (SoupStrainer(string="z"), Bs4SoupStrainer(string="z")),
        (SoupStrainer("p", string="z"), Bs4SoupStrainer("p", string="z")),
    ]

    for rusty_strainer, bs4_strainer in cases:
        rusty = BeautifulSoup(html, "html.parser", parse_only=rusty_strainer)
        bs4 = Bs4BeautifulSoup(html, "html.parser", parse_only=bs4_strainer)

        assert str(rusty) == str(bs4)
        assert [str(tag) for tag in rusty.find_all(True)] == [
            str(tag) for tag in bs4.find_all(True)
        ]
        assert [str(node) for node in rusty.find_all(string=True)] == [
            str(node) for node in bs4.find_all(string=True)
        ]


def test_constructor_parse_only_soup_strainer_list_and_regex_match_bs4():
    html = """
    <html><body>
      <article data-id="abc">A</article>
      <aside data-id="side">S</aside>
      <p class="keep">Alpha</p>
      <p>Beta</p>
    </body></html>
    """
    cases = [
        (SoupStrainer(["article", "aside"]), Bs4SoupStrainer(["article", "aside"])),
        (SoupStrainer(re.compile("^a")), Bs4SoupStrainer(re.compile("^a"))),
        (
            SoupStrainer("article", attrs={"data-id": re.compile("^a")}),
            Bs4SoupStrainer("article", attrs={"data-id": re.compile("^a")}),
        ),
        (
            SoupStrainer("p", class_=["keep", "missing"]),
            Bs4SoupStrainer("p", class_=["keep", "missing"]),
        ),
        (
            SoupStrainer(string=re.compile("Alpha")),
            Bs4SoupStrainer(string=re.compile("Alpha")),
        ),
        (
            SoupStrainer(string=["Alpha", "Missing"]),
            Bs4SoupStrainer(string=["Alpha", "Missing"]),
        ),
    ]

    for rusty_strainer, bs4_strainer in cases:
        rusty = BeautifulSoup(html, "html.parser", parse_only=rusty_strainer)
        bs4 = Bs4BeautifulSoup(html, "html.parser", parse_only=bs4_strainer)

        assert str(rusty) == str(bs4)
        assert [str(tag) for tag in rusty.find_all(True)] == [
            str(tag) for tag in bs4.find_all(True)
        ]
        assert [str(node) for node in rusty.find_all(string=True)] == [
            str(node) for node in bs4.find_all(string=True)
        ]


def test_constructor_parse_only_soup_strainer_callables_match_bs4():
    html = """
    <html><body>
      <article data-id="abc">A</article>
      <aside data-id="side">S</aside>
      <p class="keep">Alpha</p>
      <p>Beta</p>
    </body></html>
    """
    cases = [
        (
            SoupStrainer(lambda name: name and name.startswith("a")),
            Bs4SoupStrainer(lambda name: name and name.startswith("a")),
        ),
        (
            SoupStrainer(
                "article",
                attrs={"data-id": lambda value: value and value.startswith("a")},
            ),
            Bs4SoupStrainer(
                "article",
                attrs={"data-id": lambda value: value and value.startswith("a")},
            ),
        ),
        (
            SoupStrainer(string=lambda value: value == "Alpha"),
            Bs4SoupStrainer(string=lambda value: value == "Alpha"),
        ),
    ]

    for rusty_strainer, bs4_strainer in cases:
        rusty = BeautifulSoup(html, "html.parser", parse_only=rusty_strainer)
        bs4 = Bs4BeautifulSoup(html, "html.parser", parse_only=bs4_strainer)

        assert str(rusty) == str(bs4)
        assert [str(tag) for tag in rusty.find_all(True)] == [
            str(tag) for tag in bs4.find_all(True)
        ]
        assert [str(node) for node in rusty.find_all(string=True)] == [
            str(node) for node in bs4.find_all(string=True)
        ]


def test_soup_strainer_public_match_methods_match_bs4():
    html = '<div><a href="/x">x</a><a>y</a><p class="keep">p</p>text</div>'
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    cases = [
        (SoupStrainer("a"), Bs4SoupStrainer("a")),
        (SoupStrainer("a", href=True), Bs4SoupStrainer("a", href=True)),
        (SoupStrainer("p", class_="keep"), Bs4SoupStrainer("p", class_="keep")),
        (SoupStrainer(string="x"), Bs4SoupStrainer(string="x")),
        (SoupStrainer(True), Bs4SoupStrainer(True)),
        (SoupStrainer(), Bs4SoupStrainer()),
    ]
    node_pairs = [
        (rusty.div, bs4.div),
        (rusty.find("a"), bs4.find("a")),
        (rusty.find_all("a")[1], bs4.find_all("a")[1]),
        (rusty.p, bs4.p),
        (rusty.a.string, bs4.a.string),
        (rusty.div.contents[-1], bs4.div.contents[-1]),
    ]
    tag_pairs = node_pairs[:4]
    tag_creation_args = [
        (None, "a", {}),
        (None, "a", {"href": "/x"}),
        (None, "p", {"class": "keep"}),
        (None, "p", {}),
    ]

    for rusty_strainer, bs4_strainer in cases:
        assert rusty_strainer.includes_everything == bs4_strainer.includes_everything
        assert rusty_strainer.excludes_everything == bs4_strainer.excludes_everything

        for rusty_node, bs4_node in node_pairs:
            assert rusty_strainer.match(rusty_node) == bs4_strainer.match(bs4_node)
        for rusty_tag, bs4_tag in tag_pairs:
            assert rusty_strainer.matches_tag(rusty_tag) == bs4_strainer.matches_tag(bs4_tag)
        for args in tag_creation_args:
            assert rusty_strainer.allow_tag_creation(*args) == bs4_strainer.allow_tag_creation(
                *args
            )
        for value in ("x", "p", "text"):
            assert rusty_strainer.allow_string_creation(
                value
            ) == bs4_strainer.allow_string_creation(value)


def test_find_all_name_filters_match_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert names(rusty.find_all(["article", "aside"])) == names(bs4.find_all(["article", "aside"]))
    assert names(rusty.find_all(True, limit=3)) == names(bs4.find_all(True, limit=3))
    assert names(rusty.section.find_all("article", recursive=False)) == names(
        bs4.section.find_all("article", recursive=False)
    )
    assert names(rusty.find_all(re.compile("^a"))) == names(bs4.find_all(re.compile("^a")))


def test_find_all_attr_filters_match_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert texts(rusty.find_all("article", class_="product")) == texts(
        bs4.find_all("article", class_="product")
    )
    assert texts(rusty.find_all("article", class_="product featured")) == texts(
        bs4.find_all("article", class_="product featured")
    )
    assert texts(rusty.find_all("a", href=True)) == texts(bs4.find_all("a", href=True))
    assert texts(rusty.find_all("a", href=False)) == texts(bs4.find_all("a", href=False))
    assert texts(rusty.find_all(attrs={"data-id": re.compile(r"^(1|side)$")})) == texts(
        bs4.find_all(attrs={"data-id": re.compile(r"^(1|side)$")})
    )
    assert texts(rusty.find_all("article", attrs={"data-id": ["1", "missing"]})) == texts(
        bs4.find_all("article", attrs={"data-id": ["1", "missing"]})
    )


def test_underscore_attribute_filter_names_match_bs4():
    html = '<div class="real" class_="literal" data-id="hyphen" data_id="underscore">x</div>'
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert texts(rusty.find_all(data_id="underscore")) == texts(
        bs4.find_all(data_id="underscore")
    )
    assert texts(rusty.find_all(data_id="hyphen")) == texts(bs4.find_all(data_id="hyphen"))
    assert texts(rusty.find_all(attrs={"data_id": "underscore"})) == texts(
        bs4.find_all(attrs={"data_id": "underscore"})
    )
    assert texts(rusty.find_all(attrs={"data_id": "hyphen"})) == texts(
        bs4.find_all(attrs={"data_id": "hyphen"})
    )
    assert texts(rusty.find_all(attrs={"class_": "literal"})) == texts(
        bs4.find_all(attrs={"class_": "literal"})
    )
    assert texts(rusty.find_all(class_="real")) == texts(bs4.find_all(class_="real"))


def test_callable_filters_match_bs4_for_common_cases():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    def tag_name_has_t(tag):
        return tag.name.startswith("a")

    def href_is_internal(value):
        return value is not None and value.startswith("/")

    assert names(rusty.find_all(tag_name_has_t)) == names(bs4.find_all(tag_name_has_t))
    assert texts(rusty.find_all("a", href=href_is_internal)) == texts(
        bs4.find_all("a", href=href_is_internal)
    )


def test_string_filter_and_limit_match_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert texts(rusty.find_all(string="One")) == texts(bs4.find_all(string="One"))
    assert texts(rusty.find_all("p", string=re.compile("Alpha|Beta"), limit=1)) == texts(
        bs4.find_all("p", string=re.compile("Alpha|Beta"), limit=1)
    )
    assert str(rusty.find(string="Beta")) == str(bs4.find(string="Beta"))


def test_call_shortcut_matches_bs4_find_all():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert texts(rusty("a", href=True)) == texts(bs4("a", href=True))
    assert names(rusty(["article", "aside"], limit=2)) == names(
        bs4(["article", "aside"], limit=2)
    )
    assert texts(rusty.section("article", class_="product", recursive=False)) == texts(
        bs4.section("article", class_="product", recursive=False)
    )


def test_find_all_variants_return_result_set_like_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    result_cases = [
        (rusty.find_all("article", class_="product"), bs4.find_all("article", class_="product")),
        (rusty("a", href=True), bs4("a", href=True)),
        (rusty.article.find_all_next("a"), bs4.article.find_all_next("a")),
        (rusty.aside.find_all_previous(True, limit=2), bs4.aside.find_all_previous(True, limit=2)),
        (rusty.a.find_parents(True), bs4.a.find_parents(True)),
        (rusty.article.find_next_siblings(True), bs4.article.find_next_siblings(True)),
        (rusty.aside.find_previous_siblings(True), bs4.aside.find_previous_siblings(True)),
        (rusty.a.string.find_all_next(string=True), bs4.a.string.find_all_next(string=True)),
    ]

    for rusty_result, bs4_result in result_cases:
        assert isinstance(rusty_result, ResultSet)
        assert isinstance(bs4_result, Bs4ResultSet)
        assert isinstance(rusty_result, list)
        assert texts(rusty_result) == texts(bs4_result)
        assert rusty_result.source is not None

    source = rusty.find_all("article", class_="product").source
    assert isinstance(source, SoupStrainer)
    assert source.name == "article"
    assert source.attrs == {"class": "product"}


def test_result_set_missing_attribute_error_matches_bs4_guidance():
    rusty_result = BeautifulSoup("<p>x</p>", "html.parser").find_all("p")
    bs4_result = Bs4BeautifulSoup("<p>x</p>", "html.parser").find_all("p")

    with pytest.raises(AttributeError) as rusty_error:
        rusty_result.foo
    with pytest.raises(AttributeError) as bs4_error:
        bs4_result.foo

    assert str(rusty_error.value) == str(bs4_error.value)


def test_find_all_legacy_alias_matches_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert texts(rusty.findAll("a", href=True)) == texts(bs4.findAll("a", href=True))
    assert texts(rusty.section.findAll("article", recursive=False)) == texts(
        bs4.section.findAll("article", recursive=False)
    )


def test_find_child_legacy_aliases_match_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert str(rusty.findChild("article")) == str(bs4.findChild("article"))
    assert texts(rusty.findChildren("article", class_="product")) == texts(
        bs4.findChildren("article", class_="product")
    )
    assert str(rusty.section.findChild("article", recursive=False)) == str(
        bs4.section.findChild("article", recursive=False)
    )
    assert texts(rusty.section.findChildren("article", recursive=False)) == texts(
        bs4.section.findChildren("article", recursive=False)
    )


def test_document_page_element_search_methods_match_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert rusty.find_next("article") == bs4.find_next("article")
    assert texts(rusty.find_all_next("article")) == texts(bs4.find_all_next("article"))
    assert rusty.find_previous("article") == bs4.find_previous("article")
    assert texts(rusty.find_all_previous("article")) == texts(
        bs4.find_all_previous("article")
    )
    assert rusty.find_parent(True) == bs4.find_parent(True)
    assert texts(rusty.find_parents(True)) == texts(bs4.find_parents(True))
    assert rusty.find_next_sibling(True) == bs4.find_next_sibling(True)
    assert texts(rusty.find_next_siblings(True)) == texts(bs4.find_next_siblings(True))
    assert rusty.find_previous_sibling(True) == bs4.find_previous_sibling(True)
    assert texts(rusty.find_previous_siblings(True)) == texts(
        bs4.find_previous_siblings(True)
    )


def test_legacy_fetch_aliases_match_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert texts(rusty.article.fetchNextSiblings(True)) == texts(
        bs4.article.fetchNextSiblings(True)
    )
    assert texts(rusty.aside.fetchPreviousSiblings(True)) == texts(
        bs4.aside.fetchPreviousSiblings(True)
    )
    assert names(rusty.a.fetchParents(True)) == names(bs4.a.fetchParents(True))
    assert names(rusty.a.fetchAllPrevious(True, limit=2)) == names(
        bs4.a.fetchAllPrevious(True, limit=2)
    )
    assert texts(rusty.fetchAllPrevious(True)) == texts(bs4.fetchAllPrevious(True))
