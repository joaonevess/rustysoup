from bs4 import BeautifulSoup as Bs4BeautifulSoup
from bs4 import Comment as Bs4Comment
from bs4 import Doctype as Bs4Doctype
from bs4 import NavigableString as Bs4NavigableString
from bs4 import ProcessingInstruction as Bs4ProcessingInstruction
from bs4 import Script as Bs4Script
from bs4 import Stylesheet as Bs4Stylesheet

from rustysoup import (
    BeautifulSoup,
    Comment,
    Doctype,
    NavigableString,
    ProcessingInstruction,
    Script,
    Stylesheet,
)


HTML = "<div><p title='x & y'>One &amp; Two</p><br><span>Three</span></div>"


def test_decode_and_encode_contents_match_bs4_for_basic_html():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert rusty.div.decode_contents() == bs4.div.decode_contents()
    assert rusty.div.encode_contents() == bs4.div.encode_contents()


def test_legacy_serialization_aliases_match_bs4():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert rusty.getText("|", strip=True) == bs4.getText("|", strip=True)
    assert rusty.div.getText("|", strip=True) == bs4.div.getText("|", strip=True)
    assert rusty.renderContents() == bs4.renderContents()
    assert rusty.div.renderContents() == bs4.div.renderContents()
    assert rusty.decode() == bs4.decode()
    assert rusty.div.decode() == bs4.div.decode()
    assert rusty.encode() == bs4.encode()
    assert rusty.div.encode() == bs4.div.encode()


def test_prettify_matches_bs4_for_simple_tree():
    rusty = BeautifulSoup("<div><p>One</p></div>", "html.parser")
    bs4 = Bs4BeautifulSoup("<div><p>One</p></div>", "html.parser")

    assert rusty.div.prettify() == bs4.div.prettify()


def test_prettify_accepts_formatter_and_encoding_like_bs4():
    html = '<div><p title="x &amp; y">One &amp; Two</p></div>'
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert rusty.prettify(formatter="minimal") == bs4.prettify(formatter="minimal")
    assert rusty.div.prettify(formatter="minimal") == bs4.div.prettify(formatter="minimal")
    assert rusty.prettify(formatter=None) == bs4.prettify(formatter=None)
    assert rusty.div.prettify(formatter=None) == bs4.div.prettify(formatter=None)
    assert rusty.prettify("utf-8") == bs4.prettify("utf-8")
    assert rusty.div.prettify("utf-8") == bs4.div.prettify("utf-8")


def test_text_methods_accept_bs4_types_argument():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert rusty.get_text("|", strip=True, types=(NavigableString,)) == bs4.get_text(
        "|", strip=True, types=(Bs4NavigableString,)
    )
    assert rusty.div.getText("|", strip=True, types=(NavigableString,)) == bs4.div.getText(
        "|", strip=True, types=(Bs4NavigableString,)
    )


def test_get_text_types_filter_comments_like_bs4():
    html = "<div>one<!--hidden--><span>two</span></div>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert rusty.div.get_text("|", strip=True, types=None) == bs4.div.get_text(
        "|", strip=True, types=None
    )
    assert rusty.div.get_text("|", strip=True, types=(Comment,)) == bs4.div.get_text(
        "|", strip=True, types=(Bs4Comment,)
    )
    assert rusty.div.get_text("|", strip=True, types=Comment) == bs4.div.get_text(
        "|", strip=True, types=Bs4Comment
    )
    assert rusty.div.get_text("|", strip=True, types=(NavigableString, Comment)) == (
        bs4.div.get_text("|", strip=True, types=(Bs4NavigableString, Bs4Comment))
    )
    assert rusty.div.get_text("|", strip=True, types=(int,)) == bs4.div.get_text(
        "|", strip=True, types=(int,)
    )


