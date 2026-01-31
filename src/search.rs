use anyhow::Result;
use scraper::{Html, Selector};
use serde::Deserialize;
use std::env;
use crate::db::Database;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[async_trait::async_trait]
pub trait SearchProvider: Send + Sync {
    async fn search(&self, db: &Database, query: &str) -> Result<Vec<SearchResult>>;
    fn name(&self) -> &str;
}

pub struct DuckDuckGoSearch;

#[async_trait::async_trait]
impl SearchProvider for DuckDuckGoSearch {
    fn name(&self) -> &str {
        "DuckDuckGo"
    }

    async fn search(&self, _db: &Database, query: &str) -> Result<Vec<SearchResult>> {
        let url = format!("https://html.duckduckgo.com/html/?q={}", 
            urlencoding::encode(query));
        
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()?;
        
        let html = client.get(&url).send().await?.text().await?;
        let document = Html::parse_document(&html);
        
        let result_selector = Selector::parse(".result").unwrap();
        let title_selector = Selector::parse(".result__a").unwrap();
        let snippet_selector = Selector::parse(".result__snippet").unwrap();
        
        let mut results = Vec::new();
        
        for result in document.select(&result_selector).take(5) {
            if let Some(title_elem) = result.select(&title_selector).next() {
                let title = title_elem.text().collect::<String>();
                let mut url = title_elem.value().attr("href")
                    .unwrap_or("")
                    .to_string();
                
                if url.starts_with("/l/?uddg=") {
                    if let Some(decoded) = url.strip_prefix("/l/?uddg=") {
                        if let Ok(decoded_url) = urlencoding::decode(decoded) {
                            url = decoded_url.to_string();
                        }
                    }
                }
                
                if url.starts_with("//") {
                    url = format!("https:{}", url);
                }
                
                if url.is_empty() || url.starts_with('/') || (!url.starts_with("http://") && !url.starts_with("https://")) {
                    continue;
                }
                
                let snippet = result.select(&snippet_selector)
                    .next()
                    .map(|e| e.text().collect::<String>())
                    .unwrap_or_default();
                
                if !title.is_empty() {
                    results.push(SearchResult {
                        title,
                        url,
                        snippet,
                    });
                }
            }
        }
        
        Ok(results)
    }
}

pub struct BraveSearch {
    api_key: String,
}

#[derive(Deserialize)]
struct BraveResponse {
    web: BraveWeb,
}

#[derive(Deserialize)]
struct BraveWeb {
    results: Vec<BraveResult>,
}

#[derive(Deserialize)]
struct BraveResult {
    title: String,
    url: String,
    description: Option<String>,
}

#[async_trait::async_trait]
impl SearchProvider for BraveSearch {
    fn name(&self) -> &str {
        "Brave Search"
    }

    async fn search(&self, db: &Database, query: &str) -> Result<Vec<SearchResult>> {
        // Check rate limit (cost 1)
        if !db.check_search_rate_limit("search:brave", 1).await? {
            return Err(anyhow::anyhow!("Brave Search rate limit exceeded"));
        }

        let client = reqwest::Client::new();
        let response = client
            .get("https://api.search.brave.com/res/v1/web/search")
            .query(&[("q", query), ("count", "5")])
            .header("X-Subscription-Token", &self.api_key)
            .header("Accept", "application/json")
            .send()
            .await?;

        // Parse headers for rate limits
        let remaining_header = response.headers().get("x-ratelimit-remaining")
            .and_then(|h| h.to_str().ok());
        let limit_header = response.headers().get("x-ratelimit-limit")
            .and_then(|h| h.to_str().ok());
            
        if let (Some(rem_str), Some(lim_str)) = (remaining_header, limit_header) {
            // Format: "burst, month" e.g., "1, 2000"
            // We want the month part (second value)
            let rem_parts: Vec<&str> = rem_str.split(',').map(|s| s.trim()).collect();
            let lim_parts: Vec<&str> = lim_str.split(',').map(|s| s.trim()).collect();
            
            if rem_parts.len() >= 2 && lim_parts.len() >= 2 {
                if let (Ok(rem_month), Ok(lim_month)) = (rem_parts[1].parse::<i64>(), lim_parts[1].parse::<i64>()) {
                    let used_month = lim_month.saturating_sub(rem_month);
                    let _ = db.update_search_limits("search:brave", Some(used_month), Some(lim_month), None).await;
                }
            }
        }

        if !response.status().is_success() {
             return Err(anyhow::anyhow!("Brave Search API error: {}", response.status()));
        }

        let brave_resp: BraveResponse = response.json().await?;
        
        let results = brave_resp.web.results.into_iter().map(|r| SearchResult {
            title: r.title,
            url: r.url,
            snippet: r.description.unwrap_or_default(),
        }).collect();

        Ok(results)
    }
}

