//! Parse + extract benchmark against the shared fixture page.
//!
//! Generate the fixture first, then run in release mode from the repo root:
//!
//! ```bash
//! python3 scripts/benchmark/gen_page.py   # writes scripts/benchmark/page.html
//! cargo run --release --example benchmark
//! ```
//!
//! The equivalent Python Scrapling benchmark is
//! `scripts/benchmark/bench_python.py`.

use rust_scrapling::Selector;
use std::time::Instant;

const N: usize = 30;

fn run_once(html: &str) -> (usize, usize) {
    let page = Selector::from_html(html);
    let products = page.css(".product");
    let mut total = 0usize;
    for p in &products {
        let title = p.css(".title").first().unwrap().text().to_string();
        let price = p.css(".price").first().unwrap().text().to_string();
        let link = p
            .css(".detail-link")
            .first()
            .unwrap()
            .get_attribute("href")
            .unwrap()
            .to_string();
        total += title.len() + price.len() + link.len();
    }
    (products.len(), total)
}

fn main() {
    let html = std::fs::read_to_string("scripts/benchmark/page.html")
        .expect("run `python3 scripts/benchmark/gen_page.py` first");

    // Warmup + sanity check
    let (count, checksum) = run_once(&html);
    assert_eq!(count, 1000, "fixture should contain 1000 products");

    let mut times: Vec<f64> = Vec::with_capacity(N);
    for _ in 0..N {
        let t0 = Instant::now();
        run_once(&html);
        times.push(t0.elapsed().as_secs_f64());
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = times.iter().sum::<f64>() / N as f64;
    println!("rust_scrapling  parse+extract  n={N}");
    println!("  median: {:.1} ms", times[N / 2] * 1000.0);
    println!("  mean:   {:.1} ms", mean * 1000.0);
    println!("  min:    {:.1} ms", times[0] * 1000.0);
    println!("  checksum: {checksum}");

    let mut times: Vec<f64> = Vec::with_capacity(N);
    for _ in 0..N {
        let t0 = Instant::now();
        let _ = Selector::from_html(&html);
        times.push(t0.elapsed().as_secs_f64());
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "rust_scrapling  parse-only     median: {:.1} ms",
        times[N / 2] * 1000.0
    );
}
