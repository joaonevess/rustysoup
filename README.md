# rustysoup

[![CI](https://github.com/joaonevess/rustysoup/actions/workflows/ci.yml/badge.svg)](https://github.com/joaonevess/rustysoup/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/rustysoup.svg)](https://pypi.org/project/rustysoup/)
[![Python](https://img.shields.io/pypi/pyversions/rustysoup.svg)](https://pypi.org/project/rustysoup/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

BeautifulSoup ergonomics. Rust-speed HTML extraction.

`rustysoup` is for scrapers, crawlers, and data pipelines that like BeautifulSoup's API but need much higher throughput. Keep the workflow you know: parse HTML, find tags, select with CSS, read attributes, and extract text.

```python
from rustysoup import BeautifulSoup

soup = BeautifulSoup(html)

products = [
    {
        "name": card.select_one("a").get_text(strip=True),
        "url": card.select_one("a")["href"],
        "price": card.select_one(".price").get_text(strip=True),
    }
    for card in soup.select(".product")
]
```

## Install

```bash
pip install rustysoup
```

Requires Python 3.10+.

## Why rustysoup

- Use BeautifulSoup-style code when BeautifulSoup is the bottleneck.
- Move existing scrapers over with a small import change.
- Rust parser core built for high-throughput extraction.
- Fast CSS selectors, traversal, attributes, and text extraction.
- Wheels for Python 3.10+ on Linux, macOS, and Windows.

## Usage

```python
from rustysoup import BeautifulSoup

html = """
<html>
  <head><title>Catalog</title></head>
  <body>
    <div class="product" data-id="1">
      <a href="/products/1">Coffee grinder</a>
      <span class="price">$39</span>
    </div>
  </body>
</html>
"""

soup = BeautifulSoup(html)

assert soup.title.text == "Catalog"
assert soup.find("div", class_="product").get("data-id") == "1"
assert soup.find("a")["href"] == "/products/1"
assert soup.select("div.product a[href]")[0].get_text(strip=True) == "Coffee grinder"
assert soup.find("missing") is None
```

For new code, `Soup` is the same parser with a shorter name:

```python
from rustysoup import Soup

soup = Soup("<p>Hello <strong>Rust</strong></p>")
print(soup.find("p").get_text(" ", strip=True))
```

## BeautifulSoup Migration

Most extraction code can start with a single import change:

```python
# Before
from bs4 import BeautifulSoup

# After
from rustysoup import BeautifulSoup
```

Common constructor forms are supported:

```python
BeautifulSoup(html)
BeautifulSoup(html, "html.parser")
BeautifulSoup(html, "lxml")
BeautifulSoup(html, features="html.parser")
```

Parser names such as `"html.parser"` and `"lxml"` are accepted as compatibility hints and routed through the Rust engine.

Common APIs:

```python
soup.find("a")
soup.find("div", class_="product")
soup.find_all("a", href=True)
soup.select("div.product a[href]")
soup.get_text(" ", strip=True)
```

## Benchmarks

`rustysoup` is built for high-throughput extraction without forcing users into a low-level parser API.

Measured on `104` public HTML pages, `19.83 MiB` total, release build, CPython `3.14.5`, macOS arm64. Lower is better.

| Parser | Parse only | Page summary | Link graph | Selector-heavy |
|---|---:|---:|---:|---:|
| rustysoup | `65.47 ms` | `92.36 ms` | `292.60 ms` | `93.14 ms` |
| selectolax Lexbor | `168.72 ms` | `223.97 ms` | `415.35 ms` | `246.28 ms` |
| selectolax Modest | `213.64 ms` | `275.98 ms` | `458.94 ms` | `286.66 ms` |
| lxml.html | `268.88 ms` | `345.77 ms` | `526.00 ms` | `517.36 ms` |
| html5_parser | `380.05 ms` | `453.78 ms` | `632.53 ms` | `624.89 ms` |
| BeautifulSoup `lxml` | `1773.66 ms` | `1958.49 ms` | `2105.44 ms` | `4197.38 ms` |
| BeautifulSoup `html.parser` | `2342.20 ms` | `2529.23 ms` | `2676.43 ms` | `4755.78 ms` |

## Architecture

Powered by `html5ever` and Servo `selectors`.

The Python API runs on a custom Rust arena DOM optimized for parsing, traversal, selector matching, and Python object creation.

## Development

```bash
maturin develop
pytest
```

Recommended local hook:

```bash
git config core.hooksPath .githooks
```

CI checks:

```bash
./ci/check.sh rust
./ci/check.sh all
```

## Status

`rustysoup` is early, active, and designed for production-style extraction workloads. If you find a parser difference, missing BeautifulSoup API, or performance regression, please open an issue with a small HTML sample and expected output.

---

For benchmark methodology and corpus collection, see [docs/benchmarks.md](docs/benchmarks.md) and [docs/corpus.md](docs/corpus.md).
