from __future__ import annotations

"""Build a local HTML benchmark corpus from a curated URL list.

This script intentionally does not ship a corpus. Third-party HTML snapshots
should usually stay local; publish manifests, commands, timestamps, and hashes
instead of redistributing copied pages unless you have permission.
"""

import argparse
import hashlib
import json
import random
import re
import time
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable
from urllib import error, parse, request, robotparser


DEFAULT_USER_AGENT = (
    "rustysoup-benchmark-corpus/0.1 "
    "(responsible local benchmark collection; +https://github.com/joaonevess/rustysoup)"
)


@dataclass(frozen=True)
class UrlSpec:
    url: str
    category: str


@dataclass
class ManifestEntry:
    url: str
    category: str
    status: str
    filename: str | None = None
    sha256: str | None = None
    bytes: int | None = None
    content_type: str | None = None
    final_url: str | None = None
    error: str | None = None


def parse_url_file(path: Path) -> list[UrlSpec]:
    specs: list[UrlSpec] = []
    for line_number, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if "\t" in line:
            category, url = (part.strip() for part in line.split("\t", 1))
        else:
            category, url = "uncategorized", line
        parsed = parse.urlparse(url)
        if parsed.scheme not in {"http", "https"} or not parsed.netloc:
            raise SystemExit(f"{path}:{line_number}: expected an http(s) URL, got {url!r}")
        specs.append(UrlSpec(url=url, category=category or "uncategorized"))
    if not specs:
        raise SystemExit(f"{path} contained no URLs")
    return specs


def slug(value: str, default: str = "item") -> str:
    value = re.sub(r"[^a-zA-Z0-9._-]+", "-", value.lower()).strip("-._")
    return value[:80] or default


def normalize_http_url(url: str) -> str:
    """Return a URL safe for urllib without changing its logical target."""
    parts = parse.urlsplit(url)
    path = parse.quote(parts.path or "/", safe="/%:@")
    query = parse.quote(parts.query, safe="=&%:+,;/?@")
    return parse.urlunsplit((parts.scheme, parts.netloc, path, query, ""))


class RobotsCache:
    def __init__(self, user_agent: str, timeout: float) -> None:
        self.user_agent = user_agent
        self.timeout = timeout
        self._cache: dict[str, robotparser.RobotFileParser | None] = {}

    def allowed(self, url: str) -> tuple[bool, str]:
        parsed = parse.urlparse(url)
        origin = f"{parsed.scheme}://{parsed.netloc}"
        if origin not in self._cache:
            parser = robotparser.RobotFileParser()
            robots_url = parse.urljoin(origin, "/robots.txt")
            parser.set_url(robots_url)
            try:
                req = request.Request(
                    robots_url,
                    headers={
                        "User-Agent": self.user_agent,
                        "Accept": "text/plain,*/*;q=0.1",
                    },
                )
                with request.urlopen(req, timeout=self.timeout) as response:
                    body = response.read(256_000)
                parser.parse(body.decode("utf-8", errors="replace").splitlines())
            except error.HTTPError as exc:
                if exc.code in {404, 410}:
                    parser.parse([])
                    self._cache[origin] = parser
                    return True, f"robots.txt missing ({exc.code})"
                self._cache[origin] = None
                return False, f"could not read robots.txt: HTTP {exc.code}"
            except Exception as exc:
                self._cache[origin] = None
                return False, f"could not read robots.txt: {exc}"
            self._cache[origin] = parser
        parser = self._cache[origin]
        if parser is None:
            return False, "robots.txt unavailable"
        return parser.can_fetch(self.user_agent, url), "robots.txt"


def fetch_url(
    spec: UrlSpec,
    output_dir: Path,
    index: int,
    user_agent: str,
    timeout: float,
    max_bytes: int,
    min_bytes: int,
    allow_non_html: bool,
) -> ManifestEntry:
    req = request.Request(
        normalize_http_url(spec.url),
        headers={
            "User-Agent": user_agent,
            "Accept": "text/html,application/xhtml+xml;q=0.9,text/plain;q=0.5,*/*;q=0.1",
        },
    )
    try:
        with request.urlopen(req, timeout=timeout) as response:
            content_type = response.headers.get("content-type", "")
            final_url = response.geturl()
            body = response.read(max_bytes + 1)
    except (error.HTTPError, error.URLError, TimeoutError, OSError) as exc:
        return ManifestEntry(
            url=spec.url,
            category=spec.category,
            status="error",
            error=str(exc),
        )

    if len(body) > max_bytes:
        return ManifestEntry(
            url=spec.url,
            category=spec.category,
            status="skipped_too_large",
            bytes=len(body),
            content_type=content_type,
            final_url=final_url,
        )
    if len(body) < min_bytes:
        return ManifestEntry(
            url=spec.url,
            category=spec.category,
            status="skipped_too_small",
            bytes=len(body),
            content_type=content_type,
            final_url=final_url,
        )
    lowered_content_type = content_type.lower()
    looks_html = any(
        media_type in lowered_content_type
        for media_type in ("text/html", "application/xhtml+xml", "text/plain")
    )
    if not looks_html and not allow_non_html:
        return ManifestEntry(
            url=spec.url,
            category=spec.category,
            status="skipped_non_html",
            bytes=len(body),
            content_type=content_type,
            final_url=final_url,
        )

    digest = hashlib.sha256(body).hexdigest()
    host = slug(parse.urlparse(final_url or spec.url).netloc, "host")
    category = slug(spec.category, "category")
    filename = f"{index:05d}__{category}__{host}__{digest[:12]}.html"
    path = output_dir / filename
    path.write_bytes(body)
    return ManifestEntry(
        url=spec.url,
        category=spec.category,
        status="saved",
        filename=filename,
        sha256=digest,
        bytes=len(body),
        content_type=content_type,
        final_url=final_url,
    )


