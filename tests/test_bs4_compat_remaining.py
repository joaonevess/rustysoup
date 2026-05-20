import pytest
from bs4 import BeautifulSoup as Bs4BeautifulSoup

from rustysoup import BeautifulSoup


def test_new_string_append_preserves_object_parent_like_bs4():
    rusty = BeautifulSoup("<div></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div></div>", "html.parser")

    rusty_string = rusty.new_string("hello")
    bs4_string = bs4.new_string("hello")

    rusty.div.append(rusty_string)
    bs4.div.append(bs4_string)

    assert str(rusty_string.parent) == str(bs4_string.parent)


def test_comment_nodes_and_factory_match_bs4():
    from bs4 import Comment as Bs4Comment
    from rustysoup import Comment

    rusty = BeautifulSoup("<div><!--hidden--></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><!--hidden--></div>", "html.parser")

    rusty_comment = rusty.find(string=lambda node: isinstance(node, Comment))
    bs4_comment = bs4.find(string=lambda node: isinstance(node, Bs4Comment))

    assert isinstance(rusty.div.string, Comment)
    assert str(rusty_comment) == str(bs4_comment)

    rusty_new = rusty.new_string("fresh", Comment)
    bs4_new = bs4.new_string("fresh", Bs4Comment)
    rusty.div.clear()
    bs4.div.clear()
    rusty.div.append(rusty_new)
    bs4.div.append(bs4_new)

    assert isinstance(rusty_new, Comment)
    assert str(rusty_new.parent) == str(bs4_new.parent)
    assert str(rusty.div) == str(bs4.div)


def test_multi_valued_attr_list_mutation_is_live_like_bs4():
    rusty = BeautifulSoup('<p class="alpha"></p>', "html.parser")
    bs4 = Bs4BeautifulSoup('<p class="alpha"></p>', "html.parser")

    rusty.p.attrs["class"].append("beta")
    bs4.p.attrs["class"].append("beta")
    rusty.p["class"].append("gamma")
    bs4.p["class"].append("gamma")

    assert isinstance(rusty.p["class"], list)
    assert str(rusty.p) == str(bs4.p)


def test_soupsieve_contains_selector_matches_bs4():
    html = "<section><p>one</p><p><span>two</span></p></section>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert [tag.text for tag in rusty.select('p:-soup-contains("two")')] == [
        tag.text for tag in bs4.select('p:-soup-contains("two")')
    ]
    assert [str(tag) for tag in rusty.select('section :-soup-contains("two")')] == [
        str(tag) for tag in bs4.select('section :-soup-contains("two")')
    ]
    assert [tag.text for tag in rusty.select('p:-soup-contains("one", "two")')] == [
        tag.text for tag in bs4.select('p:-soup-contains("one", "two")')
    ]


def test_attrs_mapping_methods_match_bs4():
    rusty = BeautifulSoup('<p class="alpha"></p>', "html.parser")
    bs4 = Bs4BeautifulSoup('<p class="alpha"></p>', "html.parser")

    assert rusty.p.attrs.update({"id": "x"}, title="T") == bs4.p.attrs.update(
        {"id": "x"}, title="T"
    )
    assert rusty.p.attrs.setdefault("class", ["fallback"]) == bs4.p.attrs.setdefault(
        "class", ["fallback"]
    )
    assert rusty.p.attrs.setdefault("data-id", "1") == bs4.p.attrs.setdefault(
        "data-id", "1"
    )
    assert rusty.p.attrs.pop("class") == bs4.p.attrs.pop("class")
    assert rusty.p.attrs.pop("missing", "fallback") == bs4.p.attrs.pop(
        "missing", "fallback"
    )

    assert rusty.p.attrs == bs4.p.attrs
    assert str(rusty.p) == str(bs4.p)


