import pytest
from bs4 import BeautifulSoup as Bs4BeautifulSoup

from rustysoup import BeautifulSoup, NavigableString


def test_navigable_string_object_model_matches_bs4():
    rusty = BeautifulSoup("<p>hello</p>", "html.parser")
    bs4 = Bs4BeautifulSoup("<p>hello</p>", "html.parser")

    assert isinstance(rusty.p.contents[0], NavigableString)
    assert rusty.p.contents[0].parent.name == bs4.p.contents[0].parent.name
    assert str(rusty.p.contents[0]) == str(bs4.p.contents[0])
    assert rusty.p.contents[0] == str(bs4.p.contents[0])


def test_append_mutation_matches_bs4():
    rusty = BeautifulSoup("<div></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div></div>", "html.parser")

    rusty.div.append("hello")
    bs4.div.append("hello")

    assert str(rusty.div) == str(bs4.div)


def test_extract_mutation_matches_bs4():
    rusty = BeautifulSoup("<div><p>remove</p><span>keep</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>remove</p><span>keep</span></div>", "html.parser")

    extracted_rusty = rusty.p.extract()
    extracted_bs4 = bs4.p.extract()

    assert str(rusty.div) == str(bs4.div)
    assert str(extracted_rusty) == str(extracted_bs4)


def test_decompose_mutation_matches_bs4_parent_tree():
    rusty = BeautifulSoup("<div><p>remove</p><span>keep</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>remove</p><span>keep</span></div>", "html.parser")

    rusty.p.decompose()
    bs4.p.decompose()

    assert str(rusty.div) == str(bs4.div)


def test_find_next_matches_bs4():
    rusty = BeautifulSoup("<main><p>one</p><span>two</span></main>", "html.parser")
    bs4 = Bs4BeautifulSoup("<main><p>one</p><span>two</span></main>", "html.parser")

    assert str(rusty.p.find_next("span")) == str(bs4.p.find_next("span"))
    assert str(rusty.p.find_next(string=True)) == str(bs4.p.find_next(string=True))
    assert str(rusty.p.find_next("missing")) == str(bs4.p.find_next("missing"))


def test_find_all_next_matches_bs4():
    rusty = BeautifulSoup("<main><p>one</p><span>two</span><span>three</span></main>", "html.parser")
    bs4 = Bs4BeautifulSoup("<main><p>one</p><span>two</span><span>three</span></main>", "html.parser")

    assert [str(tag) for tag in rusty.p.find_all_next("span")] == [
        str(tag) for tag in bs4.p.find_all_next("span")
    ]
    assert [str(tag) for tag in rusty.p.find_all_next("span", limit=1)] == [
        str(tag) for tag in bs4.p.find_all_next("span", limit=1)
    ]


def test_new_tag_matches_bs4():
    rusty = BeautifulSoup("<div></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div></div>", "html.parser")

    assert str(rusty.new_tag("a", href="/x")) == str(bs4.new_tag("a", href="/x"))
    assert str(rusty.new_tag("a", attrs={"data-id": "1"}, href="/x")) == str(
        bs4.new_tag("a", attrs={"data-id": "1"}, href="/x")
    )
    assert str(rusty.new_tag("meta", charset="utf-8")) == str(bs4.new_tag("meta", charset="utf-8"))


def test_get_attribute_list_matches_bs4():
    rusty = BeautifulSoup('<a rel="nofollow tag" class="a b" href="/x"></a>', "html.parser")
    bs4 = Bs4BeautifulSoup('<a rel="nofollow tag" class="a b" href="/x"></a>', "html.parser")

    assert rusty.a.get_attribute_list("rel") == bs4.a.get_attribute_list("rel")
    assert rusty.a.get_attribute_list("class") == bs4.a.get_attribute_list("class")
    assert rusty.a.get_attribute_list("href") == bs4.a.get_attribute_list("href")
    assert rusty.a.get_attribute_list("missing") == bs4.a.get_attribute_list("missing")


def test_navigable_string_is_str_subclass_like_bs4():
    rusty = BeautifulSoup("<p>hello</p>", "html.parser")
    bs4 = Bs4BeautifulSoup("<p>hello</p>", "html.parser")

    assert isinstance(rusty.p.string, str) == isinstance(bs4.p.string, str)


def test_append_new_tag_matches_bs4():
    rusty = BeautifulSoup("<div></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div></div>", "html.parser")

    rusty.div.append(rusty.new_tag("a", href="/x", string="x"))
    bs4.div.append(bs4.new_tag("a", href="/x", string="x"))

    assert str(rusty.div) == str(bs4.div)


