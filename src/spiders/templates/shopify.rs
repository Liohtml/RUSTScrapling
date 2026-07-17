//! `ShopifySpider`, ported from upstream Scrapling's
//! `spiders/templates/shopify.py` (v0.4.11).
//!
//! Extracts every product from a Shopify-powered store through its JSON API
//! — no HTML parsing involved: it walks the store's `/collections.json`
//! pages, then each collection's `products.json` pages, and yields one item
//! per product variant.
//!
//! The Python original routes responses through per-request callbacks; this
//! port has no callbacks (the [`Spider`] trait routes everything through
//! `parse`), so responses are routed by their URL shape instead:
//! `/collections.json` vs `/collections/{handle}/products.json`.

use crate::parser::Selector;
use crate::spiders::request::SpiderRequest;
use crate::spiders::response::SpiderResponse;
use crate::spiders::spider::Spider;
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Mutex;
use url::Url;

/// A [`Spider`] that extracts all products from any Shopify-powered website
/// through its JSON API.
///
/// Configure it with [`ShopifySpider::builder`], passing the store's domain
/// (or any URL on it). Each scraped item is one product *variant* with the
/// fields `name`, `price`, `category`, `brand`, `identifier`, `sku`,
/// `stock`, `image_url`, `url`, `description`, `old_price`, and `barcode` —
/// the same shape as upstream Scrapling's `ShopifySpider`.
pub struct ShopifySpider {
    name: String,
    /// Store host (netloc), e.g. `shop.example.com`.
    target_website: String,
    /// Variant IDs already emitted, so a product listed in several
    /// collections yields each variant only once.
    collected_ids: Mutex<HashSet<i64>>,
    allowed_domains: HashSet<String>,
    concurrent_requests: u32,
    download_delay: f64,
    development_mode: bool,
    robots_txt_obey: bool,
}

const PAGE_LIMIT: u32 = 250;

impl ShopifySpider {
    /// `target` may be a bare domain (`shop.example.com`) or any URL on the
    /// store (`https://shop.example.com/some/page`).
    pub fn builder(target: &str) -> ShopifySpiderBuilder {
        ShopifySpiderBuilder::new(target)
    }

    /// Extract the host from a domain or URL string.
    fn host_of(target: &str) -> String {
        let candidate = if target.contains("://") {
            target.to_string()
        } else {
            format!("https://{}", target.trim_end_matches('/'))
        };
        Url::parse(&candidate)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_else(|| target.trim().to_string())
    }

    fn collections_url(&self, page: u32) -> String {
        format!(
            "https://{}/collections.json?page={page}&limit={PAGE_LIMIT}",
            self.target_website
        )
    }

    fn products_url(&self, handle: &str, page: u32) -> String {
        format!(
            "https://{}/collections/{handle}/products.json?page={page}&limit={PAGE_LIMIT}",
            self.target_website
        )
    }

    fn product_url(&self, handle: &str, product_handle: &str) -> String {
        format!(
            "https://{}/collections/{handle}/products/{product_handle}",
            self.target_website
        )
    }

    /// Current `page` query parameter of a paginated API URL (defaults to 1).
    fn page_of(url: &Url) -> u32 {
        url.query_pairs()
            .find(|(k, _)| k == "page")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(1)
    }

    /// `/collections/{handle}/products.json` → `Some(handle)`.
    fn products_handle(url: &Url) -> Option<String> {
        let mut segments = url.path_segments()?;
        match (segments.next(), segments.next(), segments.next()) {
            (Some("collections"), Some(handle), Some("products.json")) => Some(handle.to_string()),
            _ => None,
        }
    }

    /// Handle a `/collections.json` page: request every non-empty
    /// collection's first products page, plus the next collections page.
    fn parse_collections(&self, json: &serde_json::Value, page: u32) -> Vec<SpiderRequest> {
        let mut requests = Vec::new();
        let collections = json["collections"].as_array().cloned().unwrap_or_default();
        if collections.is_empty() {
            return requests;
        }
        for collection in &collections {
            let has_products = collection["products_count"].as_i64().unwrap_or(0) > 0
                || collection["products_count"].as_f64().unwrap_or(0.0) > 0.0;
            if let (Some(handle), true) = (collection["handle"].as_str(), has_products) {
                requests.push(SpiderRequest::new(&self.products_url(handle, 1)));
            }
        }
        requests.push(SpiderRequest::new(&self.collections_url(page + 1)));
        requests
    }

    /// Handle a `products.json` page: one item per unseen variant, plus the
    /// next products page while results keep coming.
    fn parse_products(
        &self,
        json: &serde_json::Value,
        handle: &str,
        page: u32,
    ) -> (Vec<serde_json::Value>, Vec<SpiderRequest>) {
        let products = json["products"].as_array().cloned().unwrap_or_default();
        if products.is_empty() {
            log::debug!("Extracted all products from collection {handle}");
            return (Vec::new(), Vec::new());
        }

        let mut items = Vec::new();
        for product in &products {
            self.process_product(product, handle, &mut items);
        }
        let next = SpiderRequest::new(&self.products_url(handle, page + 1));
        (items, vec![next])
    }

