from __future__ import annotations

"""Professional parser benchmark harness for rustysoup.

The benchmark separates cold parse+query workloads from hot query-only
workloads, records reproducibility metadata, and checks comparable workloads
with deterministic checksums. It is intentionally honest: use real corpora for
publishable claims, and treat the synthetic corpus as a stable smoke/workload
shape rather than proof of universal speed.
"""

import argparse
import gc
import hashlib
import importlib.metadata
import json
import os
import platform
import resource
import shlex
import statistics
import subprocess
import sys
import time
import zlib
from concurrent.futures import ThreadPoolExecutor
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Iterable
from urllib import parse as urlparse


try:
    from rustysoup import Soup
except ImportError:  # pragma: no cover - benchmark environment issue
    Soup = None

try:
    from bs4 import BeautifulSoup as Bs4BeautifulSoup
    from bs4 import FeatureNotFound as Bs4FeatureNotFound
except ImportError:  # pragma: no cover - optional comparison parser
    Bs4BeautifulSoup = None
    Bs4FeatureNotFound = Exception

try:
    from lxml import html as lxml_html
except ImportError:  # pragma: no cover - optional comparison parser
    lxml_html = None

try:
    import html5_parser

    HTML5_PARSER_IMPORT_ERROR = ""
except Exception as exc:  # pragma: no cover - optional comparison parser
    html5_parser = None
    HTML5_PARSER_IMPORT_ERROR = str(exc)

try:
    from selectolax.lexbor import LexborHTMLParser
except ImportError:  # pragma: no cover - optional comparison parser
    LexborHTMLParser = None

try:
    from selectolax.parser import HTMLParser as ModestHTMLParser
except ImportError:  # pragma: no cover - optional comparison parser
    ModestHTMLParser = None


PreparedRun = Callable[[], int]
OperationFactory = Callable[[list["CorpusDocument"], argparse.Namespace], "PreparedOperation"]
ParseOne = Callable[[str], Any]
QueryChecksum = Callable[[Any, "CorpusDocument"], int]

MASK_64 = (1 << 64) - 1
CHECKSUM_SEP = b"\x00"


@dataclass(frozen=True)
class OperationSpec:
    name: str
    group: str
    description: str
    comparable_checksum: bool = True
    categories: tuple[str, ...] = ()


@dataclass(frozen=True)
class CorpusDocument:
    html: str
    category: str = "uncategorized"
    filename: str | None = None
    url: str | None = None
    final_url: str | None = None

    @property
    def byte_length(self) -> int:
        return len(self.html.encode("utf-8", errors="surrogatepass"))


@dataclass(frozen=True)
class PreparedOperation:
    run: PreparedRun
    document_count: int
    total_bytes: int
    categories: tuple[str, ...] = ()


OPERATION_SPECS: dict[str, OperationSpec] = {
    "parse": OperationSpec(
        "parse",
        "parse",
        "Parse every document and discard the resulting tree.",
    ),
    "parse_hold": OperationSpec(
        "parse_hold",
        "parse",
        "Parse every document and retain all resulting trees until the operation completes.",
    ),
    "parse_threaded": OperationSpec(
        "parse_threaded",
        "parse",
        "Parse every document through a ThreadPoolExecutor using --threads workers.",
    ),
    "parse_select_links": OperationSpec(
        "parse_select_links",
        "parse+query",
        "Parse each document, select a[href], and checksum href values.",
    ),
    "parse_select_product_links": OperationSpec(
        "parse_select_product_links",
        "parse+query",
        "Parse each document, select div.product a[href], and checksum href values.",
    ),
    "parse_find_divs": OperationSpec(
        "parse_find_divs",
        "parse+query",
        "Parse each document and count div elements.",
    ),
    "parse_extract_text": OperationSpec(
        "parse_extract_text",
        "parse+query",
        "Parse each document and extract visible text.",
        comparable_checksum=False,
    ),
    "page_summary": OperationSpec(
        "page_summary",
        "parse+query",
        "Parse each document and extract title, links, script count, and description meta.",
    ),
    "link_graph": OperationSpec(
        "link_graph",
        "parse+query",
        "Parse each document, extract links, and classify internal, external, relative, and fragment URLs.",
    ),
    "article_extract": OperationSpec(
        "article_extract",
        "parse+query",
        "Parse article-like pages and extract title, headings, paragraphs, and article/main links.",
        comparable_checksum=False,
        categories=("news_article", "blog_post", "docs_article"),
    ),
    "product_extract": OperationSpec(
        "product_extract",
        "parse+query",
        "Parse ecommerce pages and extract product-ish cards, links, prices, images, and text.",
        comparable_checksum=False,
        categories=("ecommerce_category", "ecommerce_product"),
    ),
    "table_extract": OperationSpec(
        "table_extract",
        "parse+query",
        "Parse table-heavy pages and extract table, row, cell, and header structure.",
        categories=("table_heavy",),
    ),
    "form_extract": OperationSpec(
        "form_extract",
        "parse+query",
        "Parse form-heavy pages and extract controls, labels, names, types, and boolean attributes.",
        categories=("form_heavy",),
    ),
    "parse_materialize_links": OperationSpec(
        "parse_materialize_links",
        "parse+query",
        "Parse each document, materialize link nodes into Python objects, and access names, attrs, hrefs, and text.",
        comparable_checksum=False,
    ),
    "parse_selector_stress": OperationSpec(
        "parse_selector_stress",
        "parse+query",
        "Parse each document and run a set of common selectors used by crawler/scraper workloads.",
    ),
    "reuse_select_links": OperationSpec(
        "reuse_select_links",
        "query-only",
        "Preparse documents once, then repeatedly select a[href] and checksum href values.",
    ),
    "reuse_select_product_links": OperationSpec(
        "reuse_select_product_links",
        "query-only",
        "Preparse documents once, then repeatedly select div.product a[href].",
    ),
    "reuse_find_divs": OperationSpec(
        "reuse_find_divs",
        "query-only",
        "Preparse documents once, then repeatedly count div elements.",
    ),
    "reuse_extract_text": OperationSpec(
        "reuse_extract_text",
        "query-only",
        "Preparse documents once, then repeatedly extract visible text.",
        comparable_checksum=False,
    ),
    "reuse_materialize_links": OperationSpec(
        "reuse_materialize_links",
        "query-only",
        "Preparse documents once, then repeatedly materialize link nodes and access Python-facing properties.",
        comparable_checksum=False,
    ),
    "reuse_selector_stress": OperationSpec(
        "reuse_selector_stress",
        "query-only",
        "Preparse documents once, then repeatedly run common selectors.",
    ),
}

BENCHMARK_SUITES = {
    "smoke": [
        "parse",
        "page_summary",
        "link_graph",
        "parse_selector_stress",
    ],
    "standard": [
        "parse",
        "page_summary",
        "link_graph",
        "article_extract",
        "product_extract",
        "table_extract",
        "form_extract",
        "parse_materialize_links",
        "parse_selector_stress",
        "reuse_selector_stress",
    ],
    "full": list(OPERATION_SPECS),
}

DEFAULT_OPERATION_NAMES = BENCHMARK_SUITES["standard"]

OPERATION_ALIASES = {
    "select_links": "parse_select_links",
    "select_product_links": "parse_select_product_links",
    "find_divs": "parse_find_divs",
    "extract_text": "parse_extract_text",
    "materialize_links": "parse_materialize_links",
    "selector_stress": "parse_selector_stress",
}

@dataclass(frozen=True)
class Candidate:
    name: str
    available: bool
    reason: str
    parse_one: ParseOne | None
    operations: dict[str, OperationFactory]


