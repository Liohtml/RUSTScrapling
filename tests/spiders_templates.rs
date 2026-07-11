//! End-to-end tests for `CrawlSpider` and `SitemapSpider` templates, run
//! through the real crawl engine in development mode with a pre-seeded
//! response cache (no network I/O).

use rust_scrapling::fetchers::config::FetcherConfig;
use rust_scrapling::fetchers::response::Response;
use rust_scrapling::spiders::cache::{CachedResponse, ResponseCache};
use rust_scrapling::spiders::engine::CrawlerEngine;
use rust_scrapling::spiders::links::LinkExtractor;
use rust_scrapling::spiders::response::SpiderResponse;
use rust_scrapling::spiders::session::SessionManager;
use rust_scrapling::spiders::templates::{CrawlRule, CrawlSpider, SitemapSpider};
use std::collections::HashMap;
use std::sync::Arc;

async fn seed(dir: &str, pages: &[(&str, &str, &str)]) {
    let cache = ResponseCache::new(&format!("{}/cache", dir)).unwrap();
    for (url, content_type, body) in pages {
        let cached = CachedResponse {
            status: 200,
            content_type: content_type.to_string(),
            body: body.to_string(),
            url: url.to_string(),
            headers: HashMap::new(),
        };
        cache.put(url, &cached).await.unwrap();
    }
}

fn spider_response(url: &str, body: &str) -> SpiderResponse {
    SpiderResponse::new(Response::new(
        200,
        "text/html".to_string(),
        body.to_string(),
        url.to_string(),
        HashMap::new(),
    ))
}

#[tokio::test]
async fn crawl_spider_follows_rules_and_extracts_items() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_str().unwrap().to_string();
    seed(
        &base,
        &[
            (
                "https://example.com/",
                "text/html",
                r#"<a href="/articles/1">1</a><a href="/articles/2">2</a><a href="/about">skip</a>"#,
            ),
            (
                "https://example.com/articles/1",
                "text/html",
                "<h1>Article one</h1>",
            ),
            (
                "https://example.com/articles/2",
                "text/html",
                "<h1>Article two</h1>",
            ),
        ],
    )
    .await;

    let spider = CrawlSpider::builder("crawl-test")
        .start_urls(["https://example.com/"])
        .rule(CrawlRule::new(
            LinkExtractor::new().allow([r"/articles/"]).unwrap(),
        ))
        .parse_item(Arc::new(|resp: &SpiderResponse| {
            resp.css("h1")
                .into_iter()
                .map(|h| serde_json::json!({ "title": h.text().to_string() }))
                .collect()
        }))
        .development_mode(true)
        .build();

    let engine = CrawlerEngine::new(
        Arc::new(spider),
        SessionManager::new(FetcherConfig::default()),
        Some(&base),
    )
    .expect("engine builds");
    let result = engine.crawl().await;

    // Both articles are followed and parsed; /about is filtered by the rule.
    let mut titles: Vec<String> = result
        .items
        .into_iter()
        .map(|i| i["title"].as_str().unwrap().to_string())
        .collect();
    titles.sort();
    assert_eq!(titles, vec!["Article one", "Article two"]);
    assert_eq!(result.stats.cache_hits, 3, "start page + 2 articles");
}

#[tokio::test]
async fn crawl_rule_priority_and_process_request_apply() {
    let resp = spider_response(
        "https://example.com/",
        r#"<a href="/articles/1">1</a><a href="/articles/drop">2</a>"#,
    );
    let rule = CrawlRule::new(LinkExtractor::new().allow([r"/articles/"]).unwrap())
        .priority(7)
        .process_request(Arc::new(|req, _resp| {
            if req.url().ends_with("/drop") {
                None
            } else {
                Some(req)
            }
        }));

    let requests = rule.requests(&resp);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url(), "https://example.com/articles/1");
    assert_eq!(requests[0].priority(), 7);
}

