import pytest
from bs4 import BeautifulSoup as Bs4BeautifulSoup
from bs4.element import ResultSet as Bs4ResultSet

from rustysoup import ResultSet, Soup


HTML = """
<main id="main">
  <div class="product" data-id="1"><a href="/one">one</a></div>
  <div class="product muted" data-id="2"><a>two</a></div>
  <section><a href="/three">three</a></section>
</main>
"""


def test_tag_class_id_and_attribute_selectors():
    soup = Soup(HTML)

    assert [tag.get("data-id") for tag in soup.select("div.product")] == ["1", "2"]
    assert soup.select("#main")[0].name == "main"
    assert [tag.text for tag in soup.select("a[href]")] == ["one", "three"]
    assert soup.select('[data-id="1"]')[0].text == "one"


def test_descendant_child_and_compound_selectors():
    soup = Soup(HTML)

    assert [tag.text for tag in soup.select("main a")] == ["one", "two", "three"]
    assert [tag.text for tag in soup.select("div.product > a[href]")] == ["one"]
    assert soup.select("div.product a[href]")[0]["href"] == "/one"


def test_tag_select_searches_descendants():
    soup = Soup("<div><a href='/x'>x</a></div><a href='/y'>y</a>")
    div = soup.find("div")

    assert [tag["href"] for tag in div.select("a[href]")] == ["/x"]


def test_invalid_selector_raises_value_error():
    soup = Soup("<p>x</p>")

    with pytest.raises(ValueError):
        soup.select("div[")


def test_select_limit_matches_bs4_for_soup_and_tag():
    html = "<main><a href='/1'>one</a><a href='/2'>two</a><a href='/3'>three</a></main>"
    soup = Soup(html)
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert [str(tag) for tag in soup.select("a[href]", limit=2)] == [
        str(tag) for tag in bs4.select("a[href]", limit=2)
    ]
    assert [str(tag) for tag in soup.main.select("a[href]", limit=1)] == [
        str(tag) for tag in bs4.main.select("a[href]", limit=1)
    ]
    assert [str(tag) for tag in soup.select("a[href]", limit=0)] == [
        str(tag) for tag in bs4.select("a[href]", limit=0)
    ]


def test_select_returns_result_set_like_bs4_for_soup_tag_and_css_proxy():
    html = "<main><p class='a'>one</p><p>two</p><span>three</span></main>"
    soup = Soup(html)
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    result_cases = [
        (soup.select("p"), bs4.select("p")),
        (soup.main.select("p"), bs4.main.select("p")),
        (soup.css.select("p"), bs4.css.select("p")),
        (soup.main.css.select("p"), bs4.main.css.select("p")),
        (soup.css.filter("p"), bs4.css.filter("p")),
        (soup.main.css.filter("p"), bs4.main.css.filter("p")),
    ]

    for rusty_result, bs4_result in result_cases:
        assert isinstance(rusty_result, ResultSet)
        assert isinstance(bs4_result, Bs4ResultSet)
        assert isinstance(rusty_result, list)
        assert rusty_result.source is bs4_result.source
        assert [str(tag) for tag in rusty_result] == [str(tag) for tag in bs4_result]


def test_select_one_accepts_bs4_style_optional_arguments():
    html = "<main><a href='/1'>one</a><a href='/2'>two</a></main>"
    soup = Soup(html)
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert str(soup.select_one("a[href]", limit=1)) == str(bs4.select_one("a[href]", limit=1))
    assert str(soup.main.select_one("a[href]", namespaces=None)) == str(
        bs4.main.select_one("a[href]", namespaces=None)
    )


def test_css_proxy_select_methods_match_bs4():
    html = "<main><p class='a'>one</p><p>two</p><span>three</span></main>"
    soup = Soup(html)
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert [str(tag) for tag in soup.css.select("p")] == [
        str(tag) for tag in bs4.css.select("p")
    ]
    assert [str(tag) for tag in soup.main.css.select("p", limit=1)] == [
        str(tag) for tag in bs4.main.css.select("p", limit=1)
    ]
    assert str(soup.css.select_one("p.a")) == str(bs4.css.select_one("p.a"))
    assert [str(tag) for tag in soup.main.css.iselect("p")] == [
        str(tag) for tag in bs4.main.css.iselect("p")
    ]


def test_css_proxy_match_filter_closest_and_escape_match_bs4():
    html = "<main><p class='a'>one</p><p>two</p><span>three</span></main>"
    soup = Soup(html)
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert [str(tag) for tag in soup.css.filter("p")] == [
        str(tag) for tag in bs4.css.filter("p")
    ]
    assert [str(tag) for tag in soup.main.css.filter("p")] == [
        str(tag) for tag in bs4.main.css.filter("p")
    ]
    assert soup.p.css.match("p.a") == bs4.p.css.match("p.a")
    assert soup.main.css.match("p.a") == bs4.main.css.match("p.a")
    assert str(soup.p.css.closest("main")) == str(bs4.p.css.closest("main"))
    assert soup.css.closest("main") == bs4.css.closest("main")
    assert soup.css.escape("a.b#c") == bs4.css.escape("a.b#c")


def test_css_proxy_invalid_selector_raises_value_error():
    soup = Soup("<main><p>one</p></main>")

    with pytest.raises(ValueError):
        soup.p.css.match("p[")


def test_is_where_and_matches_functional_selectors_match_bs4():
    html = """
    <main>
      <section class="primary"><p class="a">one</p><a href="/one">link</a></section>
      <section class="secondary"><p class="b">two</p><span>span</span></section>
      <article><p class="a">three</p></article>
    </main>
    """
    soup = Soup(html)
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    selectors = [
        "main > :is(section, article)",
        ":is(section.primary, article) p.a",
        "p:is(.a, .b)",
        "section:where(.primary, .missing) a[href]",
        "main > :matches(section.secondary, article)",
    ]
    for selector in selectors:
        assert [str(tag) for tag in soup.select(selector)] == [
            str(tag) for tag in bs4.select(selector)
        ]

    assert str(soup.select_one(":is(section.secondary, article) p")) == str(
        bs4.select_one(":is(section.secondary, article) p")
    )
    assert soup.article.css.match(":is(article, section)") == bs4.article.css.match(
        ":is(article, section)"
    )
