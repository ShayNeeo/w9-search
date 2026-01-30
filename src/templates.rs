use axum::response::Html;
use maud::{html, Markup, DOCTYPE};

pub async fn index() -> Html<String> {
    let markup: Markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "W9 Search - AI RAG" }
                script src="https://unpkg.com/htmx.org@1.9.10" {}
                link rel="stylesheet" href="/static/style.css";
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@300;400;700&family=Space+Grotesk:wght@300;400;700&display=swap" rel="stylesheet";
            }
            body {
                div class="container" {
                    header {
                        h1 { "W9" }
                        p class="subtitle" { "Search" }
                    }
                    
                    div class="search-section" {
                        form id="query-form" {
                            div class="input-group" {
                                textarea 
                                    id="query-input"
                                    name="query"
                                    placeholder="Ask anything..."
                                    rows="3"
                                    required {}
                                {}
                                
                                div class="controls" {
                                    label class="toggle-switch" {
                                        input 
                                            type="checkbox" 
                                            id="web-search-toggle"
                                            checked {}
                                        {}
                                        span class="slider" {}
                                        span class="toggle-label" { "WebSearch" }
                                    }
                                    
                                    button type="submit" class="submit-btn" {
                                        "Query"
                                    }
                                }
                            }
                        }
                    }
                    
                    div id="answer-section" class="answer-section" {}
                    div id="sources-section" class="sources-section" {}
                }
                
                script {
                    r#"
                    document.getElementById('query-form').addEventListener('submit', async (e) => {
                        e.preventDefault();
                        const query = document.getElementById('query-input').value;
                        const webSearchEnabled = document.getElementById('web-search-toggle').checked;
                        
                        const answerSection = document.getElementById('answer-section');
                        const sourcesSection = document.getElementById('sources-section');
                        
                        answerSection.innerHTML = '<div class="loading">Processing...</div>';
                        sourcesSection.innerHTML = '';
                        
                        try {
                            const response = await fetch('/api/query', {
                                method: 'POST',
                                headers: { 'Content-Type': 'application/json' },
                                body: JSON.stringify({ query, web_search_enabled: webSearchEnabled })
                            });
                            
                            const data = await response.json();
                            
                            answerSection.innerHTML = `<div class="answer">${data.answer.replace(/\n/g, '<br>')}</div>`;
                            
                            if (data.sources && data.sources.length > 0) {
                                const sourcesHtml = data.sources.map((s, i) => 
                                    `<div class="source-item">
                                        <div class="source-number">${i + 1}</div>
                                        <div class="source-content">
                                            <a href="${s.url}" target="_blank" class="source-title">${s.title}</a>
                                            <div class="source-url">${s.url}</div>
                                        </div>
                                    </div>`
                                ).join('');
                                sourcesSection.innerHTML = `<h3>Sources</h3>${sourcesHtml}`;
                            }
                        } catch (error) {
                            answerSection.innerHTML = `<div class="error">Error: ${error.message}</div>`;
                        }
                    });
                    "#
                }
            }
        }
    };
    Html(markup.into_string())
}