def test_attrs_remaining_dict_methods_match_bs4():
    rusty = BeautifulSoup('<p id="x" title="T"></p>', "html.parser")
    bs4 = Bs4BeautifulSoup('<p id="x" title="T"></p>', "html.parser")

    assert rusty.p.attrs.popitem() == bs4.p.attrs.popitem()
    assert rusty.p.attrs == bs4.p.attrs
    assert str(rusty.p) == str(bs4.p)

    assert rusty.p.attrs.fromkeys(["a", "b"], "value") == bs4.p.attrs.fromkeys(
        ["a", "b"], "value"
    )
    assert rusty.p.attrs == bs4.p.attrs

    assert rusty.p.attrs.clear() == bs4.p.attrs.clear()
    assert rusty.p.attrs == bs4.p.attrs
    assert str(rusty.p) == str(bs4.p)

    with pytest.raises(KeyError):
        rusty.p.attrs.popitem()


def test_new_tag_append_preserves_object_parent_like_bs4():
    rusty = BeautifulSoup("<div></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div></div>", "html.parser")

    rusty_tag = rusty.new_tag("span")
    bs4_tag = bs4.new_tag("span")
    rusty.div.append(rusty_tag)
    bs4.div.append(bs4_tag)

    assert str(rusty_tag.parent) == str(bs4_tag.parent)


def test_direct_comment_constructor_append_matches_bs4():
    from bs4 import Comment as Bs4Comment
    from rustysoup import Comment

    rusty = BeautifulSoup("<div></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div></div>", "html.parser")

    rusty.div.append(Comment("hidden"))
    bs4.div.append(Bs4Comment("hidden"))

    assert str(rusty.div) == str(bs4.div)


def test_soupsieve_has_selector_matches_bs4():
    html = "<div><a>x</a></div><div><span>y</span></div><section><p><b>z</b></p></section>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert [str(tag) for tag in rusty.select("div:has(a)")] == [
        str(tag) for tag in bs4.select("div:has(a)")
    ]
    assert [str(tag) for tag in rusty.select("section:has(p b)")] == [
        str(tag) for tag in bs4.select("section:has(p b)")
    ]
    assert [str(tag) for tag in rusty.select("div:has(> a)")] == [
        str(tag) for tag in bs4.select("div:has(> a)")
    ]


def test_soupsieve_not_has_selector_matches_bs4():
    html = """
    <main>
      <div class="card"><a>x</a></div>
      <div class="card"><span>y</span></div>
      <section><p><b>z</b></p></section>
    </main>
    """
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert [str(tag) for tag in rusty.select("div:not(:has(a))")] == [
        str(tag) for tag in bs4.select("div:not(:has(a))")
    ]
    assert [str(tag) for tag in rusty.select(".card:not(:has(> a))")] == [
        str(tag) for tag in bs4.select(".card:not(:has(> a))")
    ]
    assert [str(tag) for tag in rusty.select("main :not(:has(b))")] == [
        str(tag) for tag in bs4.select("main :not(:has(b))")
    ]


def test_navigable_string_replace_with_matches_bs4():
    rusty = BeautifulSoup("<p>hello</p>", "html.parser")
    bs4 = Bs4BeautifulSoup("<p>hello</p>", "html.parser")

    rusty_old = rusty.p.string.replace_with("bye")
    bs4_old = bs4.p.string.replace_with("bye")

    assert str(rusty.p) == str(bs4.p)
    assert str(rusty_old) == str(bs4_old)
    assert rusty_old.parent == bs4_old.parent


def test_navigable_string_replace_with_existing_node_matches_bs4():
    rusty = BeautifulSoup("<div><p>hello</p></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>hello</p></div>", "html.parser")

    rusty_new = rusty.new_string("bye")
    bs4_new = bs4.new_string("bye")

    rusty_old = rusty.p.string.replace_with(rusty_new)
    bs4_old = bs4.p.string.replace_with(bs4_new)

    assert str(rusty.div) == str(bs4.div)
    assert str(rusty_new.parent) == str(bs4_new.parent)
    assert str(rusty_old) == str(bs4_old)
    assert rusty_old.parent == bs4_old.parent


def test_navigable_string_extract_matches_bs4():
    rusty = BeautifulSoup("<p>hello</p><span>next</span>", "html.parser")
    bs4 = Bs4BeautifulSoup("<p>hello</p><span>next</span>", "html.parser")

    rusty_old = rusty.p.string.extract()
    bs4_old = bs4.p.string.extract()

    assert str(rusty) == str(bs4)
    assert str(rusty_old) == str(bs4_old)
    assert rusty_old.parent == bs4_old.parent