def test_serialization_methods_accept_common_bs4_arguments():
    rusty = BeautifulSoup(HTML, "html.parser")
    bs4 = Bs4BeautifulSoup(HTML, "html.parser")

    assert rusty.div.decode_contents(formatter="minimal") == bs4.div.decode_contents(
        formatter="minimal"
    )
    assert rusty.div.encode_contents(encoding="utf-8", formatter="minimal") == (
        bs4.div.encode_contents(encoding="utf-8", formatter="minimal")
    )
    assert rusty.decode(formatter="minimal") == bs4.decode(formatter="minimal")
    assert rusty.div.decode(formatter="minimal") == bs4.div.decode(formatter="minimal")
    assert rusty.encode("utf-8", errors="xmlcharrefreplace") == bs4.encode(
        "utf-8", errors="xmlcharrefreplace"
    )
    assert rusty.div.encode("utf-8", errors="xmlcharrefreplace") == bs4.div.encode(
        "utf-8", errors="xmlcharrefreplace"
    )
    assert rusty.renderContents("utf-8") == bs4.renderContents("utf-8")
    assert rusty.div.renderContents("utf-8") == bs4.div.renderContents("utf-8")


def test_serialization_methods_accept_formatter_none_like_bs4():
    html = '<div><p title="x &amp; y">One &amp; Two</p></div>'
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert rusty.decode(formatter=None) == bs4.decode(formatter=None)
    assert rusty.div.decode(formatter=None) == bs4.div.decode(formatter=None)
    assert rusty.div.decode_contents(formatter=None) == bs4.div.decode_contents(
        formatter=None
    )
    assert rusty.encode(formatter=None) == bs4.encode(formatter=None)
    assert rusty.div.encode(formatter=None) == bs4.div.encode(formatter=None)
    assert rusty.div.encode_contents(formatter=None) == bs4.div.encode_contents(
        formatter=None
    )


def test_serialization_methods_accept_callable_formatter_like_bs4():
    html = (
        '<div><p title="x &amp; y">One &amp; Two</p>'
        '<!--hide & seek--><script>a < b && c</script></div>'
    )
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    def upper_bracket(value):
        return f"[{value.upper()}]"

    assert rusty.decode(formatter=upper_bracket) == bs4.decode(formatter=upper_bracket)
    assert rusty.div.decode(formatter=upper_bracket) == bs4.div.decode(
        formatter=upper_bracket
    )
    assert rusty.div.decode_contents(formatter=upper_bracket) == (
        bs4.div.decode_contents(formatter=upper_bracket)
    )
    assert rusty.prettify(formatter=upper_bracket) == bs4.prettify(
        formatter=upper_bracket
    )
    assert rusty.div.prettify(formatter=upper_bracket) == bs4.div.prettify(
        formatter=upper_bracket
    )
    assert rusty.encode(formatter=upper_bracket) == bs4.encode(formatter=upper_bracket)
    assert rusty.div.encode_contents(formatter=upper_bracket) == (
        bs4.div.encode_contents(formatter=upper_bracket)
    )


def test_format_string_and_output_ready_match_bs4_basics():
    rusty = BeautifulSoup("<p>one & two</p><!--hidden-->", "html.parser")
    bs4 = Bs4BeautifulSoup("<p>one & two</p><!--hidden-->", "html.parser")

    for rusty_node, bs4_node in [(rusty, bs4), (rusty.p, bs4.p), (rusty.p.string, bs4.p.string)]:
        assert rusty_node.default == bs4_node.default
        assert rusty_node.format_string("a & b", "minimal") == bs4_node.format_string(
            "a & b", "minimal"
        )
        assert rusty_node.format_string("a & b", None) == bs4_node.format_string(
            "a & b", None
        )
        assert rusty_node.formatter_for_name("minimal").substitute("a & b") == (
            bs4_node.formatter_for_name("minimal").substitute("a & b")
        )

    assert rusty.p.string.PREFIX == bs4.p.string.PREFIX
    assert rusty.p.string.SUFFIX == bs4.p.string.SUFFIX
    assert rusty.p.string.output_ready() == bs4.p.string.output_ready()

    rusty_comment = rusty.find(string=lambda node: isinstance(node, Comment))
    bs4_comment = bs4.find(string=lambda node: isinstance(node, Bs4Comment))
    assert rusty_comment.PREFIX == bs4_comment.PREFIX
    assert rusty_comment.SUFFIX == bs4_comment.SUFFIX
    assert rusty_comment.output_ready() == bs4_comment.output_ready()

    assert BeautifulSoup("<p>x</p>", "html.parser").p.setup() == (
        Bs4BeautifulSoup("<p>x</p>", "html.parser").p.setup()
    )


