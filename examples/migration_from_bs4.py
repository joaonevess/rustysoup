"""
Demonstrating drop-in compatibility for BeautifulSoup 4 users.
Most extraction logic requires only a single import change.
"""

# 1. The only change needed in most cases:
from rustysoup import BeautifulSoup

# 2. Real-world fragment (e.g., a news article summary)
HTML = """
<article class="entry">
    <header>
        <h1 class="entry-title">Rustysoup 0.1 Released</h1>
        <p class="byline">By <span class="author">Alice</span> on <time>2026-05-17</time></p>
    </header>
    <div class="entry-content">
        <p>A new <b>fast</b> HTML parser for Python.</p>
    </div>
</article>
"""

def process_article(html_markup):
    # Constructor accepts common BS4 features like 'features' or 'from_encoding'
    soup = BeautifulSoup(html_markup, "html.parser")

    # Standard BS4 find() and property access
    title_node = soup.find("h1", class_="entry-title")
    content_node = soup.find("div", class_="entry-content")

    return {
        "title": title_node.string if title_node else None,
        "author": soup.find(class_="author").get_text(),
        "is_bold": soup.find("b") is not None,
        # Native CSS support
        "date": soup.select_one("time").get_text()
    }

if __name__ == "__main__":
    result = process_article(HTML)
    print(f"Title:  {result['title']}")
    print(f"Author: {result['author']}")
    print(f"Date:   {result['date']}")
    print(f"Bold?   {result['is_bold']}")
