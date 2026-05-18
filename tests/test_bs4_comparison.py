import pytest

from rustysoup import Soup

bs4 = pytest.importorskip("bs4")


def test_basic_find_text_and_select_match_bs4():
    html = """
    <html><head><title>Hello</title></head>
    <body><div class="product"><a href="/x">Product X</a></div></body></html>
    """

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert rusty.find("title").text == beautiful.find("title").text
    assert rusty.find("div", class_="product").text.strip() == beautiful.find(
        "div", class_="product"
    ).text.strip()
    assert [tag["href"] for tag in rusty.select("div.product a[href]")] == [
        tag["href"] for tag in beautiful.select("div.product a[href]")
    ]


def test_explicit_full_document_with_doctype_matches_bs4():
    html = """<!doctype html>
<html><head><title>Hello</title><meta name="description" content="Synthetic page"></head><body><main id="main" class="catalog"><div class="product featured" data-id="1"><a href="/products/1?ref=bench">Product 1</a><span class="price">$1.99</span></div></main></body></html>"""

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert rusty.get_text("|", strip=True) == beautiful.get_text("|", strip=True)
    assert [tag["href"] for tag in rusty.select("div.product a[href]")] == [
        tag["href"] for tag in beautiful.select("div.product a[href]")
    ]


def test_explicit_full_document_without_doctype_matches_bs4():
    html = """<html><head><title>No doctype</title></head><body><table><tr><td><a href="/x?one=1&two=2">Link</a></td></tr></table></body></html>"""

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert rusty.get_text("|", strip=True) == beautiful.get_text("|", strip=True)
    assert rusty.find("a").get("href") == beautiful.find("a").get("href")


def test_explicit_full_document_with_common_entities_matches_bs4():
    html = """<!doctype html>
<html><head><title>A &amp; B</title><meta name="description" content="Tom &amp; Jerry &#x27;fast&#x27;"></head><body><main><p>One &amp; Two &mdash; Three&nbsp;Four</p><span>Icon &gt; Shape</span></main></body></html>"""

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert rusty.get_text("|", strip=True) == beautiful.get_text("|", strip=True)
    assert rusty.find("meta").get("content") == beautiful.find("meta").get("content")


def test_explicit_full_document_with_literal_ampersands_matches_bs4():
    html = """<!doctype html>
<html><head><title>AT&T Research</title></head><body><a href="/search?q=rust&sort=fast">R&D links</a><p>Terms & trademarks</p></body></html>"""

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert rusty.get_text("|", strip=True) == beautiful.get_text("|", strip=True)
    assert rusty.find("a").get("href") == beautiful.find("a").get("href")


def test_explicit_full_document_with_raw_text_elements_matches_bs4():
    html = """<!doctype html>
<html><head><script>if (a < b) { x(); }</script><style>a < b { color: red; }</style></head><body><p>After</p></body></html>"""

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert rusty.get_text("|", strip=True) == beautiful.get_text("|", strip=True)
    assert rusty.script.string == beautiful.script.string
    assert rusty.style.string == beautiful.style.string


def test_explicit_full_document_with_pre_leading_newline_matches_bs4():
    html = "<!doctype html><html><head><title>Pre</title></head><body><pre>\n  line\n</pre><p>After</p></body></html>"

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert rusty.pre.string == beautiful.pre.string
    assert rusty.get_text("|", strip=False) == beautiful.get_text("|", strip=False)


def test_explicit_full_document_with_textarea_raw_text_matches_bs4():
    html = "<!doctype html><html><head><title>Textarea</title></head><body><textarea>hello < world</textarea><p>After</p></body></html>"

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert rusty.textarea.string == beautiful.textarea.string
    assert rusty.get_text("|", strip=True) == beautiful.get_text("|", strip=True)


def test_explicit_full_document_with_gt_inside_quoted_attribute_matches_bs4():
    html = """<!doctype html>
<html><head><title>Quoted attr</title></head><body><input data-action="keydown@window->search-focus#focus"><p>After</p></body></html>"""

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert rusty.find("input").get("data-action") == beautiful.find("input").get("data-action")
    assert rusty.get_text("|", strip=True) == beautiful.get_text("|", strip=True)