def test_tag_class_constants_and_copy_self_match_bs4_shape():
    rusty = BeautifulSoup('<p id="x" class="a b"><b>child</b></p>', "html.parser")
    bs4 = Bs4BeautifulSoup('<p id="x" class="a b"><b>child</b></p>', "html.parser")

    assert rusty.p.START_ELEMENT_EVENT is rusty.START_ELEMENT_EVENT
    assert rusty.p.attribute_value_list_class.__name__ == (
        bs4.p.attribute_value_list_class.__name__
    )
    assert rusty.p.preserve_whitespace_tags == bs4.p.preserve_whitespace_tags
    assert rusty.p.cdata_list_attributes == bs4.p.cdata_list_attributes

    rusty_copy = rusty.p.copy_self()
    bs4_copy = bs4.p.copy_self()
    assert rusty_copy.name == bs4_copy.name
    assert rusty_copy.attrs == bs4_copy.attrs
    assert rusty_copy.contents == bs4_copy.contents

    assert str(rusty.copy_self()) == str(bs4.copy_self())


def test_repr_matches_bs4_serialized_markup():
    html = '<div><p id="x">hi</p><!--hidden--></div>'
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert repr(rusty) == repr(bs4)
    assert repr(rusty.div) == repr(bs4.div)
    assert repr(rusty.p) == repr(bs4.p)
    assert repr(rusty.p.string) == repr(bs4.p.string)


def test_inter_tag_whitespace_normalization_matches_bs4():
    html = "<div>\n  <p>x</p>\n\t<span>y</span>\n</div>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(bs4)
    assert rusty.get_text("|", strip=False) == bs4.get_text("|", strip=False)
    assert [str(node) for node in rusty.div.contents] == [
        str(node) for node in bs4.div.contents
    ]


def test_space_only_text_collapses_but_pre_text_preserves_like_bs4():
    html = "<div>  <span>  </span></div><pre>\n  x\n</pre>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(bs4)
    assert rusty.get_text("|", strip=False) == bs4.get_text("|", strip=False)
    assert str(rusty.pre.string) == str(bs4.pre.string)


def test_script_and_style_raw_text_matches_bs4_defaults():
    html = '<script>if (a < b) { c(); }</script><style>.x > y {}</style><p>visible</p>'
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(bs4)
    assert rusty.decode() == bs4.decode()
    assert rusty.prettify() == bs4.prettify()
    assert rusty.get_text("|", strip=False) == bs4.get_text("|", strip=False)
    assert rusty.get_text("|", strip=False, types=None) == bs4.get_text(
        "|", strip=False, types=None
    )
    assert list(rusty.strings) == list(bs4.strings)
    assert list(rusty.script.strings) == list(bs4.script.strings)
    assert rusty.script.text == bs4.script.text
    assert rusty.style.text == bs4.style.text


def test_script_and_stylesheet_string_nodes_match_bs4_types_and_output():
    html = '<script>if (a < b) { c(); }</script><style>.x > y {}</style><p>visible</p>'
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert type(rusty.script.string).__name__ == type(bs4.script.string).__name__
    assert type(rusty.style.string).__name__ == type(bs4.style.string).__name__
    assert isinstance(rusty.script.string, Script)
    assert isinstance(rusty.style.string, Stylesheet)
    assert rusty.script.string.PREFIX == bs4.script.string.PREFIX
    assert rusty.script.string.SUFFIX == bs4.script.string.SUFFIX
    assert rusty.style.string.PREFIX == bs4.style.string.PREFIX
    assert rusty.style.string.SUFFIX == bs4.style.string.SUFFIX
    assert rusty.script.string.output_ready() == bs4.script.string.output_ready()
    assert rusty.style.string.output_ready() == bs4.style.string.output_ready()

    assert [(type(node).__name__, str(node)) for node in rusty.find_all(string=True)] == [
        (type(node).__name__, str(node)) for node in bs4.find_all(string=True)
    ]
    assert [(type(node).__name__, str(node)) for node in rusty.strings] == [
        (type(node).__name__, str(node)) for node in bs4.strings
    ]
    assert [(type(node).__name__, str(node)) for node in rusty.script.strings] == [
        (type(node).__name__, str(node)) for node in bs4.script.strings
    ]

    assert rusty.get_text("|", types=(NavigableString,)) == bs4.get_text(
        "|", types=(Bs4NavigableString,)
    )
    assert rusty.get_text("|", types=(Script,)) == bs4.get_text("|", types=(Bs4Script,))
    assert rusty.get_text("|", types=(Stylesheet,)) == bs4.get_text(
        "|", types=(Bs4Stylesheet,)
    )
    assert rusty.get_text("|", types=(Script, Stylesheet, NavigableString)) == (
        bs4.get_text("|", types=(Bs4Script, Bs4Stylesheet, Bs4NavigableString))
    )

    assert rusty.script.get_text("|", types=(NavigableString,)) == bs4.script.get_text(
        "|", types=(Bs4NavigableString,)
    )
    assert rusty.script.get_text("|", types=(Script,)) == bs4.script.get_text(
        "|", types=(Bs4Script,)
    )
    assert rusty.style.get_text("|", types=(Stylesheet,)) == bs4.style.get_text(
        "|", types=(Bs4Stylesheet,)
    )
    assert rusty.script.string.get_text("|", strip=True) == bs4.script.string.get_text(
        "|", strip=True
    )
    assert rusty.script.string.get_text("|", strip=True, types=None) == (
        bs4.script.string.get_text("|", strip=True, types=None)
    )
    assert rusty.script.string.get_text("|", strip=True, types=(Script,)) == (
        bs4.script.string.get_text("|", strip=True, types=(Bs4Script,))
    )