pub struct TavilySearch {
    api_key: String,
}

#[derive(Deserialize)]
struct TavilyResponse {
    results: Vec<TavilyResult>,
}

#[derive(Deserialize)]
struct TavilyResult {
    title: String,
    url: String,
    content: String,
}

#[async_trait::async_trait]
impl SearchProvider for TavilySearch {
    fn name(&self) -> &str {
        "Tavily"
    }

    async fn search(&self, db: &Database, query: &str) -> Result<Vec<SearchResult>> {
        // Check rate limit (cost 1 for basic search)
        if !db.check_search_rate_limit("search:tavily", 1).await? {
            return Err(anyhow::anyhow!("Tavily rate limit exceeded"));
        }

        let client = reqwest::Client::new();
        let response = client
            .post("https://api.tavily.com/search")
            .json(&serde_json::json!({
                "api_key": self.api_key,
                "query": query,
                "search_depth": "basic",
                "max_results": 5
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Tavily API error: {}", response.status()));
        }

        let tavily_resp: TavilyResponse = response.json().await?;

        let results = tavily_resp.results.into_iter().map(|r| SearchResult {
            title: r.title,
            url: r.url,
            snippet: r.content, // Tavily returns 'content' which is a snippet
        }).collect();

        Ok(results)
    }
}

pub struct SearXNGSearch {
    base_url: String,
}

#[derive(Deserialize)]
struct SearXNGResponse {
    results: Vec<SearXNGResult>,
}

#[derive(Deserialize)]
struct SearXNGResult {
    title: String,
    url: String,
    content: Option<String>,
}

#[async_trait::async_trait]
impl SearchProvider for SearXNGSearch {
    fn name(&self) -> &str {
        "SearXNG"
    }

    async fn search(&self, _db: &Database, query: &str) -> Result<Vec<SearchResult>> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
            
        let base = self.base_url.trim_end_matches('/');
        let url = if base.ends_with("/search") {
            base.to_string()
        } else {
            format!("{}/search", base)
        };
        
        tracing::debug!("SearXNG URL: {}", url);
        
        let response = client
            .get(&url)
            .query(&[("q", query), ("format", "json")])
            // Add headers to satisfy SearXNG bot detection
            .header("X-Forwarded-For", "127.0.0.1") 
            .header("User-Agent", "w9-search/1.0")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            tracing::warn!("SearXNG API error: {} - Body: {}", status, text);
            return Err(anyhow::anyhow!("SearXNG API error: {}", status));
        }

        let text = response.text().await?;
        tracing::debug!("SearXNG response: {}", text.chars().take(200).collect::<String>());
        
        let searx_resp: SearXNGResponse = serde_json::from_str(&text).map_err(|e| {
            tracing::error!("Failed to parse SearXNG response: {}", e);
            e
        })?;

        let results = searx_resp.results.into_iter().map(|r| SearchResult {
            title: r.title,
            url: r.url,
            snippet: r.content.unwrap_or_default(),
        }).collect();

        Ok(results)
    }
}

pub struct WebSearch;

impl WebSearch {
    pub async fn get_provider(name: Option<&str>) -> Box<dyn SearchProvider> {
        // If a specific provider is requested, try to use it if configured
        if let Some(n) = name {
            match n.to_lowercase().as_str() {
                "searxng" => {
                    if let Ok(url) = env::var("SEARXNG_BASE_URL") {
                        if !url.is_empty() {
                            return Box::new(SearXNGSearch { base_url: url });
                        }
                    }
                },
                "tavily" => {
                    if let Ok(key) = env::var("TAVILY_API_KEY") {
                        if !key.is_empty() {
                            return Box::new(TavilySearch { api_key: key });
                        }
                    }
                },
                "brave" => {
                    if let Ok(key) = env::var("BRAVE_API_KEY") {
                        if !key.is_empty() {
                            return Box::new(BraveSearch { api_key: key });
                        }
                    }
                },
                "duckduckgo" | "ddg" => return Box::new(DuckDuckGoSearch),
                _ => {} // Fall through to auto
            }
        }

        // Auto logic (Priority: SearXNG -> Tavily -> Brave -> DDG)
        if let Ok(url) = env::var("SEARXNG_BASE_URL") {
            if !url.is_empty() {
                return Box::new(SearXNGSearch { base_url: url });
            }
        }

        if let Ok(key) = env::var("TAVILY_API_KEY") {
            if !key.is_empty() {
                return Box::new(TavilySearch { api_key: key });
            }
        }
        
        if let Ok(key) = env::var("BRAVE_API_KEY") {
             if !key.is_empty() {
                return Box::new(BraveSearch { api_key: key });
            }
        }
        
        Box::new(DuckDuckGoSearch)
    }


