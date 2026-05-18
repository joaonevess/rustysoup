from rustysoup import Soup


def test_malformed_html_is_repaired_by_html5_parser():
    soup = Soup("<html><body><p>one<p>two</body></html>")

    assert [tag.text for tag in soup.find_all("p")] == ["one", "two"]


def test_unclosed_table_and_link_remain_queryable():
    soup = Soup("<table><tr><td><a href='/x'>x")

    assert soup.find("a")["href"] == "/x"
    assert soup.select("table a[href]")[0].text == "x"


def test_noscript_contents_parse_like_bs4_html_parser():
    from bs4 import BeautifulSoup as Bs4BeautifulSoup

    html = "<noscript><p>x</p></noscript>"
    assert Soup(html).decode() == Bs4BeautifulSoup(html, "html.parser").decode()
