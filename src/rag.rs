use crate::models::{OpenRouterMessage, OpenRouterRequest, OpenRouterResponse};
use crate::search::WebSearch;
use crate::db::Database;
use anyhow::Result;
use std::sync::Arc;

pub struct RAGSystem {
    db: Arc<Database>,
    api_key: String,
    model: String,
}

impl RAGSystem {
    pub fn new(db: Arc<Database>, api_key: String) -> Self {
        Self {
            db,
            api_key,
            model: "tngtech/deepseek-r1t2-chimera:free".to_string(),
        }
    }

    pub async fn query(&self, user_query: &str, web_search_enabled: bool) -> Result<(String, Vec<crate::models::Source>)> {
        let mut context_sources = Vec::new();
        
        // Step 1: Web search if enabled
        if web_search_enabled {
            let search_results = WebSearch::search(user_query).await?;
            
            for result in search_results.iter().take(3) {
                match WebSearch::fetch_content(&result.url).await {
                    Ok(content) => {
                        let id = self.db.insert_source(
                            &result.url,
                            &result.title,
                            &content,
                        ).await?;
                        
                        context_sources.push(crate::models::Source {
                            id,
                            url: result.url.clone(),
                            title: result.title.clone(),
                            content,
                            created_at: chrono::Utc::now(),
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch {}: {}", result.url, e);
                    }
                }
            }
        }
        
        // Step 2: Retrieve relevant sources from database
        let db_sources = self.db.search_sources(user_query, 5).await?;
        context_sources.extend(db_sources);
        
        // Step 3: Build context
        let context = if context_sources.is_empty() {
            "No relevant sources found.".to_string()
        } else {
            context_sources.iter()
                .enumerate()
                .map(|(i, s)| {
                    format!("[Source {}]\nTitle: {}\nURL: {}\nContent: {}\n", 
                        i + 1, s.title, s.url, 
                        s.content.chars().take(1000).collect::<String>())
                })
                .collect::<Vec<_>>()
                .join("\n---\n\n")
        };
        
        // Step 4: Query AI with RAG context
        let system_prompt = if web_search_enabled {
            format!(
                "You are a helpful AI assistant with access to real-time web sources. \
                CRITICAL: You MUST prioritize and rely EXCLUSIVELY on the provided web sources below. \
                Do NOT use your training knowledge or general information - ONLY use information from the sources provided. \
                If the sources do not contain enough information to answer the question, say so explicitly. \
                Always cite your sources using [Source N] format when referencing any information. \
                Base your answer STRICTLY on the provided sources, even if it contradicts your training data.\n\n\
                Web Sources (use these exclusively):\n{}",
                context
            )
        } else {
            format!(
                "You are a helpful AI assistant with access to stored sources. \
                Use the provided sources to answer questions accurately when available. \
                You may supplement with your knowledge if sources don't fully cover the question. \
                Cite sources using [Source N] format when referencing them.\n\n\
                Sources:\n{}",
                context
            )
        };
        
        let messages = vec![
            OpenRouterMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
            OpenRouterMessage {
                role: "user".to_string(),
                content: user_query.to_string(),
            },
        ];
        
        let request = OpenRouterRequest {
            model: self.model.clone(),
            messages,
            tools: None,
        };
        
        let client = reqwest::Client::new();
        let response = client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "http://localhost:3000")
            .header("X-Title", "W9 Search")
            .json(&request)
            .send()
            .await?;
        
        let openrouter_response: OpenRouterResponse = response.json().await?;
        
        let answer = openrouter_response
            .choices
            .first()
            .and_then(|c| Some(c.message.content.clone()))
            .unwrap_or_else(|| "Sorry, I couldn't generate a response.".to_string());
        
        Ok((answer, context_sources))
    }
}
