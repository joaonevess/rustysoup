import pytest
from bs4 import BeautifulSoup as Bs4BeautifulSoup

from rustysoup import BeautifulSoup, Soup


def test_attrs_get_and_getitem():
    soup = Soup("<a href='/x' data-id='1'>x</a>")
    tag = soup.find("a")

    assert tag.attrs == {"href": "/x", "data-id": "1"}
    assert tag.get("href") == "/x"
    assert tag.get("missing") is None
    assert tag.get("missing", "fallback") == "fallback"
    assert tag["data-id"] == "1"

    with pytest.raises(KeyError):
        tag["missing"]


def test_boolean_attr_lookup():
    soup = Soup("<form><input disabled name='email'><input name='name'></form>")

    assert len(soup.find_all("input", disabled=True)) == 1
    assert soup.find("input", disabled=True).get("name") == "email"


def test_attrs_dict_and_kwargs_normalization():
    soup = Soup("<div class='product selected' data-id='1' data_id='2'></div>")

    assert soup.find("div", attrs={"data-id": "1"}).name == "div"
    assert soup.find("div", class_="selected").name == "div"
    assert soup.find("div", data_id="2").name == "div"
    assert soup.find("div", data_id="1") is None


def test_duplicate_attrs_keep_last_value_like_bs4_html_parser():
    html = '<p id="a" id="b" class="x" class="y" data-k="1" data-k="2"></p>'
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    rusty_p = rusty.p
    bs4_p = bs4.p

    assert str(rusty) == str(bs4)
    assert rusty_p.attrs == bs4_p.attrs
    assert rusty_p["id"] == bs4_p["id"]
    assert rusty_p.get("class") == bs4_p.get("class")
    assert rusty_p.get("data-k") == bs4_p.get("data-k")
    assert [tag["id"] for tag in rusty.select("p#b")] == [
        tag["id"] for tag in bs4.select("p#b")
    ]
    assert rusty.select("p.x") == bs4.select("p.x") == []
    assert [tag["class"] for tag in rusty.select("p.y")] == [
        tag["class"] for tag in bs4.select("p.y")
    ]


def test_duplicate_attrs_in_full_documents_keep_last_value_like_bs4_html_parser():
    html = (
        '<!doctype html><html><body><a href="/old" href="/new" '
        'rel="prev" rel="next">x</a></body></html>'
    )
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(bs4)
    assert rusty.a.attrs == bs4.a.attrs
    assert rusty.a["href"] == bs4.a["href"]
    assert rusty.a.get("rel") == bs4.a.get("rel")


def test_attribute_set_to_none_matches_bs4_semantics():
    rusty = BeautifulSoup("<p></p><p></p>", "html.parser")
    bs4 = Bs4BeautifulSoup("<p></p><p></p>", "html.parser")

    rusty.find_all("p")[1]["data-x"] = None
    bs4.find_all("p")[1]["data-x"] = None

    assert rusty.find_all("p")[1]["data-x"] == bs4.find_all("p")[1]["data-x"]
    assert rusty.find_all("p")[1].attrs == bs4.find_all("p")[1].attrs
    assert str(rusty) == str(bs4)
    assert [str(tag) for tag in rusty.select("[data-x]")] == [
        str(tag) for tag in bs4.select("[data-x]")
    ]
    assert [str(tag) for tag in rusty.select('[data-x=""]')] == [
        str(tag) for tag in bs4.select('[data-x=""]')
    ]
    assert [str(tag) for tag in rusty.find_all(attrs={"data-x": None})] == [
        str(tag) for tag in bs4.find_all(attrs={"data-x": None})
    ]
    assert [str(tag) for tag in rusty.find_all(attrs={"data-x": True})] == [
        str(tag) for tag in bs4.find_all(attrs={"data-x": True})
    ]