def test_insert_mutation_matches_bs4():
    rusty = BeautifulSoup("<div><span>two</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><span>two</span></div>", "html.parser")

    rusty.div.insert(0, "one")
    bs4.div.insert(0, "one")

    assert str(rusty.div) == str(bs4.div)


def test_replace_with_matches_bs4():
    rusty = BeautifulSoup("<div><p>old</p></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>old</p></div>", "html.parser")

    rusty.p.replace_with("new")
    bs4.p.replace_with("new")

    assert str(rusty.div) == str(bs4.div)


def test_find_previous_matches_bs4():
    rusty = BeautifulSoup("<main><p>one</p><span>two</span></main>", "html.parser")
    bs4 = Bs4BeautifulSoup("<main><p>one</p><span>two</span></main>", "html.parser")

    assert str(rusty.span.find_previous("p")) == str(bs4.span.find_previous("p"))


def test_find_parent_and_find_parents_match_bs4():
    rusty = BeautifulSoup("<div><section><p><b>x</b></p></section></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><section><p><b>x</b></p></section></div>", "html.parser")

    assert str(rusty.b.find_parent("section")) == str(bs4.b.find_parent("section"))
    assert [node.name for node in rusty.b.find_parents(True)] == [
        node.name for node in bs4.b.find_parents(True)
    ]


def test_clear_matches_bs4():
    rusty = BeautifulSoup("<div><p>remove</p><span>keep?</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>remove</p><span>keep?</span></div>", "html.parser")

    rusty.div.clear()
    bs4.div.clear()

    assert str(rusty.div) == str(bs4.div)


def test_insert_before_after_match_bs4():
    rusty = BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")

    rusty.p.insert_before("before")
    rusty.span.insert_after("after")
    bs4.p.insert_before("before")
    bs4.span.insert_after("after")

    assert str(rusty.div) == str(bs4.div)


def test_wrap_unwrap_match_bs4():
    rusty = BeautifulSoup("<div><p>text</p></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>text</p></div>", "html.parser")

    rusty.p.wrap(rusty.new_tag("section"))
    bs4.p.wrap(bs4.new_tag("section"))
    assert str(rusty.div) == str(bs4.div)

    rusty.p.unwrap()
    bs4.p.unwrap()
    assert str(rusty.div) == str(bs4.div)


def test_sibling_search_matches_bs4():
    html = "<div><p>one</p> text <span>two</span><a>three</a></div>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert str(rusty.p.find_next_sibling("span")) == str(bs4.p.find_next_sibling("span"))
    assert [str(tag) for tag in rusty.p.find_next_siblings(True)] == [
        str(tag) for tag in bs4.p.find_next_siblings(True)
    ]
    assert str(rusty.span.find_previous_sibling("p")) == str(bs4.span.find_previous_sibling("p"))
    assert [str(tag) for tag in rusty.span.find_previous_siblings(True)] == [
        str(tag) for tag in bs4.span.find_previous_siblings(True)
    ]
    assert str(rusty.p.find_next_sibling(string=True)) == str(
        bs4.p.find_next_sibling(string=True)
    )


def test_legacy_camelcase_traversal_aliases_match_bs4():
    html = "<main><section><p>one</p><span>two</span><a>three</a></section></main>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert str(rusty.p.findNext("a")) == str(bs4.p.findNext("a"))
    assert [str(node) for node in rusty.p.findAllNext("a")] == [
        str(node) for node in bs4.p.findAllNext("a")
    ]
    assert str(rusty.a.findPrevious("p")) == str(bs4.a.findPrevious("p"))
    assert [str(node) for node in rusty.a.findAllPrevious("p")] == [
        str(node) for node in bs4.a.findAllPrevious("p")
    ]
    assert str(rusty.p.findNextSibling("span")) == str(bs4.p.findNextSibling("span"))
    assert [str(node) for node in rusty.p.findNextSiblings(True)] == [
        str(node) for node in bs4.p.findNextSiblings(True)
    ]
    assert str(rusty.a.findPreviousSibling("span")) == str(
        bs4.a.findPreviousSibling("span")
    )
    assert [str(node) for node in rusty.a.findPreviousSiblings(True)] == [
        str(node) for node in bs4.a.findPreviousSiblings(True)
    ]
    assert str(rusty.a.findParent("section")) == str(bs4.a.findParent("section"))
    assert [node.name for node in rusty.a.findParents(True)] == [
        node.name for node in bs4.a.findParents(True)
    ]