@dataclass(frozen=True)
class TimedResult:
    parser: str
    operation: str
    group: str
    document_count: int
    total_bytes: int
    iterations: int
    mean_ms: float
    median_ms: float
    best_ms: float
    p95_ms: float
    worst_ms: float
    stdev_ms: float
    rsd_pct: float
    ops_per_sec: float
    docs_per_sec: float
    mib_per_sec: float
    checksum: int
    samples_ms: list[float]
    categories: tuple[str, ...] = ()
    memory_mb: float | None = None


def make_document(index: int, items_per_doc: int) -> str:
    cards = []
    for item_index in range(items_per_doc):
        product_id = index * items_per_doc + item_index
        cards.append(
            f"""
            <div class="product {'featured' if item_index % 11 == 0 else ''}" data-id="{product_id}">
              <h2><a href="/products/{product_id}?ref=bench">Product {product_id}</a></h2>
              <p class="description">Fast parser benchmark item {product_id} with nested text.</p>
              <span class="price">${product_id % 199}.99</span>
              <div class="metadata">
                <a href="/brands/{product_id % 17}">Brand {product_id % 17}</a>
                <a href="/reviews/{product_id}">Reviews</a>
              </div>
              <ul class="tags">
                <li>html</li><li>parser</li><li>benchmark</li>
              </ul>
            </div>
            """
        )
    return f"""<!doctype html>
<html>
  <head>
    <title>Benchmark Page {index}</title>
    <meta name="description" content="Synthetic benchmark page {index}">
    <meta property="og:title" content="Benchmark Page {index}">
    <script>window.__bench = {{"page": {index}, "items": {items_per_doc}}};</script>
  </head>
  <body>
    <main id="main" class="catalog">
      <nav><a href="/">Home</a><a href="/products">Products</a></nav>
      {''.join(cards)}
    </main>
    <footer><a href="/privacy">Privacy</a></footer>
  </body>
</html>"""


def make_synthetic_corpus(documents: int, items_per_doc: int) -> list[CorpusDocument]:
    return [
        CorpusDocument(
            html=make_document(index, items_per_doc),
            category="ecommerce_category",
            filename=f"synthetic-{index:05d}.html",
            url=f"synthetic://catalog/{index}",
        )
        for index in range(documents)
    ]


def load_manifest_entries(root: Path) -> dict[str, dict[str, Any]]:
    manifest_path = root / "manifest.json"
    if not manifest_path.is_file():
        return {}
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SystemExit(f"invalid corpus manifest {manifest_path}: {exc}") from exc
    entries: dict[str, dict[str, Any]] = {}
    for entry in manifest.get("entries", []):
        filename = entry.get("filename")
        if filename:
            entries[str(filename)] = entry
    return entries


def load_file_corpus(input_dir: str, max_files: int | None) -> list[CorpusDocument]:
    root = Path(input_dir)
    if not root.is_dir():
        raise SystemExit(f"--input-dir is not a directory: {input_dir}")
    suffixes = {".html", ".htm", ".xhtml", ".txt"}
    paths = sorted(path for path in root.rglob("*") if path.suffix.lower() in suffixes)
    if max_files is not None:
        paths = paths[:max_files]
    if not paths:
        raise SystemExit(f"--input-dir contains no HTML-like files: {input_dir}")
    manifest_entries = load_manifest_entries(root)
    documents: list[CorpusDocument] = []
    for path in paths:
        entry = manifest_entries.get(path.name, {})
        documents.append(
            CorpusDocument(
                html=path.read_text(encoding="utf-8", errors="replace"),
                category=str(entry.get("category") or "uncategorized"),
                filename=str(path.relative_to(root)),
                url=entry.get("url"),
                final_url=entry.get("final_url"),
            )
        )
    return documents


def load_corpus(args: argparse.Namespace) -> list[CorpusDocument]:
    if args.input_dir:
        return load_file_corpus(args.input_dir, args.max_files)
    return make_synthetic_corpus(args.documents, args.items_per_doc)


def combine_checksum(total: int, value: int) -> int:
    return ((total * 1_000_003) ^ (value & MASK_64)) & MASK_64


def checksum_text(value: Any) -> int:
    text = "" if value is None else str(value)
    crc = zlib.crc32(text.encode("utf-8", errors="surrogatepass")) & 0xFFFFFFFF
    return ((len(text) << 32) ^ crc) & MASK_64


def checksum_strings(values: Iterable[Any]) -> int:
    total = 0
    count = 0
    for value in values:
        text = "" if value is None else str(value)
        crc = zlib.crc32(text.encode("utf-8", errors="surrogatepass"))
        crc = zlib.crc32(CHECKSUM_SEP, crc) & 0xFFFFFFFF
        total = combine_checksum(total, ((len(text) << 32) ^ crc) & MASK_64)
        count += 1
    return combine_checksum(total, count)


def checksum_count(count: int) -> int:
    return combine_checksum(0, count)


def checksum_summary(title: Any, links: Iterable[Any], scripts: int, description: Any) -> int:
    total = checksum_text(title)
    total = combine_checksum(total, checksum_strings(links))
    total = combine_checksum(total, checksum_count(scripts))
    return combine_checksum(total, checksum_text(description))


def checksum_mapping(values: Iterable[tuple[Any, Any]]) -> int:
    return checksum_strings(f"{key}={value}" for key, value in values)


def normalize_text(value: Any) -> str:
    return " ".join(str(value or "").split())


def doc_base_url(doc: CorpusDocument) -> str:
    return doc.final_url or doc.url or ""


def classify_links(hrefs: Iterable[Any], doc: CorpusDocument) -> int:
    base = doc_base_url(doc)
    base_host = urlparse.urlparse(base).netloc.lower()
    counts = {
        "total": 0,
        "empty": 0,
        "fragment": 0,
        "mailto": 0,
        "javascript": 0,
        "relative": 0,
        "internal": 0,
        "external": 0,
    }
    sample_hosts: list[str] = []
    sample_paths: list[str] = []
    for value in hrefs:
        href = str(value or "").strip()
        counts["total"] += 1
        if not href:
            counts["empty"] += 1
            continue
        lowered = href.lower()
        if href.startswith("#"):
            counts["fragment"] += 1
        if lowered.startswith("mailto:"):
            counts["mailto"] += 1
        if lowered.startswith("javascript:"):
            counts["javascript"] += 1
        parsed_raw = urlparse.urlparse(href)
        if not parsed_raw.scheme and not parsed_raw.netloc:
            counts["relative"] += 1
        absolute = urlparse.urljoin(base, href) if base else href
        parsed = urlparse.urlparse(absolute)
        host = parsed.netloc.lower()
        if host and base_host and host == base_host:
            counts["internal"] += 1
        elif host and base_host:
            counts["external"] += 1
        if host and len(sample_hosts) < 24:
            sample_hosts.append(host)
        if parsed.path and len(sample_paths) < 24:
            sample_paths.append(parsed.path)
    total = checksum_mapping(sorted(counts.items()))
    total = combine_checksum(total, checksum_strings(sample_hosts))
    return combine_checksum(total, checksum_strings(sample_paths))


def checksum_docs(documents: list[CorpusDocument]) -> str:
    digest = hashlib.sha256()
    for doc in documents:
        data = doc.html.encode("utf-8", errors="surrogatepass")
        digest.update(len(data).to_bytes(8, "little"))
        digest.update(data)
    return digest.hexdigest()


def select_operation_docs(
    documents: list[CorpusDocument],
    categories: tuple[str, ...],
) -> list[CorpusDocument]:
    if not categories:
        return documents
    wanted = set(categories)
    return [doc for doc in documents if doc.category in wanted]


def prepare_operation(
    documents: list[CorpusDocument],
    categories: tuple[str, ...],
    run_factory: Callable[[list[CorpusDocument]], PreparedRun],
) -> PreparedOperation:
    selected = select_operation_docs(documents, categories)
    return PreparedOperation(
        run=run_factory(selected),
        document_count=len(selected),
        total_bytes=sum(doc.byte_length for doc in selected),
        categories=categories,
    )