def test_navigable_string_insert_before_after_matches_bs4():
    rusty = BeautifulSoup("<p>hello</p><span>next</span>", "html.parser")
    bs4 = Bs4BeautifulSoup("<p>hello</p><span>next</span>", "html.parser")

    rusty_text = rusty.p.string
    bs4_text = bs4.p.string

    rusty_before = rusty_text.insert_before("before")
    rusty_after = rusty_text.insert_after(rusty.new_tag("b", string="bold"))
    bs4_before = bs4_text.insert_before("before")
    bs4_after = bs4_text.insert_after(bs4.new_tag("b", string="bold"))

    assert str(rusty) == str(bs4)
    assert [str(node) for node in rusty_before] == [str(node) for node in bs4_before]
    assert [str(node) for node in rusty_after] == [str(node) for node in bs4_after]


def test_navigable_string_insert_existing_string_updates_parent_like_bs4():
    rusty = BeautifulSoup("<p>hello</p>", "html.parser")
    bs4 = Bs4BeautifulSoup("<p>hello</p>", "html.parser")

    rusty_new = rusty.new_string("before")
    bs4_new = bs4.new_string("before")

    rusty_inserted = rusty.p.string.insert_before(rusty_new)
    bs4_inserted = bs4.p.string.insert_before(bs4_new)

    assert str(rusty.p) == str(bs4.p)
    assert str(rusty_new.parent) == str(bs4_new.parent)
    assert [str(node) for node in rusty_inserted] == [str(node) for node in bs4_inserted]


def test_extend_index_and_legacy_replace_aliases_match_bs4():
    rusty = BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")

    rusty_extended = rusty.div.extend([" tail ", rusty.new_tag("b", string="bold")])
    bs4_extended = bs4.div.extend([" tail ", bs4.new_tag("b", string="bold")])

    assert str(rusty.div) == str(bs4.div)
    assert [str(node) for node in rusty_extended] == [str(node) for node in bs4_extended]
    assert rusty.div.index(rusty.span) == bs4.div.index(bs4.span)
    assert rusty.div.index(rusty.div.contents[2]) == bs4.div.index(bs4.div.contents[2])

    rusty_old = rusty.p.replaceWith("ONE")
    bs4_old = bs4.p.replaceWith("ONE")
    assert str(rusty.div) == str(bs4.div)
    assert str(rusty_old) == str(bs4_old)


def test_multi_item_insert_and_replace_match_bs4():
    rusty = BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")

    assert [str(node) for node in rusty.p.insert(0, "A", "B")] == [
        str(node) for node in bs4.p.insert(0, "A", "B")
    ]
    assert str(rusty) == str(bs4)

    rusty = BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    assert [str(node) for node in rusty.p.insert_before("A", "B")] == [
        str(node) for node in bs4.p.insert_before("A", "B")
    ]
    assert str(rusty) == str(bs4)

    rusty = BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    assert [str(node) for node in rusty.p.insert_after("A", "B")] == [
        str(node) for node in bs4.p.insert_after("A", "B")
    ]
    assert str(rusty) == str(bs4)

    rusty = BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    rusty_old = rusty.p.replace_with("A", "B")
    bs4_old = bs4.p.replace_with("A", "B")
    assert str(rusty_old) == str(bs4_old)
    assert str(rusty) == str(bs4)


def test_multi_item_navigable_string_mutations_match_bs4():
    rusty = BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")

    assert [str(node) for node in rusty.p.string.insert_before("A", "B")] == [
        str(node) for node in bs4.p.string.insert_before("A", "B")
    ]
    assert str(rusty) == str(bs4)

    rusty = BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    assert [str(node) for node in rusty.p.string.insert_after("A", "B")] == [
        str(node) for node in bs4.p.string.insert_after("A", "B")
    ]
    assert str(rusty) == str(bs4)

    rusty = BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    rusty_old = rusty.p.string.replace_with("A", "B")
    bs4_old = bs4.p.string.replace_with("A", "B")
    assert str(rusty_old) == str(bs4_old)
    assert str(rusty) == str(bs4)