    pub async fn search(db: &Database, query: &str, provider: Option<&str>) -> Result<Vec<SearchResult>> {
        let provider = Self::get_provider(provider).await;
        tracing::info!("Using search provider: {}", provider.name());
        provider.search(db, query).await
    }
    
    pub async fn sync_tavily_usage(db: &Database) -> Result<()> {
        if let Ok(key) = env::var("TAVILY_API_KEY") {
            if key.is_empty() { return Ok(()); }
            
            tracing::info!("Syncing Tavily usage...");
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?;
            
            let response = client.get("https://api.tavily.com/usage")
                .header("Authorization", format!("Bearer {}", key))
                .send()
                .await?;
                
            if response.status().is_success() {
                let json: serde_json::Value = response.json().await?;
                // Response body: { "key": { "usage": 150, "limit": 1000, ... } }
                if let Some(k) = json.get("key") {
                    let usage = k.get("usage").and_then(|v| v.as_i64());
                    let limit = k.get("limit").and_then(|v| v.as_i64());
                    
                    if let (Some(u), Some(l)) = (usage, limit) {
                        tracing::info!("Tavily usage: {}/{}", u, l);
                        db.update_search_limits("search:tavily", Some(u), Some(l), None).await?;
                    }
                }
            } else {
                tracing::warn!("Failed to sync Tavily usage: {}", response.status());
            }
        }
        Ok(())
    }
    
    pub async fn fetch_content(url: &str) -> Result<String> {
        let normalized_url = if url.starts_with("//") {
            format!("https:{}", url)
        } else if url.starts_with('/') {
            return Err(anyhow::anyhow!("Relative URL not supported: {}", url));
        } else if !url.starts_with("http://") && !url.starts_with("https://") {
            format!("https://{}", url)
        } else {
            url.to_string()
        };
        
        tracing::debug!("Fetching content from: {}", normalized_url);
        
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        
        let html = client.get(&normalized_url).send().await?.text().await?;
        let document = Html::parse_document(&html);
        
        // Positive selection: Look for article-like containers
        let main_selectors = ["article", "main", "#content", ".content", "#main", ".main", "body"];
        let mut best_root = document.root_element();
        
        for selector_str in main_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(elem) = document.select(&selector).next() {
                    best_root = elem;
                    break;
                }
            }
        }

        // Extraction Heuristics
        // We look for P tags and other text blocks.
        // We score them: +1 for text length, -1 for link density.
        
        let p_selector = Selector::parse("p, h1, h2, h3, h4, h5, h6, li, blockquote, div").unwrap();
        let link_selector = Selector::parse("a").unwrap();
        
        let mut extracted_blocks = Vec::new();
        
        for element in best_root.select(&p_selector) {
            let text = element.text().collect::<String>();
            let text_len = text.len();
            
            // Skip very short blocks
            if text_len < 30 { continue; }
            
            // Calculate link density
            let mut link_text_len = 0;
            for link in element.select(&link_selector) {
                link_text_len += link.text().collect::<String>().len();
            }
            
            let link_density = if text_len > 0 {
                link_text_len as f64 / text_len as f64
            } else {
                1.0
            };
            
            // Heuristic: If > 50% of the text is links, it's likely a navbar or footer list
            if link_density > 0.5 { continue; }
            
            // Heuristic: Check for class names that indicate noise
            if let Some(class_attr) = element.value().attr("class") {
                let lower = class_attr.to_lowercase();
                if lower.contains("menu") || lower.contains("nav") || lower.contains("footer") || lower.contains("copyright") {
                    continue;
                }
            }

            extracted_blocks.push(text.trim().to_string());
        }

        // Fallback: If we got nothing, try raw text from body
        if extracted_blocks.is_empty() {
             let body_text = best_root.text().collect::<String>();
             if body_text.len() > 100 {
                 extracted_blocks.push(body_text);
             }
        }
        
        // Join and clean
        let mut content = extracted_blocks.join("\n\n");
        
        // Limit length safely
        if content.len() > 15000 {
            let mut limit = 15000;
            while !content.is_char_boundary(limit) {
                limit -= 1;
            }
            content.truncate(limit);
        }
        
        Ok(content)
    }
}