def parse_docs_checksum(documents: list[CorpusDocument], parse_one: ParseOne) -> int:
    total = 0
    for doc in documents:
        parse_one(doc.html)
        total = combine_checksum(total, 1)
    return total


def parse_docs_hold_checksum(documents: list[CorpusDocument], parse_one: ParseOne) -> int:
    trees = [parse_one(doc.html) for doc in documents]
    return checksum_count(len(trees))


def parse_docs_threaded_checksum(
    documents: list[CorpusDocument],
    parse_one: ParseOne,
    threads: int,
) -> int:
    total = 0
    with ThreadPoolExecutor(max_workers=threads) as executor:
        for _ in executor.map(parse_one, (doc.html for doc in documents)):
            total = combine_checksum(total, 1)
    return total


def query_docs_checksum(
    documents: list[CorpusDocument],
    parse_one: ParseOne,
    query_one: QueryChecksum,
) -> int:
    total = 0
    for doc in documents:
        total = combine_checksum(total, query_one(parse_one(doc.html), doc))
    return total


def query_trees_checksum(trees: list[tuple[Any, CorpusDocument]], query_one: QueryChecksum) -> int:
    total = 0
    for tree, doc in trees:
        total = combine_checksum(total, query_one(tree, doc))
    return total


def parse_factory(parse_one: ParseOne, categories: tuple[str, ...] = ()) -> OperationFactory:
    def factory(documents: list[CorpusDocument], args: argparse.Namespace) -> PreparedOperation:
        return prepare_operation(
            documents,
            categories,
            lambda selected: lambda: parse_docs_checksum(selected, parse_one),
        )

    return factory


def parse_hold_factory(parse_one: ParseOne, categories: tuple[str, ...] = ()) -> OperationFactory:
    def factory(documents: list[CorpusDocument], args: argparse.Namespace) -> PreparedOperation:
        return prepare_operation(
            documents,
            categories,
            lambda selected: lambda: parse_docs_hold_checksum(selected, parse_one),
        )

    return factory


def parse_threaded_factory(parse_one: ParseOne, categories: tuple[str, ...] = ()) -> OperationFactory:
    def factory(documents: list[CorpusDocument], args: argparse.Namespace) -> PreparedOperation:
        return prepare_operation(
            documents,
            categories,
            lambda selected: lambda: parse_docs_threaded_checksum(selected, parse_one, args.threads),
        )

    return factory


def cold_query_factory(
    parse_one: ParseOne,
    query_one: QueryChecksum,
    categories: tuple[str, ...] = (),
) -> OperationFactory:
    def factory(documents: list[CorpusDocument], args: argparse.Namespace) -> PreparedOperation:
        return prepare_operation(
            documents,
            categories,
            lambda selected: lambda: query_docs_checksum(selected, parse_one, query_one),
        )

    return factory


def hot_query_factory(
    parse_one: ParseOne,
    query_one: QueryChecksum,
    categories: tuple[str, ...] = (),
) -> OperationFactory:
    def factory(documents: list[CorpusDocument], args: argparse.Namespace) -> PreparedOperation:
        selected = select_operation_docs(documents, categories)
        trees = [(parse_one(doc.html), doc) for doc in selected]
        return PreparedOperation(
            run=lambda: query_trees_checksum(trees, query_one),
            document_count=len(selected),
            total_bytes=sum(doc.byte_length for doc in selected),
            categories=categories,
        )

    return factory


def common_operations(parse_one: ParseOne, queries: dict[str, QueryChecksum]) -> dict[str, OperationFactory]:
    def categories(operation_name: str) -> tuple[str, ...]:
        return OPERATION_SPECS[operation_name].categories

    return {
        "parse": parse_factory(parse_one),
        "parse_hold": parse_hold_factory(parse_one),
        "parse_threaded": parse_threaded_factory(parse_one),
        "parse_select_links": cold_query_factory(parse_one, queries["links"]),
        "parse_select_product_links": cold_query_factory(parse_one, queries["product_links"]),
        "parse_find_divs": cold_query_factory(parse_one, queries["divs"]),
        "parse_extract_text": cold_query_factory(parse_one, queries["text"]),
        "page_summary": cold_query_factory(parse_one, queries["summary"]),
        "link_graph": cold_query_factory(parse_one, queries["link_graph"]),
        "article_extract": cold_query_factory(parse_one, queries["article"], categories("article_extract")),
        "product_extract": cold_query_factory(parse_one, queries["product"], categories("product_extract")),
        "table_extract": cold_query_factory(parse_one, queries["table"], categories("table_extract")),
        "form_extract": cold_query_factory(parse_one, queries["form"], categories("form_extract")),
        "parse_materialize_links": cold_query_factory(parse_one, queries["materialize_links"]),
        "parse_selector_stress": cold_query_factory(parse_one, queries["selector_stress"]),
        "reuse_select_links": hot_query_factory(parse_one, queries["links"]),
        "reuse_select_product_links": hot_query_factory(parse_one, queries["product_links"]),
        "reuse_find_divs": hot_query_factory(parse_one, queries["divs"]),
        "reuse_extract_text": hot_query_factory(parse_one, queries["text"]),
        "reuse_materialize_links": hot_query_factory(parse_one, queries["materialize_links"]),
        "reuse_selector_stress": hot_query_factory(parse_one, queries["selector_stress"]),
    }


def rustysoup_candidate() -> Candidate:
    if Soup is None:
        return Candidate("rustysoup", False, "rustysoup is not installed", None, {})

    def parse(html: str) -> Any:
        return Soup(html)

    return Candidate(
        "rustysoup",
        True,
        "",
        parse,
        common_operations(
            parse,
            {
                "links": lambda tree, doc: checksum_strings(
                    tag.get("href", "") for tag in tree.select("a[href]")
                ),
                "product_links": lambda tree, doc: checksum_strings(
                    tag.get("href", "") for tag in tree.select("div.product a[href]")
                ),
                "divs": lambda tree, doc: checksum_count(len(tree.find_all("div"))),
                "text": lambda tree, doc: checksum_text(tree.get_text(" ", strip=True)),
                "summary": rustysoup_summary_checksum,
                "link_graph": rustysoup_link_graph_checksum,
                "article": rustysoup_article_checksum,
                "product": rustysoup_product_checksum,
                "table": rustysoup_table_checksum,
                "form": rustysoup_form_checksum,
                "materialize_links": rustysoup_materialize_links_checksum,
                "selector_stress": rustysoup_selector_stress_checksum,
            },
        ),
    )


def rustysoup_summary_checksum(tree: Any, doc: CorpusDocument) -> int:
    title = tree.find("title")
    meta = tree.find("meta", attrs={"name": "description"})
    return checksum_summary(
        title.text if title is not None else "",
        (tag.get("href", "") for tag in tree.find_all("a", href=True)),
        len(tree.find_all("script")),
        meta.get("content", "") if meta is not None else "",
    )


def rustysoup_link_graph_checksum(tree: Any, doc: CorpusDocument) -> int:
    return classify_links((tag.get("href", "") for tag in tree.find_all("a", href=True)), doc)


def rustysoup_article_checksum(tree: Any, doc: CorpusDocument) -> int:
    title = tree.find("title")
    total = checksum_text(title.text if title is not None else "")
    for name in ("h1", "h2", "h3"):
        total = combine_checksum(total, checksum_strings(tag.get_text(" ", strip=True) for tag in tree.find_all(name)))
    total = combine_checksum(total, checksum_strings(tag.get_text(" ", strip=True) for tag in tree.find_all("p")))
    article_links = list(tree.select("article a[href]")) + list(tree.select("main a[href]"))
    return combine_checksum(total, checksum_strings(tag.get("href", "") for tag in article_links))


