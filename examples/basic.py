"""
A high-performance product data extraction example.
Focuses on clean CSS selectors and efficient attribute access.
"""

from rustysoup import Soup

HTML = """
<section class="results">
    <div class="item" data-id="101" data-category="grinders">
        <a class="item-link" href="/p/101">
            <h2 class="title">Apex Burr Grinder</h2>
            <img src="grinder.jpg" alt="Apex Grinder">
        </a>
        <div class="meta">
            <span class="price">$149.00</span>
            <span class="stock-status in-stock">In Stock</span>
        </div>
    </div>
    <div class="item" data-id="102" data-category="brewers">
        <a class="item-link" href="/p/102">
            <h2 class="title">Pour-over Kit</h2>
            <img src="kit.jpg" alt="Pour-over">
        </a>
        <div class="meta">
            <span class="price">$45.00</span>
            <span class="stock-status out-of-stock">Backordered</span>
        </div>
    </div>
</section>
"""

def extract_products(html: str):
    soup = Soup(html)
    products = []

    # select() returns a ResultSet (list-like) of Tag objects
    for item in soup.select("div.item[data-id]"):
        # Direct attribute access and clean text extraction
        link = item.select_one("a.item-link")
        
        product = {
            "id": int(item["data-id"]),
            "category": item.get("data-category"),
            "title": item.select_one("h2.title").get_text(strip=True),
            "url": link["href"] if link else None,
            "price": item.select_one(".price").get_text(strip=True),
            "available": "in-stock" in item.select_one(".stock-status")["class"]
        }
        products.append(product)

    return products

if __name__ == "__main__":
    import json
    data = extract_products(HTML)
    print(json.dumps(data, indent=2))