def test_doctype_node_model_and_serialization_match_bs4():
    html = "<!DOCTYPE html><html><body><p>x</p></body></html>"
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(bs4)
    assert rusty.decode() == bs4.decode()
    assert rusty.prettify() == bs4.prettify()

    rusty_doctype = rusty.contents[0]
    bs4_doctype = bs4.contents[0]
    assert type(rusty_doctype).__name__ == type(bs4_doctype).__name__
    assert isinstance(rusty_doctype, Doctype)
    assert str(rusty_doctype) == str(bs4_doctype)
    assert rusty_doctype.output_ready() == bs4_doctype.output_ready()
    assert rusty_doctype.PREFIX == bs4_doctype.PREFIX
    assert rusty_doctype.SUFFIX == bs4_doctype.SUFFIX

    assert [(type(node).__name__, str(node)) for node in rusty.find_all(string=True)] == [
        (type(node).__name__, str(node)) for node in bs4.find_all(string=True)
    ]
    assert list(rusty.strings) == list(bs4.strings)
    assert rusty.get_text("|") == bs4.get_text("|")
    assert rusty.get_text("|", types=None) == bs4.get_text("|", types=None)
    assert rusty.get_text("|", types=(NavigableString,)) == bs4.get_text(
        "|", types=(Bs4NavigableString,)
    )
    assert rusty.get_text("|", types=(Doctype, NavigableString)) == bs4.get_text(
        "|", types=(Bs4Doctype, Bs4NavigableString)
    )


def test_processing_instruction_node_model_and_serialization_match_bs4():
    html = '<?pi data?><p>x</p><!--?not pi?--><?xml version="1.0"?>'
    rusty = BeautifulSoup(html, "html.parser")
    bs4 = Bs4BeautifulSoup(html, "html.parser")

    assert str(rusty) == str(bs4)
    assert rusty.decode() == bs4.decode()
    assert rusty.prettify() == bs4.prettify()

    rusty_pi = rusty.contents[0]
    bs4_pi = bs4.contents[0]
    assert type(rusty_pi).__name__ == type(bs4_pi).__name__
    assert isinstance(rusty_pi, ProcessingInstruction)
    assert str(rusty_pi) == str(bs4_pi)
    assert rusty_pi.output_ready() == bs4_pi.output_ready()
    assert rusty_pi.PREFIX == bs4_pi.PREFIX
    assert rusty_pi.SUFFIX == bs4_pi.SUFFIX

    assert [(type(node).__name__, str(node)) for node in rusty.find_all(string=True)] == [
        (type(node).__name__, str(node)) for node in bs4.find_all(string=True)
    ]
    assert list(rusty.strings) == list(bs4.strings)
    assert rusty.get_text("|") == bs4.get_text("|")
    assert rusty.get_text("|", types=None) == bs4.get_text("|", types=None)
    assert rusty.get_text("|", types=(ProcessingInstruction,)) == bs4.get_text(
        "|", types=(Bs4ProcessingInstruction,)
    )