def rustysoup_product_checksum(tree: Any, doc: CorpusDocument) -> int:
    selectors = ("article.product_pod", ".product", "li.product", ".product-card", ".product-item")
    total = checksum_count(0)
    for selector in selectors:
        for node in tree.select(selector):
            total = combine_checksum(total, checksum_text(node.get_text(" ", strip=True)))
            total = combine_checksum(total, checksum_strings(tag.get("href", "") for tag in node.select("a[href]")))
            total = combine_checksum(total, checksum_strings(tag.get("src", "") for tag in node.select("img[src]")))
            prices = list(node.select(".price")) + list(node.select(".price_color")) + list(node.select('[itemprop="price"]'))
            total = combine_checksum(total, checksum_strings(price.get_text(" ", strip=True) for price in prices))
    return total


def rustysoup_table_checksum(tree: Any, doc: CorpusDocument) -> int:
    total = checksum_count(len(tree.find_all("table")))
    total = combine_checksum(total, checksum_count(len(tree.select("table tr"))))
    total = combine_checksum(total, checksum_count(len(tree.select("table th"))))
    cells = list(tree.select("table td"))[:128]
    total = combine_checksum(total, checksum_count(len(tree.select("table td"))))
    return combine_checksum(total, checksum_strings(cell.get_text(" ", strip=True) for cell in cells))


def rustysoup_form_checksum(tree: Any, doc: CorpusDocument) -> int:
    controls = []
    for name in ("input", "select", "textarea", "button"):
        controls.extend(tree.find_all(name))
    total = checksum_count(len(tree.find_all("form")))
    total = combine_checksum(total, checksum_strings(label.get_text(" ", strip=True) for label in tree.find_all("label")))
    total = combine_checksum(
        total,
        checksum_strings(
            f"{control.name}:{control.get('name', '')}:{control.get('type', '')}:{control.has_attr('disabled')}:{control.has_attr('checked')}"
            for control in controls
        ),
    )
    return total


def rustysoup_materialize_links_checksum(tree: Any, doc: CorpusDocument) -> int:
    total = 0
    for tag in tree.find_all("a", href=True):
        total = combine_checksum(total, checksum_text(tag.name))
        total = combine_checksum(total, checksum_count(len(tag.attrs)))
        total = combine_checksum(total, checksum_text(tag.get("href", "")))
        total = combine_checksum(total, checksum_text(tag.get_text(" ", strip=True)))
    return total


def rustysoup_selector_stress_checksum(tree: Any, doc: CorpusDocument) -> int:
    selectors = (
        "a[href]",
        "div",
        "table tr",
        "form input",
        'meta[name="description"]',
        "main p",
        ".product",
        "script",
        "link[href]",
        "[data-id]",
    )
    return checksum_mapping((selector, len(tree.select(selector))) for selector in selectors)


def bs4_candidate(parser: str) -> Candidate:
    name = f"beautifulsoup4:{parser}"
    if Bs4BeautifulSoup is None:
        return Candidate(name, False, "beautifulsoup4 is not installed", None, {})
    try:
        Bs4BeautifulSoup("<p>probe</p>", parser)
    except Bs4FeatureNotFound:
        return Candidate(name, False, f"bs4 parser {parser!r} is not available", None, {})

    def parse(html: str) -> Any:
        return Bs4BeautifulSoup(html, parser)

    return Candidate(
        name,
        True,
        "",
        parse,
        common_operations(
            parse,
            {
                "links": lambda tree, doc: checksum_strings(
                    tag.get("href", "") for tag in tree.select("a[href]")
                ),
                "product_links": lambda tree, doc: checksum_strings(
                    tag.get("href", "") for tag in tree.select("div.product a[href]")
                ),
                "divs": lambda tree, doc: checksum_count(len(tree.find_all("div"))),
                "text": lambda tree, doc: checksum_text(tree.get_text(" ", strip=True)),
                "summary": bs4_summary_checksum,
                "link_graph": rustysoup_link_graph_checksum,
                "article": rustysoup_article_checksum,
                "product": rustysoup_product_checksum,
                "table": rustysoup_table_checksum,
                "form": rustysoup_form_checksum,
                "materialize_links": rustysoup_materialize_links_checksum,
                "selector_stress": rustysoup_selector_stress_checksum,
            },
        ),
    )


def bs4_summary_checksum(tree: Any, doc: CorpusDocument) -> int:
    title = tree.find("title")
    meta = tree.find("meta", attrs={"name": "description"})
    return checksum_summary(
        title.text if title is not None else "",
        (tag.get("href", "") for tag in tree.find_all("a", href=True)),
        len(tree.find_all("script")),
        meta.get("content", "") if meta is not None else "",
    )


LXML_PRODUCT_LINKS_XPATH = (
    ".//div[contains(concat(' ', normalize-space(@class), ' '), ' product ')]//a[@href]"
)
LXML_PRODUCT_HREFS_XPATH = f"{LXML_PRODUCT_LINKS_XPATH}/@href"
LXML_VISIBLE_TEXT_XPATH = ".//text()[not(ancestor::script) and not(ancestor::style)]"
LXML_PRODUCT_NODE_XPATH = (
    ".//*[contains(concat(' ', normalize-space(@class), ' '), ' product ') "
    "or contains(concat(' ', normalize-space(@class), ' '), ' product_pod ') "
    "or contains(concat(' ', normalize-space(@class), ' '), ' product-card ') "
    "or contains(concat(' ', normalize-space(@class), ' '), ' product-item ')]"
)


def lxml_candidate() -> Candidate:
    if lxml_html is None:
        return Candidate("lxml.html", False, "lxml is not installed", None, {})
    parse = lxml_html.fromstring
    return Candidate(
        "lxml.html",
        True,
        "",
        parse,
        common_operations(
            parse,
            {
                "links": lambda tree, doc: checksum_strings(tree.xpath(".//a[@href]/@href")),
                "product_links": lambda tree, doc: checksum_strings(tree.xpath(LXML_PRODUCT_HREFS_XPATH)),
                "divs": lambda tree, doc: checksum_count(len(tree.xpath(".//div"))),
                "text": lambda tree, doc: checksum_text(lxml_visible_text(tree)),
                "summary": lxml_summary_checksum,
                "link_graph": lxml_link_graph_checksum,
                "article": lxml_article_checksum,
                "product": lxml_product_checksum,
                "table": lxml_table_checksum,
                "form": lxml_form_checksum,
                "materialize_links": lxml_materialize_links_checksum,
                "selector_stress": lxml_selector_stress_checksum,
            },
        ),
    )


def html5_parser_candidate() -> Candidate:
    if html5_parser is None:
        reason = html5_parser_unavailable_reason()
        return Candidate("html5_parser", False, reason, None, {})

    def parse(html: str) -> Any:
        try:
            return html5_parser.parse(html, namespace_elements=False)
        except TypeError:
            return html5_parser.parse(html)

    return Candidate(
        "html5_parser",
        True,
        "",
        parse,
        common_operations(
            parse,
            {
                "links": lambda tree, doc: checksum_strings(tree.xpath(".//a[@href]/@href")),
                "product_links": lambda tree, doc: checksum_strings(tree.xpath(LXML_PRODUCT_HREFS_XPATH)),
                "divs": lambda tree, doc: checksum_count(len(tree.xpath(".//div"))),
                "text": lambda tree, doc: checksum_text(lxml_visible_text(tree)),
                "summary": lxml_summary_checksum,
                "link_graph": lxml_link_graph_checksum,
                "article": lxml_article_checksum,
                "product": lxml_product_checksum,
                "table": lxml_table_checksum,
                "form": lxml_form_checksum,
                "materialize_links": lxml_materialize_links_checksum,
                "selector_stress": lxml_selector_stress_checksum,
            },
        ),
    )


