use anyhow::Result;
use scraper::{Html, Selector};

pub struct WebSearch;

impl WebSearch {
    pub async fn search(query: &str) -> Result<Vec<SearchResult>> {
        // Using DuckDuckGo HTML search (no API key needed)
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
                let url = title_elem.value().attr("href")
                    .unwrap_or("")
                    .to_string();
                
                let snippet = result.select(&snippet_selector)
                    .next()
                    .map(|e| e.text().collect::<String>())
                    .unwrap_or_default();
                
                if !url.is_empty() && !title.is_empty() {
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
    
    pub async fn fetch_content(url: &str) -> Result<String> {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        
        let html = client.get(url).send().await?.text().await?;
        let document = Html::parse_document(&html);
        
        // Extract text content
        let body_selector = Selector::parse("body").unwrap();
        let p_selector = Selector::parse("p").unwrap();
        
        let mut content = String::new();
        
        if let Some(body) = document.select(&body_selector).next() {
            for p in body.select(&p_selector) {
                let text = p.text().collect::<String>();
                if text.len() > 20 {
                    content.push_str(&text);
                    content.push_str("\n\n");
                }
            }
        }
        
        // Fallback to all text if no paragraphs
        if content.is_empty() {
            content = document.root_element().text().collect::<String>();
        }
        
        // Limit content length
        if content.len() > 5000 {
            content.truncate(5000);
        }
        
        Ok(content)
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}