    fn process_product(
        &self,
        product: &serde_json::Value,
        collection_handle: &str,
        items: &mut Vec<serde_json::Value>,
    ) {
        let empty = Vec::new();
        let variants = product["variants"].as_array().unwrap_or(&empty);
        for variant in variants {
            if let Some(id) = variant["id"].as_i64() {
                let mut seen = self.collected_ids.lock().unwrap_or_else(|p| p.into_inner());
                if !seen.insert(id) {
                    continue;
                }
            }

            let product_title = product["title"].as_str().unwrap_or_default();
            let variant_title = variant["title"].as_str().unwrap_or_default();
            let name = if variant_title.is_empty() || variant_title == "Default Title" {
                product_title.to_string()
            } else {
                format!("{product_title} - {variant_title}")
            };

            // "lipstick-sets" -> "Lipstick Sets"
            let category: String = collection_handle
                .split('-')
                .filter(|w| !w.is_empty())
                .map(|w| {
                    let mut chars = w.chars();
                    match chars.next() {
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            // compare_at_price counts only when present and non-zero.
            let old_price = variant["compare_at_price"]
                .as_str()
                .filter(|p| p.parse::<f64>().map(|v| v != 0.0).unwrap_or(false))
                .unwrap_or("");

            let description = product["body_html"]
                .as_str()
                .map(|html| {
                    Selector::from_html(html)
                        .get_all_text(" ", true, &[], None)
                        .to_string()
                })
                .unwrap_or_default();

            items.push(serde_json::json!({
                "name": name,
                "price": variant["price"].as_str().unwrap_or_default(),
                "category": category,
                "brand": product["vendor"].as_str().unwrap_or_default(),
                "identifier": variant["id"],
                "sku": variant["sku"].as_str().unwrap_or_default(),
                "stock": if variant["available"].as_bool().unwrap_or(false) {
                    serde_json::Value::Null
                } else {
                    serde_json::json!(0)
                },
                "image_url": product["images"][0]["src"].as_str().unwrap_or_default(),
                "url": self.product_url(
                    collection_handle,
                    product["handle"].as_str().unwrap_or_default(),
                ),
                "description": description,
                "old_price": old_price,
                "barcode": variant["barcode"].as_str().unwrap_or_default(),
            }));
        }
    }
}

#[async_trait]
impl Spider for ShopifySpider {
    fn name(&self) -> &str {
        &self.name
    }

    fn start_urls(&self) -> Vec<String> {
        vec![self.collections_url(1)]
    }

    fn allowed_domains(&self) -> HashSet<String> {
        self.allowed_domains.clone()
    }

    fn concurrent_requests(&self) -> u32 {
        self.concurrent_requests
    }

    fn download_delay(&self) -> f64 {
        self.download_delay
    }

    fn development_mode(&self) -> bool {
        self.development_mode
    }

    fn robots_txt_obey(&self) -> bool {
        self.robots_txt_obey
    }

    async fn parse(
        &self,
        response: SpiderResponse,
    ) -> (Vec<serde_json::Value>, Vec<SpiderRequest>) {
        let Ok(url) = Url::parse(response.url()) else {
            return (Vec::new(), Vec::new());
        };
        let Ok(json) = response.json() else {
            log::warn!("Shopify endpoint returned non-JSON: {}", response.url());
            return (Vec::new(), Vec::new());
        };
        let page = Self::page_of(&url);

        if url.path() == "/collections.json" {
            return (Vec::new(), self.parse_collections(&json, page));
        }
        if let Some(handle) = Self::products_handle(&url) {
            return self.parse_products(&json, &handle, page);
        }
        (Vec::new(), Vec::new())
    }
}

pub struct ShopifySpiderBuilder {
    spider: ShopifySpider,
}

impl ShopifySpiderBuilder {
    fn new(target: &str) -> Self {
        let host = ShopifySpider::host_of(target);
        Self {
            spider: ShopifySpider {
                name: "shopify".to_string(),
                target_website: host,
                collected_ids: Mutex::new(HashSet::new()),
                allowed_domains: HashSet::new(),
                concurrent_requests: 4,
                download_delay: 0.0,
                development_mode: false,
                robots_txt_obey: false,
            },
        }
    }

    pub fn name(mut self, name: &str) -> Self {
        self.spider.name = name.to_string();
        self
    }

    pub fn allowed_domains<I, S>(mut self, domains: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.spider
            .allowed_domains
            .extend(domains.into_iter().map(|d| d.as_ref().to_string()));
        self
    }

    pub fn concurrent_requests(mut self, n: u32) -> Self {
        self.spider.concurrent_requests = n;
        self
    }

    pub fn download_delay(mut self, secs: f64) -> Self {
        self.spider.download_delay = secs;
        self
    }

    pub fn development_mode(mut self, on: bool) -> Self {
        self.spider.development_mode = on;
        self
    }

    pub fn robots_txt_obey(mut self, on: bool) -> Self {
        self.spider.robots_txt_obey = on;
        self
    }

    pub fn build(self) -> ShopifySpider {
        self.spider
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_normalization() {
        assert_eq!(ShopifySpider::host_of("example.com"), "example.com");
        assert_eq!(
            ShopifySpider::host_of("https://shop.example.com"),
            "shop.example.com"
        );
        assert_eq!(
            ShopifySpider::host_of("http://shop.example.com/some/page?x=1"),
            "shop.example.com"
        );
        assert_eq!(ShopifySpider::host_of("example.com/"), "example.com");
    }

    #[test]
    fn url_shapes() {
        let spider = ShopifySpider::builder("example.com").build();
        assert_eq!(
            spider.collections_url(1),
            "https://example.com/collections.json?page=1&limit=250"
        );
        assert_eq!(
            spider.products_url("lipsticks", 2),
            "https://example.com/collections/lipsticks/products.json?page=2&limit=250"
        );

        let url =
            Url::parse("https://example.com/collections/lipsticks/products.json?page=3&limit=250")
                .unwrap();
        assert_eq!(
            ShopifySpider::products_handle(&url).as_deref(),
            Some("lipsticks")
        );
        assert_eq!(ShopifySpider::page_of(&url), 3);

        let url = Url::parse("https://example.com/collections.json?page=2&limit=250").unwrap();
        assert_eq!(ShopifySpider::products_handle(&url), None);
        assert_eq!(ShopifySpider::page_of(&url), 2);
    }
}