def html5_parser_unavailable_reason() -> str:
    if not HTML5_PARSER_IMPORT_ERROR:
        return "html5_parser is not installed"
    if "libxml2" in HTML5_PARSER_IMPORT_ERROR:
        return (
            "local html5_parser and lxml builds are not binary-compatible; "
            "rebuild lxml from source with: python -m pip install "
            "--force-reinstall --no-binary lxml lxml"
        )
    return HTML5_PARSER_IMPORT_ERROR


def lxml_visible_text(tree: Any) -> str:
    return " ".join(
        text.strip()
        for text in tree.xpath(LXML_VISIBLE_TEXT_XPATH)
        if str(text).strip()
    )


def lxml_text_content(node: Any) -> str:
    text_content = getattr(node, "text_content", None)
    if callable(text_content):
        return text_content()
    itertext = getattr(node, "itertext", None)
    if callable(itertext):
        return "".join(itertext())
    return ""


def lxml_summary_checksum(tree: Any, doc: CorpusDocument) -> int:
    return checksum_summary(
        tree.xpath("string(.//title[1])"),
        tree.xpath(".//a[@href]/@href"),
        len(tree.xpath(".//script")),
        tree.xpath("string(.//meta[@name='description']/@content)"),
    )


def lxml_link_graph_checksum(tree: Any, doc: CorpusDocument) -> int:
    return classify_links(tree.xpath(".//a[@href]/@href"), doc)


def lxml_article_checksum(tree: Any, doc: CorpusDocument) -> int:
    total = checksum_text(tree.xpath("string(.//title[1])"))
    total = combine_checksum(total, checksum_strings(normalize_text(text) for text in tree.xpath(".//h1//text()")))
    total = combine_checksum(total, checksum_strings(normalize_text(text) for text in tree.xpath(".//h2//text()")))
    total = combine_checksum(total, checksum_strings(normalize_text(text) for text in tree.xpath(".//h3//text()")))
    total = combine_checksum(total, checksum_strings(normalize_text(lxml_text_content(node)) for node in tree.xpath(".//p")))
    return combine_checksum(total, checksum_strings(tree.xpath(".//article//a[@href]/@href | .//main//a[@href]/@href")))


def lxml_product_checksum(tree: Any, doc: CorpusDocument) -> int:
    total = checksum_count(0)
    for node in tree.xpath(LXML_PRODUCT_NODE_XPATH):
        total = combine_checksum(total, checksum_text(normalize_text(lxml_text_content(node))))
        total = combine_checksum(total, checksum_strings(node.xpath(".//a[@href]/@href")))
        total = combine_checksum(total, checksum_strings(node.xpath(".//img[@src]/@src")))
        total = combine_checksum(
            total,
            checksum_strings(
                normalize_text(lxml_text_content(price))
                for price in node.xpath(
                    ".//*[contains(concat(' ', normalize-space(@class), ' '), ' price ') "
                    "or contains(concat(' ', normalize-space(@class), ' '), ' price_color ') "
                    "or @itemprop='price']"
                )
            ),
        )
    return total


def lxml_table_checksum(tree: Any, doc: CorpusDocument) -> int:
    total = checksum_count(len(tree.xpath(".//table")))
    total = combine_checksum(total, checksum_count(len(tree.xpath(".//table//tr"))))
    total = combine_checksum(total, checksum_count(len(tree.xpath(".//table//th"))))
    cells = tree.xpath(".//table//td")
    total = combine_checksum(total, checksum_count(len(cells)))
    return combine_checksum(total, checksum_strings(normalize_text(lxml_text_content(cell)) for cell in cells[:128]))


def lxml_form_checksum(tree: Any, doc: CorpusDocument) -> int:
    controls = tree.xpath(".//input | .//select | .//textarea | .//button")
    total = checksum_count(len(tree.xpath(".//form")))
    total = combine_checksum(total, checksum_strings(normalize_text(lxml_text_content(label)) for label in tree.xpath(".//label")))
    total = combine_checksum(
        total,
        checksum_strings(
            f"{control.tag}:{control.get('name', '')}:{control.get('type', '')}:{'disabled' in control.attrib}:{'checked' in control.attrib}"
            for control in controls
        ),
    )
    return total


def lxml_materialize_links_checksum(tree: Any, doc: CorpusDocument) -> int:
    total = 0
    for node in tree.xpath(".//a[@href]"):
        total = combine_checksum(total, checksum_text(node.tag))
        total = combine_checksum(total, checksum_count(len(node.attrib)))
        total = combine_checksum(total, checksum_text(node.get("href", "")))
        total = combine_checksum(total, checksum_text(normalize_text(lxml_text_content(node))))
    return total


def lxml_selector_stress_checksum(tree: Any, doc: CorpusDocument) -> int:
    counts = (
        ("a[href]", len(tree.xpath(".//a[@href]"))),
        ("div", len(tree.xpath(".//div"))),
        ("table tr", len(tree.xpath(".//table//tr"))),
        ("form input", len(tree.xpath(".//form//input"))),
        ('meta[name="description"]', len(tree.xpath(".//meta[@name='description']"))),
        ("main p", len(tree.xpath(".//main//p"))),
        (".product", len(tree.xpath(".//*[contains(concat(' ', normalize-space(@class), ' '), ' product ')]"))),
        ("script", len(tree.xpath(".//script"))),
        ("link[href]", len(tree.xpath(".//link[@href]"))),
        ("[data-id]", len(tree.xpath(".//*[@data-id]"))),
    )
    return checksum_mapping(counts)


def selectolax_candidate(name: str, parser_cls: Any) -> Candidate:
    if parser_cls is None:
        return Candidate(name, False, f"{name} is not installed", None, {})

    return Candidate(
        name,
        True,
        "",
        parser_cls,
        common_operations(
            parser_cls,
            {
                "links": lambda tree, doc: checksum_strings(
                    node.attributes.get("href", "") for node in tree.css("a[href]")
                ),
                "product_links": lambda tree, doc: checksum_strings(
                    node.attributes.get("href", "")
                    for node in tree.css("div.product a[href]")
                ),
                "divs": lambda tree, doc: checksum_count(len(tree.css("div"))),
                "text": lambda tree, doc: checksum_text(tree.text(separator=" ", strip=True)),
                "summary": selectolax_summary_checksum,
                "link_graph": selectolax_link_graph_checksum,
                "article": selectolax_article_checksum,
                "product": selectolax_product_checksum,
                "table": selectolax_table_checksum,
                "form": selectolax_form_checksum,
                "materialize_links": selectolax_materialize_links_checksum,
                "selector_stress": selectolax_selector_stress_checksum,
            },
        ),
    )


def selectolax_summary_checksum(tree: Any, doc: CorpusDocument) -> int:
    title = tree.css_first("title")
    meta = tree.css_first('meta[name="description"]')
    return checksum_summary(
        title.text() if title is not None else "",
        (node.attributes.get("href", "") for node in tree.css("a[href]")),
        len(tree.css("script")),
        meta.attributes.get("content", "") if meta is not None else "",
    )


def selectolax_link_graph_checksum(tree: Any, doc: CorpusDocument) -> int:
    return classify_links((node.attributes.get("href", "") for node in tree.css("a[href]")), doc)


def selectolax_article_checksum(tree: Any, doc: CorpusDocument) -> int:
    title = tree.css_first("title")
    total = checksum_text(title.text() if title is not None else "")
    for selector in ("h1", "h2", "h3"):
        total = combine_checksum(total, checksum_strings(node.text(separator=" ", strip=True) for node in tree.css(selector)))
    total = combine_checksum(total, checksum_strings(node.text(separator=" ", strip=True) for node in tree.css("p")))
    links = list(tree.css("article a[href]")) + list(tree.css("main a[href]"))
    return combine_checksum(total, checksum_strings(node.attributes.get("href", "") for node in links))