def test_explicit_full_document_with_malformed_attrs_matches_bs4():
    html = '<!doctype html><html><body><img src="x" width="16" , height="16" /></body></html>'

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert rusty.find("img").attrs == beautiful.find("img").attrs


def test_explicit_full_document_with_framework_attrs_matches_bs4():
    html = '<!doctype html><html><body><section #="" @click="go" [value]="item" data-x="1"></section></body></html>'

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert rusty.find("section").attrs == beautiful.find("section").attrs


def test_explicit_full_document_with_processing_instruction_matches_bs4_shape():
    html = '<!doctype html><html><body><svg><?xml version="1.0" encoding="UTF-8"?><path></path></svg></body></html>'

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)


def test_explicit_full_document_with_more_named_entities_matches_bs4():
    html = '<!doctype html><html><body><a href="#x">&para;</a><p>v1 &middot; docs &raquo; item &bull; next</p></body></html>'

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert rusty.get_text("|", strip=True) == beautiful.get_text("|", strip=True)


def test_explicit_full_document_with_forgiving_end_tags_matches_bs4():
    html = '<!doctype html><html><body><div><ul><li><div></div></li><li></li><li><a>scores</a></ul></div><p><a href="/x"></a>full press release</a></p></body></html>'

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert rusty.get_text("|", strip=True) == beautiful.get_text("|", strip=True)


def test_clean_fragment_matches_bs4():
    html = '<div class="product" data-id="1"><a href="/x">Product X</a><span class="price">$10</span></div><p>tail</p>'

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert rusty.get_text("|", strip=True) == beautiful.get_text("|", strip=True)
    assert [tag["href"] for tag in rusty.select("div.product a[href]")] == [
        tag["href"] for tag in beautiful.select("div.product a[href]")
    ]


def test_self_closing_non_void_fragment_matches_bs4():
    html = "<div/>x<p/>y<br/>z"

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert rusty.get_text("|", strip=False) == beautiful.get_text("|", strip=False)


def test_missing_find_matches_bs4_shape():
    html = "<main><p>x</p></main>"

    assert Soup(html).find("article") is None
    assert bs4.BeautifulSoup(html, "html.parser").find("article") is None


def test_template_contents_are_preserved_like_bs4_html_parser():
    html = '<div><template><p>x</p><span data-id="1">y</span></template><p>z</p></div>'

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert [str(child) for child in rusty.template.contents] == [
        str(child) for child in beautiful.template.contents
    ]
    assert [str(tag) for tag in rusty.select("template p")] == [
        str(tag) for tag in beautiful.select("template p")
    ]
    assert rusty.get_text("|", strip=True) == beautiful.get_text("|", strip=True)
    assert rusty.template.get_text("|", strip=True) == beautiful.template.get_text(
        "|", strip=True
    )
    assert rusty.template.p.get_text("|", strip=True) == beautiful.template.p.get_text(
        "|", strip=True
    )


def test_template_contents_in_full_documents_match_bs4_html_parser():
    html = "<!doctype html><html><body><template><p>x</p></template><p>z</p></body></html>"

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert [str(tag) for tag in rusty.select("template p")] == [
        str(tag) for tag in beautiful.select("template p")
    ]
    assert rusty.get_text("|", strip=True) == beautiful.get_text("|", strip=True)


def test_template_strings_match_bs4_text_visibility_rules():
    html = "<template>loose<p>x</p><script>hidden()</script></template><p>z</p>"

    rusty = Soup(html)
    beautiful = bs4.BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(beautiful)
    assert rusty.get_text("|", strip=True) == beautiful.get_text("|", strip=True)
    assert rusty.template.text == beautiful.template.text
    assert rusty.template.p.text == beautiful.template.p.text
    assert [(str(value), type(value).__name__) for value in rusty.template.strings] == [
        (str(value), type(value).__name__) for value in beautiful.template.strings
    ]
    assert [(str(value), type(value).__name__) for value in rusty.template.p.strings] == [
        (str(value), type(value).__name__) for value in beautiful.template.p.strings
    ]
    assert type(rusty.template.p.string).__name__ == type(beautiful.template.p.string).__name__