#[test]
fn sitemap_body_parsing_handles_urlset_and_index() {
    let spider = SitemapSpider::builder("sm").build();

    let urlset = r#"<?xml version="1.0" encoding="UTF-8"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
          <url><loc>https://example.com/page1</loc><lastmod>2026-01-01</lastmod></url>
          <url><loc>https://example.com/page2</loc></url>
        </urlset>"#;
    let result = spider.parse_sitemap_body(urlset);
    assert_eq!(
        result.urls,
        vec!["https://example.com/page1", "https://example.com/page2"]
    );
    assert!(result.sitemaps.is_empty());

    let index = r#"<?xml version="1.0" encoding="UTF-8"?>
        <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
          <sitemap><loc>https://example.com/sitemap-a.xml</loc></sitemap>
          <sitemap><loc>https://example.com/sitemap-b.xml</loc></sitemap>
        </sitemapindex>"#;
    let result = spider.parse_sitemap_body(index);
    assert!(result.urls.is_empty());
    assert_eq!(
        result.sitemaps,
        vec![
            "https://example.com/sitemap-a.xml",
            "https://example.com/sitemap-b.xml"
        ]
    );
}

#[test]
fn sitemap_alternate_links_are_optional() {
    let urlset = r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
        <url>
          <loc>https://example.com/en</loc>
          <xhtml:link rel="alternate" hreflang="de" href="https://example.com/de"/>
        </url>
      </urlset>"#;

    let without = SitemapSpider::builder("sm").build();
    assert_eq!(
        without.parse_sitemap_body(urlset).urls,
        vec!["https://example.com/en"]
    );

    let with = SitemapSpider::builder("sm")
        .sitemap_alternate_links(true)
        .build();
    assert_eq!(
        with.parse_sitemap_body(urlset).urls,
        vec!["https://example.com/en", "https://example.com/de"]
    );
}

#[tokio::test]
async fn sitemap_spider_crawls_from_robots_txt_through_index_to_pages() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_str().unwrap().to_string();
    seed(
        &base,
        &[
            (
                "https://example.com/robots.txt",
                "text/plain",
                "User-agent: *\nDisallow:\nSitemap: https://example.com/sitemap-index.xml\n",
            ),
            (
                "https://example.com/sitemap-index.xml",
                "application/xml",
                r#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                     <sitemap><loc>https://example.com/sitemap-articles.xml</loc></sitemap>
                     <sitemap><loc>https://example.com/sitemap-media.xml</loc></sitemap>
                   </sitemapindex>"#,
            ),
            (
                "https://example.com/sitemap-articles.xml",
                "application/xml",
                r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                     <url><loc>https://example.com/articles/1</loc></url>
                     <url><loc>https://example.com/pricing</loc></url>
                   </urlset>"#,
            ),
            (
                "https://example.com/articles/1",
                "text/html",
                "<h1>Sitemap article</h1>",
            ),
        ],
    )
    .await;

    let spider = SitemapSpider::builder("sitemap-test")
        .sitemap_urls(["https://example.com/robots.txt"])
        // Only descend into the articles sitemap, not media.
        .sitemap_follow(LinkExtractor::new().allow([r"articles"]).unwrap())
        // Only dispatch article pages; /pricing is dropped by the rule.
        .rule(CrawlRule::new(
            LinkExtractor::new().allow([r"/articles/"]).unwrap(),
        ))
        .parse_item(Arc::new(|resp: &SpiderResponse| {
            resp.css("h1")
                .into_iter()
                .map(|h| serde_json::json!({ "title": h.text().to_string() }))
                .collect()
        }))
        .development_mode(true)
        .build();

    let engine = CrawlerEngine::new(
        Arc::new(spider),
        SessionManager::new(FetcherConfig::default()),
        Some(&base),
    )
    .expect("engine builds");
    let result = engine.crawl().await;

    let items: Vec<serde_json::Value> = result.items.into_iter().collect();
    assert_eq!(items.len(), 1, "only the article page yields an item");
    assert_eq!(items[0]["title"], serde_json::json!("Sitemap article"));
    // robots.txt + index + articles sitemap + article page; the media
    // sitemap is filtered by sitemap_follow and /pricing by the rule (both
    // are also absent from the cache, so following them would show up as
    // failed requests).
    assert_eq!(result.stats.cache_hits, 4);
    assert_eq!(result.stats.failed_requests_count, 0);
}

