"""Generate the shared benchmark page: 1000 product cards, ~600 KB."""

import random

random.seed(42)

cards = []
for i in range(1000):
    cards.append(f"""
    <article class="product" data-sku="SKU-{i:05d}" data-category="cat-{i % 20}">
      <h2 class="title">Product {i} — {'Widget' if i % 2 else 'Gadget'} {random.choice(['Pro', 'Max', 'Lite', 'Ultra'])}</h2>
      <p class="description">High-quality item number {i} with several useful properties and a longer description text so the document has realistic text density for extraction benchmarks.</p>
      <span class="price" data-currency="EUR">{random.randint(1, 999)}.{random.randint(0, 99):02d}</span>
      <div class="meta">
        <span class="rating">{random.randint(1, 5)} stars</span>
        <a class="detail-link" href="/products/{i}?ref=list&page={i // 50}">Details</a>
        <a class="review-link" href="/products/{i}/reviews">Reviews</a>
      </div>
    </article>""")

html = f"""<!doctype html>
<html><head><title>Product Catalog</title></head>
<body>
<nav><a href="/">Home</a><a href="/products">Products</a><a href="/cart">Cart</a></nav>
<main id="catalog">{''.join(cards)}</main>
<footer><p>Benchmark fixture page</p></footer>
</body></html>"""

with open("page.html", "w") as f:
    f.write(html)
print(f"page.html written: {len(html)/1024:.0f} KB")
