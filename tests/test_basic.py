from rustysoup import BeautifulSoup, Soup, Tag


HTML = """
<html>
  <head><title>Hello</title></head>
  <body>
    <div class="product" data-id="1">
      <a href="/x">Product X</a>
      <span class="price">$10</span>
    </div>
  </body>
</html>
"""


def test_acceptance_example():
    soup = BeautifulSoup(HTML, "html.parser")

    assert soup.find("title").text == "Hello"
    assert soup.title.text == "Hello"
    assert soup.find("div", class_="product").get("data-id") == "1"
    assert soup.find("a")["href"] == "/x"
    assert soup.select("div.product a[href]")[0].text == "Product X"
    assert soup.find("span", class_="price").text == "$10"
    assert soup.find("missing") is None
    assert len(soup.find_all("a")) == 1


def test_public_imports_and_alias():
    soup = Soup("<p>Hello</p>")

    assert BeautifulSoup is Soup
    assert isinstance(soup.find("p"), Tag)
    assert soup.find("p").name == "p"


def test_str_and_repr():
    soup = Soup("<div><a href='/x'>x</a></div>")
    tag = soup.find("a")

    assert str(tag) == '<a href="/x">x</a>'
    assert repr(tag) == '<a href="/x">x</a>'
    assert repr(soup) == '<div><a href="/x">x</a></div>'
