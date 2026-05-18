from rustysoup import Soup


def test_text_property_concatenates_descendant_text():
    soup = Soup("<div>Hello <span>there</span><span> friend</span></div>")

    assert soup.find("div").text == "Hello there friend"


def test_get_text_separator_and_strip():
    soup = Soup("<div>\n  Hello <span>there</span>\n  <span>friend</span>\n</div>")
    div = soup.find("div")

    assert div.get_text(strip=True) == "Hellotherefriend"
    assert div.get_text(" ", strip=True) == "Hello there friend"


def test_soup_text():
    soup = Soup("<html><head><title>T</title></head><body><p>A</p><p>B</p></body></html>")

    assert soup.get_text("", strip=True) == "TAB"
    assert soup.get_text("|", strip=True) == "T|A|B"