def test_zero_item_insert_and_replace_match_bs4():
    rusty = BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")

    assert rusty.p.insert(0) == bs4.p.insert(0)
    assert rusty.p.insert_before() == bs4.p.insert_before()
    assert rusty.p.insert_after() == bs4.p.insert_after()

    rusty_old = rusty.p.replace_with()
    bs4_old = bs4.p.replace_with()
    assert str(rusty_old) == str(bs4_old)
    assert str(rusty) == str(bs4)


def test_self_mutation_guards_match_bs4_and_preserve_tree():
    def assert_value_error_matches(rusty_call, bs4_call):
        with pytest.raises(ValueError) as rusty_error:
            rusty_call()
        with pytest.raises(ValueError) as bs4_error:
            bs4_call()
        assert str(rusty_error.value) == str(bs4_error.value)

    cases = [
        (lambda soup: soup.p.insert_before(soup.p), lambda soup: soup.p.insert_before(soup.p)),
        (lambda soup: soup.p.insert_after(soup.p), lambda soup: soup.p.insert_after(soup.p)),
        (lambda soup: soup.p.append(soup.p), lambda soup: soup.p.append(soup.p)),
        (lambda soup: soup.p.insert(0, soup.p), lambda soup: soup.p.insert(0, soup.p)),
        (
            lambda soup: soup.p.string.insert_before(soup.p.string),
            lambda soup: soup.p.string.insert_before(soup.p.string),
        ),
        (
            lambda soup: soup.p.string.insert_after(soup.p.string),
            lambda soup: soup.p.string.insert_after(soup.p.string),
        ),
    ]
    for rusty_call, bs4_call in cases:
        rusty = BeautifulSoup("<div><p>one</p></div>", "html.parser")
        bs4 = Bs4BeautifulSoup("<div><p>one</p></div>", "html.parser")
        assert_value_error_matches(lambda: rusty_call(rusty), lambda: bs4_call(bs4))
        assert str(rusty) == str(bs4)

    rusty = BeautifulSoup("<div><p>one</p></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>one</p></div>", "html.parser")
    assert str(rusty.p.replace_with(rusty.p)) == str(bs4.p.replace_with(bs4.p))
    assert str(rusty) == str(bs4)

    rusty = BeautifulSoup("<p>one</p>", "html.parser")
    bs4 = Bs4BeautifulSoup("<p>one</p>", "html.parser")
    rusty_text = rusty.p.string
    bs4_text = bs4.p.string
    assert str(rusty_text.replace_with(rusty_text)) == str(
        bs4_text.replace_with(bs4_text)
    )
    assert str(rusty) == str(bs4)


def test_negative_insert_index_errors_match_bs4():
    rusty = BeautifulSoup("<p>ab</p>", "html.parser")
    bs4 = Bs4BeautifulSoup("<p>ab</p>", "html.parser")

    for rusty_node, bs4_node in ((rusty, bs4), (rusty.p, bs4.p)):
        with pytest.raises(IndexError) as rusty_error:
            rusty_node.insert(-1, "x")
        with pytest.raises(IndexError) as bs4_error:
            bs4_node.insert(-1, "x")
        assert str(rusty_error.value) == str(bs4_error.value)


def test_document_multi_item_insert_and_root_mutation_errors_match_bs4():
    rusty = BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")

    assert [str(node) for node in rusty.insert(0, "A", "B")] == [
        str(node) for node in bs4.insert(0, "A", "B")
    ]
    assert str(rusty) == str(bs4)

    for method_name in ("insert_before", "insert_after"):
        with pytest.raises(NotImplementedError) as rusty_error:
            getattr(rusty, method_name)("x")
        with pytest.raises(NotImplementedError) as bs4_error:
            getattr(bs4, method_name)("x")
        assert str(rusty_error.value) == str(bs4_error.value)


def test_replace_with_children_and_smooth_match_bs4():
    rusty = BeautifulSoup("<div><p><b>one</b><i>two</i></p></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p><b>one</b><i>two</i></p></div>", "html.parser")

    rusty_old = rusty.p.replace_with_children()
    bs4_old = bs4.p.replace_with_children()

    assert str(rusty.div) == str(bs4.div)
    assert str(rusty_old) == str(bs4_old)

    rusty = BeautifulSoup("<p>a<span>b</span></p>", "html.parser")
    bs4 = Bs4BeautifulSoup("<p>a<span>b</span></p>", "html.parser")
    rusty.p.append("c")
    rusty.p.append("d")
    bs4.p.append("c")
    bs4.p.append("d")

    assert rusty.p.smooth() == bs4.p.smooth()
    assert str(rusty.p) == str(bs4.p)
    assert [str(node) for node in rusty.p.contents] == [
        str(node) for node in bs4.p.contents
    ]


