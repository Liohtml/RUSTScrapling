"""Benchmark Python Scrapling: parse + CSS select + text/attr extraction."""

import pathlib
import statistics
import time

from scrapling.parser import Selector

# The fixture lives next to this script (written there by gen_page.py).
html = (pathlib.Path(__file__).resolve().parent / "page.html").read_text()

N = 30


def run_once():
    page = Selector(html)
    products = page.css(".product")
    total = 0
    for p in products:
        title = p.css(".title")[0].text
        price = p.css(".price")[0].text
        link = p.css(".detail-link")[0].attrib["href"]
        total += len(str(title)) + len(str(price)) + len(str(link))
    return len(products), total


# Warmup
count, checksum = run_once()
assert count == 1000, count

times = []
for _ in range(N):
    t0 = time.perf_counter()
    run_once()
    times.append(time.perf_counter() - t0)

print(f"python scrapling  parse+extract  n={N}")
print(f"  median: {statistics.median(times)*1000:.1f} ms")
print(f"  mean:   {statistics.mean(times)*1000:.1f} ms")
print(f"  min:    {min(times)*1000:.1f} ms")
print(f"  checksum: {checksum}")

# Parse-only
times = []
for _ in range(N):
    t0 = time.perf_counter()
    Selector(html)
    times.append(time.perf_counter() - t0)
print(f"python scrapling  parse-only     median: {statistics.median(times)*1000:.1f} ms")
