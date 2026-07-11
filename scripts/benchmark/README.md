# Benchmark: RUSTScrapling vs Python Scrapling

Head-to-head parse + extraction benchmark on an identical fixture: a ~616 KB
e-commerce page with 1,000 product cards. Each run parses the page and
extracts title, price, and detail link from every card (3,000 fields).

## Run it

```bash
# 1. Generate the shared fixture (writes page.html next to this file)
python3 scripts/benchmark/gen_page.py

# 2. Python Scrapling (from this directory, so page.html resolves)
pip install scrapling
cd scripts/benchmark && python3 bench_python.py

# 3. RUSTScrapling (from the repo root)
cargo run --release --example benchmark
```

## Reference results

Same container, single-threaded, median of 30 runs. Python 3.11 /
Scrapling 0.4.10 (lxml) vs rustc release build:

| Workload | Python Scrapling | RUSTScrapling |
|----------|-----------------:|--------------:|
| parse + extract | 105.7 ms | 19.2 ms |
| parse only | 15.6 ms | 15.7 ms |
| peak process RSS | 37 MB | 9 MB |

Numbers vary with hardware; the *ratios* are the interesting part. Raw DOM
parsing is a tie (lxml's C parser vs html5ever) — the Rust advantage is in
selector matching and extraction, where there is no interpreter overhead.