def selectolax_product_checksum(tree: Any, doc: CorpusDocument) -> int:
    selectors = ("article.product_pod", ".product", "li.product", ".product-card", ".product-item")
    total = checksum_count(0)
    for selector in selectors:
        for node in tree.css(selector):
            total = combine_checksum(total, checksum_text(node.text(separator=" ", strip=True)))
            total = combine_checksum(total, checksum_strings(link.attributes.get("href", "") for link in node.css("a[href]")))
            total = combine_checksum(total, checksum_strings(image.attributes.get("src", "") for image in node.css("img[src]")))
            prices = list(node.css(".price")) + list(node.css(".price_color")) + list(node.css('[itemprop="price"]'))
            total = combine_checksum(total, checksum_strings(price.text(separator=" ", strip=True) for price in prices))
    return total


def selectolax_table_checksum(tree: Any, doc: CorpusDocument) -> int:
    total = checksum_count(len(tree.css("table")))
    total = combine_checksum(total, checksum_count(len(tree.css("table tr"))))
    total = combine_checksum(total, checksum_count(len(tree.css("table th"))))
    cells = tree.css("table td")
    total = combine_checksum(total, checksum_count(len(cells)))
    return combine_checksum(total, checksum_strings(cell.text(separator=" ", strip=True) for cell in cells[:128]))


def selectolax_form_checksum(tree: Any, doc: CorpusDocument) -> int:
    controls = []
    for selector in ("input", "select", "textarea", "button"):
        controls.extend(tree.css(selector))
    total = checksum_count(len(tree.css("form")))
    total = combine_checksum(total, checksum_strings(label.text(separator=" ", strip=True) for label in tree.css("label")))
    total = combine_checksum(
        total,
        checksum_strings(
            f"{control.tag}:{control.attributes.get('name', '')}:{control.attributes.get('type', '')}:{'disabled' in control.attributes}:{'checked' in control.attributes}"
            for control in controls
        ),
    )
    return total


def selectolax_materialize_links_checksum(tree: Any, doc: CorpusDocument) -> int:
    total = 0
    for node in tree.css("a[href]"):
        total = combine_checksum(total, checksum_text(node.tag))
        total = combine_checksum(total, checksum_count(len(node.attributes)))
        total = combine_checksum(total, checksum_text(node.attributes.get("href", "")))
        total = combine_checksum(total, checksum_text(node.text(separator=" ", strip=True)))
    return total


def selectolax_selector_stress_checksum(tree: Any, doc: CorpusDocument) -> int:
    selectors = (
        "a[href]",
        "div",
        "table tr",
        "form input",
        'meta[name="description"]',
        "main p",
        ".product",
        "script",
        "link[href]",
        "[data-id]",
    )
    return checksum_mapping((selector, len(tree.css(selector))) for selector in selectors)


def all_candidates() -> list[Candidate]:
    return [
        rustysoup_candidate(),
        bs4_candidate("html.parser"),
        bs4_candidate("lxml"),
        lxml_candidate(),
        html5_parser_candidate(),
        selectolax_candidate("selectolax:lexbor", LexborHTMLParser),
        selectolax_candidate("selectolax:modest", ModestHTMLParser),
    ]


def candidate_by_name(name: str) -> Candidate | None:
    for candidate in all_candidates():
        if candidate.name == name:
            return candidate
    return None


def select_candidates(names: Iterable[str] | None) -> list[Candidate]:
    candidates = all_candidates()
    if not names:
        return candidates
    wanted = set(names)
    selected = [candidate for candidate in candidates if candidate.name in wanted]
    found = {candidate.name for candidate in selected}
    missing = sorted(wanted - found)
    if missing:
        available = ", ".join(candidate.name for candidate in candidates)
        raise SystemExit(f"unknown parser(s): {', '.join(missing)}. Available: {available}")
    return selected


def candidate_operations(candidate: Candidate) -> dict[str, OperationFactory]:
    return dict(candidate.operations)


def normalize_operation_names(names: list[str] | None, threads: int, suite: str) -> list[str]:
    if not names:
        operations = list(BENCHMARK_SUITES[suite])
        if threads > 1:
            operations.append("parse_threaded")
        return operations

    normalized: list[str] = []
    for name in names:
        if name in BENCHMARK_SUITES:
            for operation_name in BENCHMARK_SUITES[name]:
                if operation_name not in normalized:
                    normalized.append(operation_name)
            continue
        if name == "all":
            for operation_name in OPERATION_SPECS:
                if operation_name not in normalized:
                    normalized.append(operation_name)
            continue
        operation_name = OPERATION_ALIASES.get(name, name)
        if operation_name not in OPERATION_SPECS:
            available = ", ".join(OPERATION_SPECS)
            raise SystemExit(f"unknown operation {name!r}. Available: {available}")
        if operation_name not in normalized:
            normalized.append(operation_name)
    return normalized


def time_operation(
    operation: PreparedOperation,
    iterations: int,
    warmups: int,
    min_time: float,
    max_iterations: int,
    disable_gc: bool,
) -> tuple[list[float], int]:
    for _ in range(warmups):
        operation()

    samples: list[float] = []
    checksum: int | None = None
    measured_seconds = 0.0
    while len(samples) < iterations or (
        min_time > 0 and measured_seconds < min_time and len(samples) < max_iterations
    ):
        if disable_gc:
            was_enabled = gc.isenabled()
            gc.disable()
        else:
            was_enabled = False
        try:
            start = time.perf_counter_ns()
            iteration_checksum = operation()
            elapsed = time.perf_counter_ns() - start
        finally:
            if disable_gc and was_enabled:
                gc.enable()
        if checksum is None:
            checksum = iteration_checksum
        elif checksum != iteration_checksum:
            checksum = combine_checksum(checksum, iteration_checksum)
        elapsed_ms = elapsed / 1_000_000
        measured_seconds += elapsed_ms / 1_000
        samples.append(elapsed_ms)
    return samples, checksum or 0


def percentile(samples: list[float], pct: float) -> float:
    if not samples:
        return 0.0
    if len(samples) == 1:
        return samples[0]
    ordered = sorted(samples)
    rank = (len(ordered) - 1) * pct
    lower = int(rank)
    upper = min(lower + 1, len(ordered) - 1)
    weight = rank - lower
    return ordered[lower] * (1 - weight) + ordered[upper] * weight


def result_from_samples(
    parser: str,
    operation_name: str,
    samples: list[float],
    checksum: int,
    doc_count: int,
    total_bytes: int,
    memory_mb: float | None,
    categories: tuple[str, ...] = (),
) -> TimedResult:
    mean_ms = statistics.mean(samples)
    median_ms = statistics.median(samples)
    stdev_ms = statistics.stdev(samples) if len(samples) > 1 else 0.0
    seconds = median_ms / 1_000
    return TimedResult(
        parser=parser,
        operation=operation_name,
        group=OPERATION_SPECS[operation_name].group,
        document_count=doc_count,
        total_bytes=total_bytes,
        iterations=len(samples),
        mean_ms=mean_ms,
        median_ms=median_ms,
        best_ms=min(samples),
        p95_ms=percentile(samples, 0.95),
        worst_ms=max(samples),
        stdev_ms=stdev_ms,
        rsd_pct=(stdev_ms / mean_ms * 100) if mean_ms else 0.0,
        ops_per_sec=1000 / median_ms if median_ms else 0.0,
        docs_per_sec=doc_count / seconds if seconds else 0.0,
        mib_per_sec=(total_bytes / (1024 * 1024)) / seconds if seconds else 0.0,
        checksum=checksum,
        samples_ms=samples,
        categories=categories,
        memory_mb=memory_mb,
    )


