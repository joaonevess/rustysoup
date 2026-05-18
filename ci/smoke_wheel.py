from __future__ import annotations

import importlib.metadata

from rustysoup import BeautifulSoup, Soup, Tag


HTML = """
<html>
  <head>
    <title>Hello</title>
    <meta name="description" content="Smoke test">
  </head>
  <body>
    <main id="main">
      <div class="product" data-id="1">
        <a href="/x">Product X</a>
        <span class="price">$10</span>
      </div>
      <form><input name="email" disabled></form>
    </main>
  </body>
</html>
"""


def main() -> None:
    assert importlib.metadata.version("rustysoup")
    soup = BeautifulSoup(HTML, "html.parser")
    assert isinstance(soup.find("div"), Tag)
    assert isinstance(Soup(HTML).find("a"), Tag)
    assert soup.find("title").text == "Hello"
    assert soup.title.text == "Hello"
    assert soup.find("div", class_="product").get("data-id") == "1"
    assert soup.find("a")["href"] == "/x"
    assert soup.select("div.product a[href]")[0].text == "Product X"
    assert soup.find("span", class_="price").text == "$10"
    assert soup.find("missing") is None
    assert len(soup.find_all("a")) == 1
    assert soup.find("input", disabled=True).get("name") == "email"
    assert soup.get_text(" ", strip=True)

    link = soup.find("a")
    del soup
    assert link["href"] == "/x"
    assert link.parent.name == "div"

    try:
        BeautifulSoup(HTML, "html.parser").select("div > > a")
    except ValueError:
        pass
    else:
        raise AssertionError("invalid CSS selector did not raise ValueError")


if __name__ == "__main__":
    main()