def test_document_tag_like_attrs_and_mutation_match_bs4():
    rusty = BeautifulSoup("<div>one</div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div>one</div>", "html.parser")

    assert rusty.has_attr("class") == bs4.has_attr("class")
    assert rusty.has_key("class") == bs4.has_key("class")
    assert rusty.get("class", "fallback") == bs4.get("class", "fallback")
    assert rusty.get_attribute_list("class") == bs4.get_attribute_list("class")
    assert rusty.index(rusty.div) == bs4.index(bs4.div)

    rusty.append(rusty.new_tag("p", string="two"))
    bs4.append(bs4.new_tag("p", string="two"))
    rusty.insert(1, " middle ")
    bs4.insert(1, " middle ")
    assert str(rusty) == str(bs4)

    rusty_extended = rusty.extend([" tail ", rusty.new_tag("span", string="three")])
    bs4_extended = bs4.extend([" tail ", bs4.new_tag("span", string="three")])
    assert str(rusty) == str(bs4)
    assert [str(node) for node in rusty_extended] == [str(node) for node in bs4_extended]

    rusty.smooth()
    bs4.smooth()
    assert str(rusty) == str(bs4)

    assert rusty.clear() == bs4.clear()
    assert str(rusty) == str(bs4)


def test_document_extract_decompose_and_invalid_mutations_match_bs4():
    rusty = BeautifulSoup("<div>one</div><p>two</p>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div>one</div><p>two</p>", "html.parser")

    assert str(rusty.extract()) == str(bs4.extract())
    assert str(rusty) == str(bs4)

    for method_name in ["insert_before", "insert_after"]:
        rusty = BeautifulSoup("<div>one</div>", "html.parser")
        bs4 = Bs4BeautifulSoup("<div>one</div>", "html.parser")
        try:
            getattr(bs4, method_name)("x")
        except Exception as exc:
            expected_type = type(exc)
        with pytest.raises(expected_type):
            getattr(rusty, method_name)("x")

    for method_name in [
        "replace_with",
        "replaceWith",
        "wrap",
        "unwrap",
        "replace_with_children",
        "replaceWithChildren",
    ]:
        rusty = BeautifulSoup("<div>one</div>", "html.parser")
        bs4 = Bs4BeautifulSoup("<div>one</div>", "html.parser")
        expected_type = None
        try:
            if method_name == "wrap":
                getattr(bs4, method_name)(bs4.new_tag("section"))
            elif "children" in method_name.lower() or method_name == "unwrap":
                getattr(bs4, method_name)()
            else:
                getattr(bs4, method_name)("x")
        except Exception as exc:
            expected_type = type(exc)
        with pytest.raises(expected_type):
            if method_name == "wrap":
                getattr(rusty, method_name)(rusty.new_tag("section"))
            elif "children" in method_name.lower() or method_name == "unwrap":
                getattr(rusty, method_name)()
            else:
                getattr(rusty, method_name)("x")

    rusty = BeautifulSoup("<div>one</div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div>one</div>", "html.parser")
    assert rusty.decompose() == bs4.decompose()
    assert rusty.name == bs4.name
    assert rusty.decomposed == bs4.decomposed
    assert rusty.contents == bs4.contents
    assert rusty.attrs == bs4.attrs
    assert rusty.can_be_empty_element == bs4.can_be_empty_element
    assert str(rusty) == str(bs4)


