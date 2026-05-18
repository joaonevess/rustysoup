import os
import subprocess
import sys
import textwrap


def run_child(code: str, timeout: int = 120) -> str:
    env = os.environ.copy()
    env["PYTHONFAULTHANDLER"] = "1"
    proc = subprocess.run(
        [sys.executable, "-c", textwrap.dedent(code)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        timeout=timeout,
        env=env,
    )
    assert proc.returncode == 0, (
        f"child process exited with {proc.returncode}\n"
        f"stdout:\n{proc.stdout}\n"
        f"stderr:\n{proc.stderr}"
    )
    return proc.stdout


def test_deep_descendant_selector_does_not_crash_process():
    out = run_child(
        """
        from rustysoup import BeautifulSoup

        n = 80_000
        html = "<div>" * n + "x" + "</div>" * n
        soup = BeautifulSoup(html, "html.parser")

        assert len(soup.select("div div")) == n - 1
        assert len(soup.select("body div")) == 0
        print("ok")
        """
    )

    assert out.strip() == "ok"


def test_deep_parse_only_filter_does_not_crash_process():
    out = run_child(
        """
        from rustysoup import BeautifulSoup, SoupStrainer

        n = 60_000
        html = "<div>" * n + "x" + "</div>" * n

        no_match = BeautifulSoup(html, "html.parser", parse_only=SoupStrainer("span"))
        assert len(no_match.contents) == 0

        matched = BeautifulSoup(html, "html.parser", parse_only=SoupStrainer("div"))
        assert matched.get_text() == "x"
        print("ok")
        """
    )

    assert out.strip() == "ok"


def test_deep_text_and_serialization_do_not_crash_process():
    out = run_child(
        """
        from rustysoup import BeautifulSoup

        n = 100_000
        html = "<div>" * n + "x" + "</div>" * n
        soup = BeautifulSoup(html, "html.parser")

        assert soup.get_text() == "x"
        assert len(soup.decode()) == len(html)
        print("ok")
        """
    )

    assert out.strip() == "ok"