def test_multi_valued_attr_access_matches_bs4():
    rusty = BeautifulSoup('<a rel="nofollow tag" class="a b"></a>', "html.parser")
    bs4 = Bs4BeautifulSoup('<a rel="nofollow tag" class="a b"></a>', "html.parser")

    assert rusty.a["rel"] == bs4.a["rel"]
    assert rusty.a["class"] == bs4.a["class"]
    assert rusty.a.attrs == bs4.a.attrs


def test_tag_string_assignment_matches_bs4():
    rusty = BeautifulSoup("<p>old</p>", "html.parser")
    bs4 = Bs4BeautifulSoup("<p>old</p>", "html.parser")

    rusty.p.string = "new"
    bs4.p.string = "new"

    assert str(rusty.p) == str(bs4.p)


def test_replace_with_tag_matches_bs4():
    rusty = BeautifulSoup("<div><p>old</p></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>old</p></div>", "html.parser")

    rusty.p.replace_with(rusty.new_tag("span", string="new"))
    bs4.p.replace_with(bs4.new_tag("span", string="new"))

    assert str(rusty.div) == str(bs4.div)


def test_replace_with_return_value_matches_bs4():
    rusty = BeautifulSoup("<div><p>old</p></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>old</p></div>", "html.parser")

    assert str(rusty.p.replace_with("new")) == str(bs4.p.replace_with("new"))


def test_attr_mutation_matches_bs4():
    rusty = BeautifulSoup('<a href="/x" class="a b"></a>', "html.parser")
    bs4 = Bs4BeautifulSoup('<a href="/x" class="a b"></a>', "html.parser")

    rusty.a["href"] = "/y"
    rusty.a["data-id"] = "1"
    rusty.a["class"] = ["c", "d"]
    bs4.a["href"] = "/y"
    bs4.a["data-id"] = "1"
    bs4.a["class"] = ["c", "d"]

    assert rusty.a.attrs == bs4.a.attrs
    assert str(rusty.a) == str(bs4.a)

    del rusty.a["data-id"]
    del bs4.a["data-id"]

    assert rusty.a.attrs == bs4.a.attrs
    assert str(rusty.a) == str(bs4.a)


def test_tag_rename_matches_bs4():
    rusty = BeautifulSoup('<a href="/x">x</a>', "html.parser")
    bs4 = Bs4BeautifulSoup('<a href="/x">x</a>', "html.parser")

    rusty.a.name = "link"
    bs4.a.name = "link"

    assert str(rusty.link) == str(bs4.link)
    assert rusty.find("a") == bs4.find("a")


def test_next_previous_elements_properties_match_bs4():
    html = "<main><p>one</p><span>two</span></main>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert [str(node) for node in rusty.p.next_elements] == [
        str(node) for node in bs4.p.next_elements
    ]
    assert [str(node) for node in rusty.span.previous_elements] == [
        str(node) for node in bs4.span.previous_elements
    ]
    assert [str(node) for node in rusty.p.string.next_elements] == [
        str(node) for node in bs4.p.string.next_elements
    ]


def test_attrs_dict_mutation_matches_bs4():
    rusty = BeautifulSoup('<a href="/x"></a>', "html.parser")
    bs4 = Bs4BeautifulSoup('<a href="/x"></a>', "html.parser")

    rusty.a.attrs["href"] = "/y"
    bs4.a.attrs["href"] = "/y"

    assert str(rusty.a) == str(bs4.a)


def test_new_string_matches_bs4():
    rusty = BeautifulSoup("<div></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div></div>", "html.parser")

    rusty_string = rusty.new_string("<hello & goodbye>")
    bs4_string = bs4.new_string("<hello & goodbye>")

    assert str(rusty_string) == str(bs4_string)
    assert rusty_string.parent == bs4_string.parent

    rusty.div.append(rusty_string)
    bs4.div.append(bs4_string)
    assert str(rusty.div) == str(bs4.div)


def test_text_alias_matches_bs4():
    rusty = BeautifulSoup("<p>hello</p>", "html.parser")
    bs4 = Bs4BeautifulSoup("<p>hello</p>", "html.parser")

    assert [str(node) for node in rusty.find_all(text="hello")] == [
        str(node) for node in bs4.find_all(text="hello")
    ]