def test_document_builder_internal_defaults_and_basic_handlers_match_bs4():
    rusty = BeautifulSoup("<p>x</p>", "html.parser")
    bs4 = Bs4BeautifulSoup("<p>x</p>", "html.parser")

    assert rusty.ASCII_SPACES == bs4.ASCII_SPACES
    assert rusty.DEFAULT_BUILDER_FEATURES == bs4.DEFAULT_BUILDER_FEATURES
    assert rusty.ROOT_TAG_NAME == bs4.ROOT_TAG_NAME
    assert rusty.contains_replacement_characters == bs4.contains_replacement_characters
    assert rusty.declared_html_encoding == bs4.declared_html_encoding
    assert rusty.element_classes == bs4.element_classes
    assert rusty.markup == bs4.markup
    assert rusty.parse_only == bs4.parse_only
    assert rusty.currentTag is rusty
    assert [node.name for node in rusty.tagStack] == [node.name for node in bs4.tagStack]

    rusty = BeautifulSoup("", "html.parser")
    bs4 = Bs4BeautifulSoup("", "html.parser")
    rusty_tag = rusty.handle_starttag("div", None, None, {"id": "x"})
    bs4_tag = bs4.handle_starttag("div", None, None, {"id": "x"})
    assert str(rusty_tag) == str(bs4_tag)
    assert str(rusty) == str(bs4)

    assert rusty.handle_data("hi") == bs4.handle_data("hi")
    assert rusty.current_data == bs4.current_data
    assert rusty.endData() == bs4.endData()
    assert str(rusty) == str(bs4)
    assert rusty.current_data == bs4.current_data

    assert str(rusty.popTag()) == str(bs4.popTag())
    assert rusty.open_tag_counter == bs4.open_tag_counter
    assert rusty.reset() == bs4.reset()
    assert str(rusty) == str(bs4)


def test_navigable_string_search_methods_match_bs4():
    html = "<div><p>one</p> text <span>two</span><a>three</a></div>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    rusty_text = rusty.p.string
    bs4_text = bs4.p.string

    assert str(rusty_text.find_next("span")) == str(bs4_text.find_next("span"))
    assert [str(node) for node in rusty_text.find_all_next(["span", "a"])] == [
        str(node) for node in bs4_text.find_all_next(["span", "a"])
    ]
    assert [str(node) for node in rusty_text.find_all_next(string=True, limit=2)] == [
        str(node) for node in bs4_text.find_all_next(string=True, limit=2)
    ]
    assert str(rusty.span.string.find_previous("p")) == str(
        bs4.span.string.find_previous("p")
    )
    assert [str(node) for node in rusty.span.string.find_all_previous(True)] == [
        str(node) for node in bs4.span.string.find_all_previous(True)
    ]
    assert str(rusty_text.find_parent("p")) == str(bs4_text.find_parent("p"))
    assert [node.name for node in rusty_text.find_parents(True)] == [
        node.name for node in bs4_text.find_parents(True)
    ]
    assert rusty_text.find_next_sibling(string=True) == bs4_text.find_next_sibling(
        string=True
    )
    assert [str(node) for node in rusty_text.find_next_siblings(True)] == [
        str(node) for node in bs4_text.find_next_siblings(True)
    ]


def test_navigable_string_legacy_search_aliases_and_wrap_match_bs4():
    rusty = BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>one</p><span>two</span></div>", "html.parser")

    assert str(rusty.p.string.findNext("span")) == str(bs4.p.string.findNext("span"))
    assert [str(node) for node in rusty.p.string.findAllNext("span")] == [
        str(node) for node in bs4.p.string.findAllNext("span")
    ]
    assert str(rusty.span.string.findPrevious("p")) == str(
        bs4.span.string.findPrevious("p")
    )
    assert [str(node) for node in rusty.span.string.findAllPrevious("p")] == [
        str(node) for node in bs4.span.string.findAllPrevious("p")
    ]
    assert str(rusty.p.string.findParent("p")) == str(bs4.p.string.findParent("p"))
    assert [node.name for node in rusty.p.string.findParents(True)] == [
        node.name for node in bs4.p.string.findParents(True)
    ]

    rusty = BeautifulSoup("<p>hello</p>", "html.parser")
    bs4 = Bs4BeautifulSoup("<p>hello</p>", "html.parser")
    rusty_text = rusty.p.string
    bs4_text = bs4.p.string

    rusty_wrapped = rusty_text.wrap(rusty.new_tag("b"))
    bs4_wrapped = bs4_text.wrap(bs4.new_tag("b"))

    assert str(rusty.p) == str(bs4.p)
    assert str(rusty_wrapped) == str(bs4_wrapped)
    assert str(rusty_text.parent) == str(bs4_text.parent)
