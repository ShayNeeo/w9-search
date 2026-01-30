use axum::{extract::State, response::Html};
use maud::{html, Markup, DOCTYPE};
use crate::AppState;

pub async fn index(State(state): State<AppState>) -> Html<String> {
    // Fetch models dynamically from LLMManager
    let mut models = state.llm_manager.get_models().await;
    
    // Sort models by provider, then name
    models.sort_by(|a, b| {
        let provider_cmp = a.provider.as_str().cmp(b.provider.as_str());
        if provider_cmp == std::cmp::Ordering::Equal {
            a.name.cmp(&b.name)
        } else {
            provider_cmp
        }
    });

    let markup: Markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "W9 Search - AI RAG" }
                script src="https://unpkg.com/htmx.org@1.9.10" {}
                script src="https://cdn.jsdelivr.net/npm/marked@11.1.1/marked.min.js" {}
                script src="https://cdn.jsdelivr.net/npm/mermaid@10.6.1/dist/mermaid.min.js" {}
                link rel="stylesheet" href="/static/style.css";
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link href=(r#"https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@300;400;700&family=Space+Grotesk:wght@300;400;700&display=swap"#) rel="stylesheet";
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

                                    div class="model-select" {
                                        label for="model-select" { "Model" }
                                        select id="model-select" name="model" {
                                            @for model in &models {
                                                option value=(model.id) { (format!("{} ({})", model.name, model.provider)) }
                                            }
                                        }
                                    }
                                    
                                    button type="button" id="sync-btn" class="secondary-btn" { "Sync Limits" }
                                    
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
                    (maud::PreEscaped(r#"
                    // Initialize Mermaid
                    mermaid.initialize({ 
                        startOnLoad: false,
                        theme: 'dark',
                        themeVariables: {
                            primaryColor: '#ff6b9d',
                            primaryTextColor: '#e0e0e0',
                            primaryBorderColor: '#4ecdc4',
                            lineColor: '#4ecdc4',
                            secondaryColor: '#141414',
                            tertiaryColor: '#0a0a0a'
                        }
                    });
                    
                    // Configure Marked for markdown rendering
                    marked.setOptions({
                        breaks: true,
                        gfm: true,
                        headerIds: true,
                        mangle: false
                    });
                    
                    function renderMarkdown(markdown) {
                        // Render markdown to HTML
                        const html = marked.parse(markdown);
                        
                        // Create a temporary container to process mermaid diagrams
                        const tempDiv = document.createElement('div');
                        tempDiv.innerHTML = html;
                        
                        // Find and process mermaid code blocks
                        const mermaidBlocks = tempDiv.querySelectorAll('code.language-mermaid, pre code.language-mermaid');
                        mermaidBlocks.forEach((block, index) => {
                            const mermaidCode = block.textContent;
                            const mermaidId = 'mermaid-' + Date.now() + '-' + index;
                            
                            // Create mermaid div
                            const mermaidDiv = document.createElement('div');
                            mermaidDiv.className = 'mermaid';
                            mermaidDiv.id = mermaidId;
                            mermaidDiv.textContent = mermaidCode;
                            
                            // Replace code block with mermaid div
                            const pre = block.closest('pre');
                            if (pre) {
                                pre.parentNode.replaceChild(mermaidDiv, pre);
                            } else {
                                block.parentNode.replaceChild(mermaidDiv, block);
                            }
                        });
                        
                        return tempDiv.innerHTML;
                    }
                    
                    function renderMermaid(container) {
                        // Find all mermaid divs and render them
                        const mermaidDivs = container.querySelectorAll('.mermaid');
                        mermaidDivs.forEach((div) => {
                            if (!div.hasAttribute('data-processed')) {
                                mermaid.run({ nodes: [div] });
                                div.setAttribute('data-processed', 'true');
                            }
                        });
                    }
                    
                    document.getElementById('sync-btn').addEventListener('click', async () => {
                        const btn = document.getElementById('sync-btn');
                        btn.disabled = true;
                        btn.textContent = 'Syncing...';
                        try {
                            const response = await fetch('/api/sync', { method: 'POST' });
                            if (response.ok) {
                                btn.textContent = 'Synced!';
                                setTimeout(() => { btn.textContent = 'Sync Limits'; btn.disabled = false; }, 2000);
                            } else {
                                btn.textContent = 'Failed';
                                setTimeout(() => { btn.textContent = 'Sync Limits'; btn.disabled = false; }, 2000);
                            }
                        } catch (e) {
                            console.error(e);
                            btn.textContent = 'Error';
                            setTimeout(() => { btn.textContent = 'Sync Limits'; btn.disabled = false; }, 2000);
                        }
                    });
                    
                    document.getElementById('query-form').addEventListener('submit', async (e) => {
                        e.preventDefault();
                        const query = document.getElementById('query-input').value;
                        const webSearchEnabled = document.getElementById('web-search-toggle').checked;
                        const modelSelect = document.getElementById('model-select');
                        const model = modelSelect ? modelSelect.value : null;
                        
                        const answerSection = document.getElementById('answer-section');
                        const sourcesSection = document.getElementById('sources-section');
                        
                        answerSection.innerHTML = '<div class="loading">Processing...</div>';
                        sourcesSection.innerHTML = '';
                        
                        try {
                            const response = await fetch('/api/query', {
                                method: 'POST',
                                headers: { 'Content-Type': 'application/json' },
                                body: JSON.stringify({ query, web_search_enabled: webSearchEnabled, model })
                            });
                            
                            const data = await response.json();
                            
                            // Render markdown with mermaid support
                            const markdownHtml = renderMarkdown(data.answer);
                            answerSection.innerHTML = `<div class="answer markdown-body">${markdownHtml}</div>`;
                            
                            // Render mermaid diagrams after markdown is rendered
                            setTimeout(() => {
                                renderMermaid(answerSection);
                            }, 100);
                            
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
                    "#))
                }
            }
        }
    };
    Html(markup.into_string())
}