def current_peak_rss_bytes() -> int:
    value = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    if platform.system() == "Darwin":
        return int(value)
    return int(value) * 1024


def measure_memory_mb(args: argparse.Namespace, parser: str, operation: str) -> float | None:
    command = [
        sys.executable,
        os.path.abspath(__file__),
        "--worker-json",
        "--parser",
        parser,
        "--operation",
        operation,
        "--documents",
        str(args.documents),
        "--items-per-doc",
        str(args.items_per_doc),
        "--iterations",
        str(args.memory_iterations),
        "--warmups",
        "1",
        "--threads",
        str(args.threads),
        "--suite",
        args.suite,
    ]
    if args.input_dir:
        command.extend(["--input-dir", args.input_dir])
    if args.max_files is not None:
        command.extend(["--max-files", str(args.max_files)])
    if args.disable_gc:
        command.append("--disable-gc")
    result = subprocess.run(command, check=True, capture_output=True, text=True)
    payload = json.loads(result.stdout)
    return payload.get("peak_rss_delta_mb")


def run_worker(args: argparse.Namespace) -> None:
    html_docs = load_corpus(args)
    parser_name = args.parsers[0] if args.parsers else ""
    operation_name = args.operations[0] if args.operations else ""
    candidate = candidate_by_name(parser_name)
    if candidate is None or not candidate.available:
        raise SystemExit(f"parser unavailable: {parser_name}")
    operations = candidate_operations(candidate)
    operation_factory = operations[operation_name]
    gc.collect()
    before = current_peak_rss_bytes()
    operation = operation_factory(html_docs, args)
    samples, checksum = time_operation(
        operation.run,
        args.iterations,
        args.warmups,
        args.min_time,
        args.max_iterations,
        args.disable_gc,
    )
    gc.collect()
    after = current_peak_rss_bytes()
    print(
        json.dumps(
            {
                "parser": parser_name,
                "operation": operation_name,
                "best_ms": min(samples),
                "checksum": checksum,
                "peak_rss_delta_mb": max(0, after - before) / (1024 * 1024),
            }
        )
    )


def package_version(distribution: str) -> str | None:
    try:
        return importlib.metadata.version(distribution)
    except importlib.metadata.PackageNotFoundError:
        return None


def git_revision() -> str | None:
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=Path(__file__).resolve().parents[1],
            check=True,
            capture_output=True,
            text=True,
        )
    except Exception:
        return None
    return result.stdout.strip() or None


def corpus_metadata(args: argparse.Namespace, documents: list[CorpusDocument]) -> dict[str, Any]:
    lengths = [doc.byte_length for doc in documents]
    categories: dict[str, int] = {}
    for doc in documents:
        categories[doc.category] = categories.get(doc.category, 0) + 1
    return {
        "source": "files" if args.input_dir else "synthetic",
        "input_dir": args.input_dir,
        "documents_requested": args.documents,
        "items_per_doc": args.items_per_doc,
        "max_files": args.max_files,
        "actual_documents": len(documents),
        "total_html_bytes": sum(lengths),
        "min_document_bytes": min(lengths) if lengths else 0,
        "median_document_bytes": statistics.median(lengths) if lengths else 0,
        "max_document_bytes": max(lengths) if lengths else 0,
        "categories": categories,
        "fingerprint": checksum_docs(documents),
    }


def environment_metadata(args: argparse.Namespace) -> dict[str, Any]:
    return {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "command": shlex.join(sys.argv),
        "python": sys.version.replace("\n", " "),
        "python_executable": sys.executable,
        "implementation": platform.python_implementation(),
        "platform": platform.platform(),
        "machine": platform.machine(),
        "processor": platform.processor(),
        "git_revision": git_revision(),
        "versions": {
            "rustysoup": package_version("rustysoup"),
            "beautifulsoup4": package_version("beautifulsoup4"),
            "lxml": package_version("lxml"),
            "html5-parser": package_version("html5-parser"),
            "selectolax": package_version("selectolax"),
        },
        "settings": {
            "iterations": args.iterations,
            "warmups": args.warmups,
            "min_time": args.min_time,
            "max_iterations": args.max_iterations,
            "threads": args.threads,
            "disable_gc": args.disable_gc,
            "memory": args.memory,
            "memory_iterations": args.memory_iterations,
            "baseline": args.baseline,
            "suite": args.suite,
        },
    }


def checksum_mismatches(results: list[TimedResult]) -> dict[str, dict[int, list[str]]]:
    by_operation: dict[str, dict[int, list[str]]] = {}
    for result in results:
        spec = OPERATION_SPECS[result.operation]
        if not spec.comparable_checksum:
            continue
        checksums = by_operation.setdefault(result.operation, {})
        checksums.setdefault(result.checksum, []).append(result.parser)
    return {
        operation: checksums
        for operation, checksums in by_operation.items()
        if len(checksums) > 1
    }


def render_markdown_report(
    metadata: dict[str, Any],
    corpus: dict[str, Any],
    results: list[TimedResult],
    skipped: list[tuple[str, str]],
    baseline: str,
) -> str:
    lines: list[str] = []
    total_mib = corpus["total_html_bytes"] / (1024 * 1024)
    lines.append("# rustysoup Benchmark Report")
    lines.append("")
    lines.append("Benchmarks are workload-dependent. Treat these numbers as reproducible measurements, not universal claims.")
    lines.append("")
    lines.append("## Run")
    lines.append("")
    lines.append(f"- Generated: `{metadata['generated_at']}`")
    lines.append(f"- Command: `{metadata['command']}`")
    lines.append(f"- Platform: `{metadata['platform']}`")
    lines.append(f"- Python: `{metadata['python'].split()[0]}`")
    if metadata.get("git_revision"):
        lines.append(f"- Git revision: `{metadata['git_revision']}`")
    lines.append(f"- Suite: `{metadata['settings']['suite']}`")
    lines.append(
        f"- Iterations: `{metadata['settings']['iterations']}`, warmups: `{metadata['settings']['warmups']}`"
    )
    if metadata["settings"]["disable_gc"]:
        lines.append("- GC: disabled during timed samples")
    else:
        lines.append("- GC: Python default behavior")
    lines.append("")
    lines.append("## Corpus")
    lines.append("")
    lines.append(f"- Source: `{corpus['source']}`")
    if corpus["input_dir"]:
        lines.append(f"- Input directory: `{corpus['input_dir']}`")
    else:
        lines.append(f"- Synthetic documents: `{corpus['documents_requested']}`")
        lines.append(f"- Items per document: `{corpus['items_per_doc']}`")
    lines.append(f"- Actual documents: `{corpus['actual_documents']}`")
    lines.append(f"- Total HTML: `{total_mib:.2f} MiB`")
    lines.append(f"- Document bytes min/median/max: `{corpus['min_document_bytes']}` / `{corpus['median_document_bytes']}` / `{corpus['max_document_bytes']}`")
    if corpus.get("categories"):
        category_mix = ", ".join(
            f"{name}={count}" for name, count in sorted(corpus["categories"].items())
        )
        lines.append(f"- Category mix: `{category_mix}`")
    lines.append(f"- Corpus fingerprint: `{corpus['fingerprint']}`")
    lines.append("")
    lines.append("## Package Versions")
    lines.append("")
    for name, version in metadata["versions"].items():
        lines.append(f"- `{name}`: `{version or 'not installed'}`")
    if skipped:
        lines.append("")
        lines.append("## Skipped")
        lines.append("")
        for name, reason in skipped:
            lines.append(f"- `{name}`: {reason}")

    mismatches = checksum_mismatches(results)
    if mismatches:
        lines.append("")
        lines.append("## Checksum Warnings")
        lines.append("")
        lines.append("Comparable operations produced different checksums. Inspect parser behavior before using these rows for claims.")
        for operation, checksums in mismatches.items():
            variants = "; ".join(
                f"`{checksum:x}`: {', '.join(parsers)}"
                for checksum, parsers in checksums.items()
            )
            lines.append(f"- `{operation}`: {variants}")

    lines.append("")
    lines.append("## Results")
    by_operation: dict[str, list[TimedResult]] = {}
    for result in results:
        by_operation.setdefault(result.operation, []).append(result)

    for operation in OPERATION_SPECS:
        rows = by_operation.get(operation)
        if not rows:
            continue
        spec = OPERATION_SPECS[operation]
        baseline_row = next((row for row in rows if row.parser == baseline), None)
        has_memory = any(row.memory_mb is not None for row in rows)
        lines.append("")
        lines.append(f"### {operation}")
        lines.append("")
        lines.append(spec.description)
        if spec.categories:
            lines.append("")
            lines.append(f"Categories: `{', '.join(spec.categories)}`")
        lines.append("")
        memory_header = " | Peak RSS delta" if has_memory else ""
        lines.append(
            f"| Parser | Docs | Median | Best | Mean | p95 | Docs/sec | MiB/sec | vs {baseline} | RSD | Checksum{memory_header} |"
        )
        lines.append(f"|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:{'|---:' if has_memory else ''}|")
        for row in sorted(rows, key=lambda item: item.median_ms):
            ratio = ""
            if baseline_row is not None and row.median_ms > 0:
                ratio = f"{baseline_row.median_ms / row.median_ms:.2f}x"
            memory = f" | {row.memory_mb:.1f} MB" if row.memory_mb is not None else ""
            lines.append(
                f"| {row.parser} | {row.document_count} | {row.median_ms:.2f} ms | {row.best_ms:.2f} ms | "
                f"{row.mean_ms:.2f} ms | {row.p95_ms:.2f} ms | {row.docs_per_sec:.1f} | "
                f"{row.mib_per_sec:.1f} | {ratio} | {row.rsd_pct:.1f}% | `{row.checksum:x}`{memory} |"
            )
    lines.append("")
    return "\n".join(lines)


