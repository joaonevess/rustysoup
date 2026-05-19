from __future__ import annotations

import ast
import sys
from pathlib import Path

import pytest

import rustysoup
from rustysoup import BeautifulSoup, Soup


def test_public_import_surface_exports_expected_names():
    expected = {
        "BeautifulSoup",
        "Soup",
        "Tag",
        "NavigableString",
        "ResultSet",
        "SoupStrainer",
        "FeatureNotFound",
        "__version__",
    }

    assert expected.issubset(set(rustysoup.__all__))
    for name in expected:
        assert hasattr(rustysoup, name)

    assert BeautifulSoup is Soup
    assert isinstance(rustysoup.__version__, str)
    assert rustysoup.__version__


def test_public_stub_files_parse():
    package_dir = Path(rustysoup.__file__).parent
    stub_files = sorted(package_dir.glob("*.pyi"))

    assert stub_files
    for stub_file in stub_files:
        ast.parse(stub_file.read_text(encoding="utf-8"), filename=str(stub_file))


def test_invalid_selector_error_message_is_clear():
    soup = BeautifulSoup("<main><a href='/x'>x</a></main>")

    with pytest.raises(ValueError) as exc_info:
        soup.select("main a[href")

    message = str(exc_info.value)
    assert "Invalid CSS selector" in message
    assert "main a[href" in message


def test_special_string_conversion_reuses_cached_wrapper_class(monkeypatch):
    first = BeautifulSoup("<script>s</script><style>c</style>", "html.parser")
    wrapper_classes = [
        rustysoup.NavigableString,
        rustysoup.Comment,
        rustysoup.CData,
        rustysoup.Declaration,
        rustysoup.Doctype,
        rustysoup.ProcessingInstruction,
        rustysoup.TemplateString,
    ]

    for cls in wrapper_classes:
        assert isinstance(first.new_string("one", cls), cls)
    assert isinstance(first.script.string, rustysoup.Script)
    assert isinstance(first.style.string, rustysoup.Stylesheet)

    monkeypatch.setitem(sys.modules, "rustysoup", object())

    second = BeautifulSoup("<script>s</script><style>c</style>", "html.parser")
    for cls in wrapper_classes:
        assert isinstance(second.new_string("two", cls), cls)
    assert isinstance(second.script.string, rustysoup.Script)
    assert isinstance(second.style.string, rustysoup.Stylesheet)