#[tokio::test]
async fn sitemap_spider_without_rules_follows_all_urls() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_str().unwrap().to_string();
    seed(
        &base,
        &[
            (
                "https://example.com/sitemap.xml",
                "application/xml",
                r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                     <url><loc>https://example.com/a</loc></url>
                     <url><loc>https://example.com/b</loc></url>
                   </urlset>"#,
            ),
            ("https://example.com/a", "text/html", "<h1>A</h1>"),
            ("https://example.com/b", "text/html", "<h1>B</h1>"),
        ],
    )
    .await;

    let spider = SitemapSpider::builder("sitemap-all")
        .sitemap_urls(["https://example.com/sitemap.xml"])
        .parse_item(Arc::new(|resp: &SpiderResponse| {
            vec![serde_json::json!({ "url": resp.url() })]
        }))
        .development_mode(true)
        .build();

    let engine = CrawlerEngine::new(
        Arc::new(spider),
        SessionManager::new(FetcherConfig::default()),
        Some(&base),
    )
    .expect("engine builds");
    let result = engine.crawl().await;
    assert_eq!(result.items.len(), 2, "all sitemap URLs are followed");
}

#[test]
fn self_closing_alternate_links_do_not_swallow_locs() {
    // html5ever treats a self-closing <xhtml:link/> as an open tag, nesting
    // whatever follows inside it. Descendant traversal must still find every
    // loc and alternate regardless of element order.
    let link_first = r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
        <url>
          <xhtml:link rel="alternate" hreflang="de" href="https://example.com/de"/>
          <loc>https://example.com/en</loc>
        </url>
      </urlset>"#;
    let plain = SitemapSpider::builder("sm").build();
    assert_eq!(
        plain.parse_sitemap_body(link_first).urls,
        vec!["https://example.com/en"],
        "loc after a self-closing alternate link must not be lost"
    );

    // Multiple alternates: the second link nests inside the first under
    // html5ever, but all alternates and the loc must survive.
    let multi = r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
        <url>
          <loc>https://example.com/en</loc>
          <xhtml:link rel="alternate" hreflang="de" href="https://example.com/de"/>
          <xhtml:link rel="alternate" hreflang="fr" href="https://example.com/fr"/>
        </url>
      </urlset>"#;
    let with = SitemapSpider::builder("sm")
        .sitemap_alternate_links(true)
        .build();
    let urls = with.parse_sitemap_body(multi).urls;
    assert!(urls.contains(&"https://example.com/en".to_string()));
    assert!(urls.contains(&"https://example.com/de".to_string()));
    assert!(
        urls.contains(&"https://example.com/fr".to_string()),
        "second alternate must not be swallowed by the first"
    );
}

#[tokio::test]
async fn relative_sitemap_urls_are_resolved_against_response_url() {
    use rust_scrapling::spiders::spider::Spider;

    let spider = SitemapSpider::builder("sm").build();

    // Relative loc entries in a urlset.
    let resp = spider_response(
        "https://example.com/sitemap.xml",
        r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
             <url><loc>/page1</loc></url>
           </urlset>"#,
    );
    let (_, requests) = spider.parse(resp).await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url(), "https://example.com/page1");

    // Relative Sitemap directive in robots.txt, plus an inline comment.
    let resp = spider_response(
        "https://example.com/robots.txt",
        "User-agent: *\nSitemap: /sitemap.xml # primary\n",
    );
    let (_, requests) = spider.parse(resp).await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url(), "https://example.com/sitemap.xml");
}

#[tokio::test]
async fn content_page_mentioning_urlset_is_not_misclassified() {
    use rust_scrapling::spiders::spider::Spider;

    let spider = SitemapSpider::builder("sm")
        .parse_item(Arc::new(|resp: &SpiderResponse| {
            vec![serde_json::json!({ "url": resp.url() })]
        }))
        .build();

    let resp = spider_response(
        "https://example.com/blog/sitemaps-explained",
        r#"<!doctype html><html><body>
            <h1>All about sitemaps</h1>
            <script>var example = "<urlset xmlns='x'>";</script>
        </body></html>"#,
    );
    let (items, requests) = spider.parse(resp).await;
    assert_eq!(items.len(), 1, "content page must still yield its items");
    assert!(requests.is_empty());

    // A real sitemap with an XML prolog and leading comment still parses.
    let resp = spider_response(
        "https://example.com/sitemap.xml",
        "<?xml version=\"1.0\"?>\n<!-- generated -->\n<urlset><url><loc>https://example.com/a</loc></url></urlset>",
    );
    let (items, requests) = spider.parse(resp).await;
    assert!(items.is_empty());
    assert_eq!(requests.len(), 1);
}