def list_capabilities() -> None:
    print("Parsers:")
    for candidate in all_candidates():
        status = "available" if candidate.available else f"skipped: {candidate.reason}"
        print(f"  {candidate.name:<24} {status}")
    print("\nOperations:")
    for name, spec in OPERATION_SPECS.items():
        category_note = f" categories={','.join(spec.categories)}" if spec.categories else ""
        print(f"  {name:<28} [{spec.group}] {spec.description}{category_note}")
    print("\nSuites:")
    for name, operations in BENCHMARK_SUITES.items():
        print(f"  {name:<28} {', '.join(operations)}")
    if OPERATION_ALIASES:
        print("\nAliases:")
        for alias, target in OPERATION_ALIASES.items():
            print(f"  {alias:<28} -> {target}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Benchmark rustysoup against BeautifulSoup, lxml, html5_parser, and selectolax."
    )
    parser.add_argument("--documents", type=int, default=25)
    parser.add_argument("--items-per-doc", type=int, default=200)
    parser.add_argument(
        "--input-dir",
        help="read .html/.htm/.xhtml/.txt files from this directory instead of generating a synthetic corpus",
    )
    parser.add_argument("--max-files", type=int, help="maximum number of input files to read")
    parser.add_argument("--iterations", type=int, default=15)
    parser.add_argument("--warmups", type=int, default=3)
    parser.add_argument(
        "--suite",
        choices=sorted(BENCHMARK_SUITES),
        default="standard",
        help="operation suite to run when --operation is not provided",
    )
    parser.add_argument(
        "--min-time",
        type=float,
        default=0.0,
        help="keep sampling each parser/operation until at least this many measured seconds have elapsed",
    )
    parser.add_argument(
        "--max-iterations",
        type=int,
        default=100,
        help="maximum samples per parser/operation when --min-time is used",
    )
    parser.add_argument(
        "--threads",
        type=int,
        default=1,
        help="worker count for parse_threaded",
    )
    parser.add_argument("--disable-gc", action="store_true", help="disable Python GC during timed samples")
    parser.add_argument("--memory", action="store_true", help="measure isolated peak RSS deltas")
    parser.add_argument("--memory-iterations", type=int, default=3)
    parser.add_argument(
        "--parser",
        action="append",
        dest="parsers",
        help="parser name to include; can be repeated",
    )
    parser.add_argument(
        "--operation",
        action="append",
        dest="operations",
        help="operation name to include; can be repeated; use 'all' for every operation",
    )
    parser.add_argument("--baseline", default="rustysoup")
    parser.add_argument("--json", action="store_true", help="emit JSON to stdout instead of Markdown")
    parser.add_argument("--json-output", help="write JSON payload to this path")
    parser.add_argument("--report", help="write Markdown report to this path")
    parser.add_argument(
        "--fail-on-checksum-mismatch",
        action="store_true",
        help="exit non-zero if comparable workloads produce different checksums",
    )
    parser.add_argument("--list", action="store_true", help="list available parsers and operations")
    parser.add_argument("--worker-json", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.threads < 1:
        parser.error("--threads must be >= 1")
    if args.iterations < 1:
        parser.error("--iterations must be >= 1")
    if args.warmups < 0:
        parser.error("--warmups must be >= 0")
    if args.max_iterations < args.iterations:
        parser.error("--max-iterations must be >= --iterations")
    args.operations = normalize_operation_names(args.operations, args.threads, args.suite)
    return args


def main() -> None:
    args = parse_args()
    if args.list:
        list_capabilities()
        return
    if args.worker_json:
        run_worker(args)
        return

    html_docs = load_corpus(args)
    corpus = corpus_metadata(args, html_docs)
    metadata = environment_metadata(args)
    selected = select_candidates(args.parsers)
    results: list[TimedResult] = []
    skipped: list[tuple[str, str]] = []

    for candidate in selected:
        if not candidate.available:
            skipped.append((candidate.name, candidate.reason))
            continue
        operations = candidate_operations(candidate)
        for operation_name in args.operations:
            operation_factory = operations.get(operation_name)
            if operation_factory is None:
                skipped.append((candidate.name, f"operation {operation_name!r} is unavailable"))
                continue
            gc.collect()
            operation = operation_factory(html_docs, args)
            if operation.document_count == 0:
                categories = ", ".join(operation.categories) or "all"
                skipped.append((candidate.name, f"operation {operation_name!r} has no matching documents for categories: {categories}"))
                continue
            samples, checksum = time_operation(
                operation.run,
                args.iterations,
                args.warmups,
                args.min_time,
                args.max_iterations,
                args.disable_gc,
            )
            memory_mb = (
                measure_memory_mb(args, candidate.name, operation_name) if args.memory else None
            )
            results.append(
                result_from_samples(
                    candidate.name,
                    operation_name,
                    samples,
                    checksum,
                    operation.document_count,
                    operation.total_bytes,
                    memory_mb,
                    operation.categories,
                )
            )
            del operation
            gc.collect()

    payload = {
        "metadata": metadata,
        "corpus": corpus,
        "operations": {name: asdict(spec) for name, spec in OPERATION_SPECS.items()},
        "results": [asdict(result) for result in results],
        "skipped": [{"parser": name, "reason": reason} for name, reason in skipped],
        "checksum_mismatches": checksum_mismatches(results),
    }
    report = render_markdown_report(metadata, corpus, results, skipped, args.baseline)

    if args.json_output:
        Path(args.json_output).write_text(json.dumps(payload, indent=2), encoding="utf-8")
    if args.report:
        Path(args.report).write_text(report, encoding="utf-8")
    if args.json:
        print(json.dumps(payload, indent=2))
    else:
        print(report)

    if args.fail_on_checksum_mismatch and payload["checksum_mismatches"]:
        raise SystemExit(2)


if __name__ == "__main__":
    main()