def iter_specs(args: argparse.Namespace) -> list[UrlSpec]:
    specs = parse_url_file(Path(args.url_file))
    if args.shuffle:
        rng = random.Random(args.seed)
        rng.shuffle(specs)
    if args.limit is not None:
        specs = specs[: args.limit]
    return specs


def collect(args: argparse.Namespace) -> dict[str, object]:
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    specs = iter_specs(args)
    robots = RobotsCache(args.user_agent, args.timeout)
    host_last_fetch: dict[str, float] = {}
    entries: list[ManifestEntry] = []

    for index, spec in enumerate(specs, 1):
        parsed = parse.urlparse(spec.url)
        host = parsed.netloc.lower()
        if args.respect_robots:
            allowed, reason = robots.allowed(spec.url)
            if not allowed:
                entry = ManifestEntry(
                    url=spec.url,
                    category=spec.category,
                    status="skipped_robots",
                    error=reason,
                )
                entries.append(entry)
                if not args.quiet:
                    print(f"{entry.status:<18} {spec.category:<24} {spec.url}")
                continue

        last_fetch = host_last_fetch.get(host)
        if last_fetch is not None:
            wait = args.rate_limit - (time.monotonic() - last_fetch)
            if wait > 0:
                time.sleep(wait)

        entry = fetch_url(
            spec,
            output_dir,
            index,
            args.user_agent,
            args.timeout,
            args.max_bytes,
            args.min_bytes,
            args.allow_non_html,
        )
        host_last_fetch[host] = time.monotonic()
        entries.append(entry)
        if not args.quiet:
            print(f"{entry.status:<18} {spec.category:<24} {spec.url}")

    manifest = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "url_file": str(Path(args.url_file).resolve()),
        "output_dir": str(output_dir.resolve()),
        "user_agent": args.user_agent,
        "respect_robots": args.respect_robots,
        "rate_limit_seconds": args.rate_limit,
        "timeout_seconds": args.timeout,
        "max_bytes": args.max_bytes,
        "min_bytes": args.min_bytes,
        "requested_urls": len(specs),
        "saved": sum(1 for entry in entries if entry.status == "saved"),
        "entries": [asdict(entry) for entry in entries],
    }
    manifest_path = output_dir / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return manifest


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Collect a local HTML corpus for rustysoup benchmarks from a curated URL file."
    )
    parser.add_argument("--url-file", required=True, help="UTF-8 URL file; use 'category<TAB>url' per line")
    parser.add_argument("--output-dir", default=".benchmarks/corpora/default")
    parser.add_argument("--limit", type=int)
    parser.add_argument("--shuffle", action="store_true")
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--timeout", type=float, default=20.0)
    parser.add_argument("--rate-limit", type=float, default=2.0, help="minimum seconds between requests per host")
    parser.add_argument("--max-bytes", type=int, default=5_000_000)
    parser.add_argument("--min-bytes", type=int, default=200)
    parser.add_argument("--allow-non-html", action="store_true")
    parser.add_argument("--ignore-robots", action="store_true", help="only use for URLs you are allowed to fetch")
    parser.add_argument("--user-agent", default=DEFAULT_USER_AGENT)
    parser.add_argument("--quiet", action="store_true")
    args = parser.parse_args(argv)
    args.respect_robots = not args.ignore_robots
    if args.rate_limit < 0:
        parser.error("--rate-limit must be >= 0")
    if args.max_bytes < 1:
        parser.error("--max-bytes must be >= 1")
    if args.min_bytes < 0:
        parser.error("--min-bytes must be >= 0")
    return args


def main() -> None:
    manifest = collect(parse_args())
    print(
        f"Saved {manifest['saved']} of {manifest['requested_urls']} URLs to {manifest['output_dir']}"
    )
    print(f"Manifest: {Path(manifest['output_dir']) / 'manifest.json'}")


if __name__ == "__main__":
    main()
