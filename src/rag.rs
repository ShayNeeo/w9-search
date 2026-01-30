use crate::search::WebSearch;
use crate::db::Database;
use crate::tools::Tools;
use anyhow::Result;
use std::sync::Arc;
use serde_json::{json, Value};

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
    
    /// Enhance search query with temporal context for time-sensitive queries
    fn enhance_query_with_temporal_context(query: &str) -> String {
        let query_lower = query.to_lowercase();
        
        // Check if query is time-sensitive
        let time_sensitive_keywords = [
            "current", "today", "now", "present", "latest", "recent", 
            "who is", "what is the current", "who are the current",
            "president", "leader", "ceo", "chairman", "minister",
            "happened today", "news", "breaking", "update"
        ];
        
        // Check for comparison queries that might need calculation
        let needs_calculation = query_lower.contains("compare") || 
            query_lower.contains("difference") ||
            query_lower.contains("larger") ||
            query_lower.contains("smaller") ||
            query_lower.contains("more than") ||
            query_lower.contains("less than");
        
        // Check for unit conversion queries
        let needs_conversion = query_lower.contains("convert") ||
            query_lower.contains("to ") && (query_lower.contains("km") || 
            query_lower.contains("miles") || query_lower.contains("celsius") ||
            query_lower.contains("fahrenheit") || query_lower.contains("kg") ||
            query_lower.contains("pounds"));
        
        let is_time_sensitive = time_sensitive_keywords.iter()
            .any(|keyword| query_lower.contains(keyword));
        
        if is_time_sensitive {
            // Get current date to add context
            let current_date = chrono::Utc::now().format("%Y-%m-%d").to_string();
            format!("{} as of {}", query, current_date)
        } else if needs_calculation {
            // Add context for calculation queries
            format!("{} (use calculation tools if needed)", query)
        } else if needs_conversion {
            // Add context for conversion queries
            format!("{} (use unit conversion tools if needed)", query)
        } else {
            query.to_string()
        }
    }

    pub async fn query(&self, user_query: &str, web_search_enabled: bool) -> Result<(String, Vec<crate::models::Source>)> {
        tracing::info!("Starting RAG query: '{}' (web_search: {})", user_query, web_search_enabled);
        let mut context_sources = Vec::new();
        
        // Step 1: Web search if enabled
        // Enhance search query with temporal context for time-sensitive queries
        if web_search_enabled {
            let enhanced_query = Self::enhance_query_with_temporal_context(user_query);
            tracing::info!("Enhanced search query: '{}'", enhanced_query);
            
            let search_results = match WebSearch::search(&enhanced_query).await {
                Ok(results) => {
                    tracing::info!("Web search returned {} results", results.len());
                    results
                },
                Err(e) => {
                    tracing::warn!("Web search failed: {}, continuing without web sources", e);
                    Vec::new()
                }
            };
            
            for (idx, result) in search_results.iter().take(3).enumerate() {
                tracing::info!("Fetching content from result {}: {}", idx + 1, result.url);
                match WebSearch::fetch_content(&result.url).await {
                    Ok(content) => {
                        tracing::info!("Fetched {} bytes from {}", content.len(), result.url);
                        match self.db.insert_source(
                            &result.url,
                            &result.title,
                            &content,
                        ).await {
                            Ok(id) => {
                                tracing::info!("Stored source {} in database", id);
                                context_sources.push(crate::models::Source {
                                    id,
                                    url: result.url.clone(),
                                    title: result.title.clone(),
                                    content,
                                    created_at: chrono::Utc::now(),
                                });
                            },
                            Err(e) => {
                                tracing::warn!("Failed to store source {}: {}", result.url, e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch {}: {}", result.url, e);
                    }
                }
            }
        }
        
        // Step 2: Retrieve relevant sources from database
        tracing::info!("Searching database for relevant sources...");
        let db_sources = match self.db.search_sources(user_query, 5).await {
            Ok(sources) => {
                tracing::info!("Found {} relevant sources in database", sources.len());
                sources
            },
            Err(e) => {
                tracing::warn!("Database search failed: {}, continuing without DB sources", e);
                Vec::new()
            }
        };
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
                "You are a helpful AI assistant with access to real-time web sources and powerful tools.\n\n\
                CRITICAL WORKFLOW FOR TIME-SENSITIVE QUERIES:\n\
                1. If the question asks about 'current', 'today', 'now', 'present', or anything requiring up-to-date information:\n\
                   - FIRST, use the get_current_date tool to get the exact current date\n\
                   - THEN, use that date context when evaluating web sources or formulating follow-up searches\n\
                   - Example: 'Who is the current US president?' → Get current date → Search with date context → Answer based on sources\n\
                2. For questions about 'current' status, positions, events, or people:\n\
                   - Always fetch the current date first to establish temporal context\n\
                   - Use the date to determine if sources are recent enough\n\
                   - If sources don't include the current date, note this limitation\n\n\
                INTELLIGENT TOOL USAGE PATTERNS:\n\
                \n\
                TIME-SENSITIVE QUERIES:\n\
                - 'current', 'today', 'now', 'present', 'latest' → get_current_date FIRST\n\
                - 'who is the current X' → get_current_date → search with date context\n\
                - 'what happened today' → get_current_date → search for today's events\n\
                - Age questions → get_current_date → days_between_dates\n\
                \n\
                MATHEMATICAL & COMPARISON QUERIES:\n\
                - Any math expression → calculate tool\n\
                - 'compare', 'difference between', 'larger than' → compare_values\n\
                - 'convert X to Y' → unit_convert\n\
                - 'format as currency/percentage' → format_number\n\
                \n\
                DATA EXTRACTION & ANALYSIS:\n\
                - 'extract', 'find keywords', 'main topics' → extract_keywords\n\
                - 'who/what/where mentioned' → extract_entities\n\
                - 'validate URL', 'check link' → validate_url\n\
                \n\
                DATE & TIME OPERATIONS:\n\
                - 'how many days', 'time between' → days_between_dates\n\
                - 'format date' → format_date\n\
                - 'convert timezone' → timezone_convert\n\
                \n\
                GENERAL GUIDELINES:\n\
                - Always use tools proactively when they can provide accurate, real-time data\n\
                - Combine multiple tools when needed (e.g., get_current_date + days_between_dates for age)\n\
                - Use web search results ONLY - do not rely on training data for current information\n\
                - When in doubt about current information, use get_current_date to establish context\n\n\
                SOURCE PRIORITY:\n\
                - CRITICAL: You MUST prioritize and rely EXCLUSIVELY on the provided web sources below\n\
                - Do NOT use your training knowledge for current/real-time information\n\
                - If sources don't contain enough information, say so explicitly\n\
                - Always cite sources using [Source N] format when referencing any information\n\
                - Base your answer STRICTLY on the provided sources, even if it contradicts your training data\n\n\
                Web Sources (use these exclusively):\n{}",
                context
            )
        } else {
            format!(
                "You are a helpful AI assistant with access to stored sources and powerful tools.\n\n\
                INTELLIGENT TOOL USAGE:\n\
                \n\
                TIME-SENSITIVE QUERIES:\n\
                - 'current', 'today', 'now', 'present' → get_current_date FIRST\n\
                - Age calculations → get_current_date + days_between_dates\n\
                - 'how long ago' → days_between_dates\n\
                \n\
                MATHEMATICAL OPERATIONS:\n\
                - Any calculation → calculate tool\n\
                - Comparisons → compare_values\n\
                - Unit conversions → unit_convert\n\
                - Number formatting → format_number\n\
                \n\
                TEXT ANALYSIS:\n\
                - Extract main points → extract_keywords\n\
                - Find entities (people, places) → extract_entities\n\
                \n\
                GENERAL GUIDELINES:\n\
                - Use tools proactively to get accurate, real-time data\n\
                - Combine tools intelligently (e.g., date + calculation for age)\n\
                - Prioritize stored sources when available\n\
                - You may supplement with your knowledge if sources don't fully cover the question\n\
                - Always cite sources using [Source N] format when referencing them\n\n\
                Sources:\n{}",
                context
            )
        };
        
        let mut messages: Vec<Value> = vec![
            json!({
                "role": "system",
                "content": system_prompt
            }),
            json!({
                "role": "user",
                "content": user_query
            }),
        ];
        
        // Get tools definition
        let tools = Tools::get_tools_definition();
        tracing::info!("Starting AI query with {} tools available", tools.len());
        
        let mut request_json = json!({
            "model": self.model,
            "messages": messages,
            "tools": tools
        });
        
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;
        
        // Handle tool calling loop (max 3 iterations)
        let mut max_iterations = 3;
        let mut final_answer = String::new();
        
        while max_iterations > 0 {
            tracing::info!("AI query iteration {} (remaining: {})", 4 - max_iterations, max_iterations - 1);
            
            let response = match client
                .post("https://openrouter.ai/api/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .header("HTTP-Referer", "http://localhost:3000")
                .header("X-Title", "W9 Search")
                .json(&request_json)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    tracing::info!("OpenRouter API response status: {}", status);
                    if !status.is_success() {
                        let error_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                        tracing::error!("OpenRouter API error ({}): {}", status, error_text);
                        return Err(anyhow::anyhow!("OpenRouter API error: {} - {}", status, error_text));
                    }
                    resp
                },
                Err(e) => {
                    tracing::error!("Failed to send request to OpenRouter: {}", e);
                    return Err(anyhow::anyhow!("OpenRouter request failed: {}", e));
                }
            };
            
            let response_json: Value = match response.json().await {
                Ok(json) => json,
                Err(e) => {
                    tracing::error!("Failed to parse OpenRouter response as JSON: {}", e);
                    return Err(anyhow::anyhow!("Failed to parse API response: {}", e));
                }
            };
            
            tracing::debug!("OpenRouter response: {}", serde_json::to_string_pretty(&response_json).unwrap_or_default());
            
            if let Some(choices) = response_json.get("choices").and_then(|c| c.as_array()) {
                if choices.is_empty() {
                    tracing::warn!("OpenRouter returned empty choices array");
                    max_iterations -= 1;
                    continue;
                }
                
                if let Some(choice) = choices.first() {
                    if let Some(message) = choice.get("message") {
                        // Check if there's content (final answer)
                        if let Some(content) = message.get("content").and_then(|c| c.as_str()) {
                            if !content.is_empty() {
                                tracing::info!("Received final answer from AI (length: {} chars)", content.len());
                                final_answer = content.to_string();
                                break;
                            }
                        }
                        
                        // Check for tool calls
                        if let Some(tool_calls) = message.get("tool_calls").and_then(|tc| tc.as_array()) {
                            if !tool_calls.is_empty() {
                                tracing::info!("AI requested {} tool calls", tool_calls.len());
                                
                                // Execute tools and add responses
                                for (idx, tool_call) in tool_calls.iter().enumerate() {
                                    if let Some(function) = tool_call.get("function") {
                                        let function_name = function.get("name")
                                            .and_then(|n| n.as_str())
                                            .unwrap_or("");
                                        
                                        let arguments_str = function.get("arguments")
                                            .and_then(|a| a.as_str())
                                            .unwrap_or("{}");
                                        
                                        tracing::info!("Tool call {}: {} with args: {}", idx + 1, function_name, arguments_str);
                                        
                                        let arguments: Value = match serde_json::from_str(arguments_str) {
                                            Ok(args) => args,
                                            Err(e) => {
                                                tracing::warn!("Failed to parse tool arguments: {}, using empty object", e);
                                                json!({})
                                            }
                                        };
                                        
                                        let tool_result = match Tools::execute_tool(function_name, &arguments) {
                                            Ok(result) => {
                                                tracing::info!("Tool {} executed successfully, result length: {}", function_name, result.len());
                                                result
                                            },
                                            Err(e) => {
                                                tracing::warn!("Tool {} execution error: {}", function_name, e);
                                                format!("Error executing {}: {}", function_name, e)
                                            }
                                        };
                                        
                                        let tool_call_id = tool_call.get("id")
                                            .and_then(|id| id.as_str())
                                            .unwrap_or("");
                                        
                                        // Add tool response message
                                        messages.push(json!({
                                            "role": "tool",
                                            "content": tool_result,
                                            "tool_call_id": tool_call_id
                                        }));
                                    } else {
                                        tracing::warn!("Tool call {} missing function field", idx + 1);
                                    }
                                }
                                
                                // Add the assistant's tool call message
                                messages.push(message.clone());
                                
                                // Update request for next iteration
                                request_json = json!({
                                    "model": self.model,
                                    "messages": messages,
                                    "tools": tools
                                });
                                
                                tracing::info!("Preparing next iteration with {} messages", messages.len());
                                max_iterations -= 1;
                                continue;
                            }
                        }
                        
                        // Check finish_reason
                        if let Some(finish_reason) = choice.get("finish_reason").and_then(|fr| fr.as_str()) {
                            tracing::info!("AI finished with reason: {}", finish_reason);
                            if finish_reason == "stop" {
                                // Try to get content even if not in message.content
                                if let Some(content) = message.get("content").and_then(|c| c.as_str()) {
                                    if !content.is_empty() {
                                        final_answer = content.to_string();
                                        break;
                                    }
                                }
                            }
                        }
                    } else {
                        tracing::warn!("Choice missing message field");
                    }
                }
            } else {
                tracing::warn!("OpenRouter response missing choices field");
                if let Some(error) = response_json.get("error") {
                    tracing::error!("OpenRouter API error: {}", serde_json::to_string(error).unwrap_or_default());
                    return Err(anyhow::anyhow!("OpenRouter API error: {}", serde_json::to_string(error).unwrap_or_default()));
                }
            }
            
            max_iterations -= 1;
            tracing::warn!("No valid response extracted, remaining iterations: {}", max_iterations);
        }
        
        if final_answer.is_empty() {
            tracing::warn!("No answer generated after {} iterations", 3);
            final_answer = "Sorry, I couldn't generate a response. Please try again.".to_string();
        } else {
            tracing::info!("Successfully generated answer (length: {} chars)", final_answer.len());
        }
        
        Ok((final_answer, context_sources))
    }
}
