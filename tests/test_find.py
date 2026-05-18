from rustysoup import Soup


def test_find_by_name_and_attrs():
    soup = Soup(
        """
        <section>
          <article class="product featured" data-id="1"><span>one</span></article>
          <article class="product" data-id="2"><span>two</span></article>
        </section>
        """
    )

    assert soup.find("article", attrs={"data-id": "1"}).text == "one"
    assert soup.find("article", class_="featured").get("data-id") == "1"
    assert soup.find(attrs={"data-id": "2"}).text == "two"


def test_find_all_preserves_document_order():
    soup = Soup("<main><a>one</a><div><a>two</a></div><a>three</a></main>")

    assert [tag.text for tag in soup.find_all("a")] == ["one", "two", "three"]


def test_tag_find_searches_descendants():
    soup = Soup("<div id='outer'><p><span>hit</span></p></div>")
    div = soup.find("div")

    assert div.find("span").text == "hit"
    assert div.find("div") is None


def test_parent_children_and_descendants():
    soup = Soup("<div><p><span>one</span></p><a>two</a></div>")
    div = soup.find("div")
    span = soup.find("span")

    assert span.parent.name == "p"
    assert [child.name for child in div.children] == ["p", "a"]
    assert [tag.name for tag in div.descendants if getattr(tag, "name", None)] == ["p", "span", "a"]
