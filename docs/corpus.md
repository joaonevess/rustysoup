# Benchmark Corpus

The `rustysoup` benchmark corpus is designed to mirror real extraction work: navigation-heavy pages, article bodies, product cards, tables, forms, malformed markup, and large documents.

Third-party HTML snapshots are not committed to source control. The repository provides collection tools and manifests so benchmark runs can be reproduced locally.

## Corpus Stratification

A strong corpus is stratified across common web shapes:

- Homepages: navigation-heavy pages with many links and metadata.
- News articles: article pages with scripts, ads, embeds, comments, and malformed fragments.
- News section/listing pages: many repeated cards and links.
- E-commerce category pages: product grids, filters, nested cards, lazy images.
- E-commerce product pages: structured data, price blocks, reviews, variants.
- Documentation pages: code blocks, tables, sidebars, headings, anchors.
- Blog posts: mixed prose, code, images, embeds.
- Forum or discussion threads: repeated comments and nested quoting.
- Profiles or directory pages: many small repeated records.
- Table-heavy pages: financial, sports, docs, or wiki-like tables.
- Form-heavy pages: inputs, labels, selects, disabled/boolean attributes.
- International pages: non-English text, non-ASCII attributes, varied encodings.
- Large pages: 1-5 MB pages with lots of script/data blobs.
- Malformed legacy pages: older HTML, unclosed tags, unusual attributes.

Popular sites are useful, but coverage matters more than rank. The corpus should exercise the HTML structures that real scrapers parse every day.

## What To Avoid

- Login-only pages.
- Pages restricted by robots.txt, terms of service, or access controls.
- Infinite calendars/search spaces.
- Private, personal, or sensitive pages.
- Raw HTML checked into git without permission.
- Dynamic pages where the server response is mostly an empty app shell, unless that shell is a workload you care about.

## Responsible Collection

Start from the curated seed list in `benches/corpus_urls.txt`, or create your own URL list:

```text
homepage	https://example.com/
docs_article	https://example.com/docs/getting-started
ecommerce_product	https://example.com/products/widget
```

Then collect locally:

```bash
python benches/collect_corpus.py \
  --url-file benches/corpus_urls.txt \
  --output-dir .benchmarks/corpora/public-web-001 \
  --rate-limit 2 \
  --max-bytes 5000000
```

The collector:

- respects robots.txt by default,
- rate-limits per host,
- records status, content type, final URL, bytes, and SHA-256,
- writes `manifest.json`,
- saves raw HTML files for local benchmark runs.

Run benchmarks against the corpus:

```bash
python benches/benchmark_parsers.py \
  --input-dir .benchmarks/corpora/public-web-001 \
  --iterations 25 \
  --warmups 5 \
  --min-time 0.25 \
  --report benchmark-report.md \
  --json-output benchmark-report.json
```

## Public Reporting

Published benchmark reports should include:

- the benchmark command,
- package versions,
- platform and Python version,
- corpus size,
- category mix,
- corpus fingerprint,
- skipped parser backends and reasons,
- checksum warnings if any.
