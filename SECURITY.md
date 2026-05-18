# Security Policy

## Supported Versions

`rustysoup` is pre-1.0. Security fixes target the latest published release.

## Reporting

Please report vulnerabilities through GitHub private vulnerability reporting:

https://github.com/joaonevess/rustysoup/security/advisories/new

Do not publish exploit details in public issues, pull requests, discussions, or benchmark artifacts. If private reporting is unavailable, open a minimal public issue asking for a private security contact and omit technical details.

Include:

- affected `rustysoup` version or commit,
- Python version, operating system, and installation source,
- minimal reproducer,
- expected security impact,
- whether the issue is already public.

## Scope

Security-relevant issues include:

- memory safety bugs,
- panics, aborts, or crashes reachable from untrusted HTML,
- denial-of-service behavior from crafted input,
- unsafe file or network access,
- Python extension undefined behavior,
- package artifact, release, or supply-chain compromise.

`rustysoup` parses HTML. It does not execute JavaScript, fetch remote resources, enforce browser security policy, or sanitize HTML for safe rendering. Parser differences, unsupported BeautifulSoup behavior, and malformed HTML handling are compatibility issues unless they create a concrete security impact.

## Response

Valid reports are triaged privately. Confirmed vulnerabilities are fixed, released, and documented with appropriate credit unless the reporter requests otherwise.
