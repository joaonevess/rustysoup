import pytest
from bs4 import BeautifulSoup as Bs4BeautifulSoup

from rustysoup import BeautifulSoup, FeatureNotFound, Soup


HTML = """
<!doctype html>
<html>
  <head>
    <title>Catalog</title>
    <meta name="description" content="A catalog page">
  </head>
  <body>
    <main id="main">
      <article class="product featured" data-id="1">
        <h2><a href="/products/1">Product One</a></h2>
        <p> Alpha <b>Beta</b> </p>
        <span class="price">$10</span>
      </article>
      <article class="product" data-id="2">
        <h2><a href="/products/2">Product Two</a></h2>
        <p>Gamma</p>
        <span class="price">$20</span>
      </article>
      <form id="signup">
        <input name="email" disabled>
        <input name="ok" type="checkbox" checked>
      </form>
      <nav><a href="#top">Top</a></nav>
    </main>
  </body>
</html>
"""


def texts(nodes):
    return [node.get_text(" ", strip=True) for node in nodes]


def names(nodes):
    return [node.name for node in nodes]


def attrs_dict(tag):
    return {key: list(value) if isinstance(value, list) else value for key, value in tag.attrs.items()}


def test_parser_names_common_migration_paths():
    for features in (None, "html.parser", "html", ["html.parser"], ("html.parser",), "lxml", "lxml-html"):
        if features is None:
            soup = BeautifulSoup("<p>x</p>")
        else:
            soup = BeautifulSoup("<p>x</p>", features)
        assert soup.find("p").text == "x"

    with pytest.raises(FeatureNotFound):
        BeautifulSoup("<p>x</p>", "html5lib")

    with pytest.raises(FeatureNotFound):
        BeautifulSoup("<p>x</p>", "no-such-parser")


def test_find_and_find_all_common_migration_patterns_match_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert rusty.find("title").text == bs4.find("title").text == "Catalog"
    assert rusty.title.text == bs4.title.text == "Catalog"
    assert rusty.find("article", class_="featured")["data-id"] == bs4.find("article", class_="featured")["data-id"]
    assert rusty.find("a", href=True)["href"] == bs4.find("a", href=True)["href"]
    assert rusty.find("missing") is None

    assert texts(rusty.find_all("article")) == texts(bs4.find_all("article"))
    assert [tag["href"] for tag in rusty.find_all("a", href=True)] == [
        tag["href"] for tag in bs4.find_all("a", href=True)
    ]
    assert names(rusty.find_all(["article", "form"], limit=3)) == names(
        bs4.find_all(["article", "form"], limit=3)
    )
    assert [tag["data-id"] for tag in rusty.main.find_all("article", recursive=False)] == [
        tag["data-id"] for tag in bs4.main.find_all("article", recursive=False)
    ]


def test_select_common_migration_patterns_match_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    selectors = [
        "main",
        "#main",
        "article.product",
        "article.featured a[href]",
        'article[data-id="2"] .price',
        "main > article",
        "form input",
        "[disabled]",
    ]
    for selector in selectors:
        assert [str(tag) for tag in rusty.select(selector)] == [
            str(tag) for tag in bs4.select(selector)
        ]

    assert [tag.text for tag in rusty.select("article.product a[href]")] == [
        "Product One",
        "Product Two",
    ]
    with pytest.raises(ValueError):
        rusty.select("article > > a")


def test_attrs_text_and_truthiness_common_patterns_match_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    rusty_article = rusty.find("article", class_="featured")
    bs4_article = bs4.find("article", class_="featured")
    assert attrs_dict(rusty_article) == attrs_dict(bs4_article)
    assert rusty_article.get("missing", "fallback") == bs4_article.get("missing", "fallback")

    rusty_input = rusty.find("input", disabled=True)
    bs4_input = bs4.find("input", disabled=True)
    assert bool(rusty_input) == bool(bs4_input) is True
    assert rusty_input["disabled"] == bs4_input["disabled"] == ""
    assert rusty_input.get("name") == bs4_input.get("name") == "email"

    with pytest.raises(KeyError):
        rusty_article["missing"]

    assert rusty_article.text == bs4_article.text
    assert rusty_article.get_text("|", strip=True) == bs4_article.get_text("|", strip=True)
    assert rusty.get_text(" ", strip=True) == bs4.get_text(" ", strip=True)


def test_malformed_html_stays_queryable_for_common_bs4_workflows():
    html = "<html><body><p>one<p>two<table><tr><td><a href=/x>x<form><input disabled>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert len(rusty.find_all("p")) == len(bs4.find_all("p")) == 2
    assert rusty.find_all("p")[0].get_text(" ", strip=True).startswith("one")
    assert rusty.find_all("p")[1].get_text(" ", strip=True).startswith("two")
    assert rusty.find("a")["href"] == bs4.find("a")["href"] == "/x"
    assert rusty.select("table a[href]")[0].text == bs4.select("table a[href]")[0].text
    assert bool(rusty.find("input", disabled=True)) is True


def test_traversal_common_migration_patterns_match_bs4():
    html = "<div><p id='one'><span>One</span></p><p id='two'>Two</p><a href='/last'>Last</a></div>"
    rusty = Soup(html)
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert rusty.span.parent["id"] == bs4.span.parent["id"] == "one"
    assert names(child for child in rusty.div.children if getattr(child, "name", None)) == names(
        child for child in bs4.div.children if getattr(child, "name", None)
    )
    assert names(node for node in rusty.div.descendants if getattr(node, "name", None)) == names(
        node for node in bs4.div.descendants if getattr(node, "name", None)
    )
    assert rusty.span.find_next("p")["id"] == bs4.span.find_next("p")["id"] == "two"
    assert rusty.a.find_previous("p")["id"] == bs4.a.find_previous("p")["id"] == "two"
    assert rusty.p.next_sibling["id"] == bs4.p.next_sibling["id"] == "two"
    assert rusty.find("p", id="two").previous_sibling["id"] == bs4.find("p", id="two").previous_sibling["id"] == "one"
