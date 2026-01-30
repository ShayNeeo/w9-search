use axum::{
    extract::State, 
    http::StatusCode, 
    response::{IntoResponse, sse::{Event, Sse}}, 
    Json
};
use futures::stream::Stream;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use std::convert::Infallible;
use std::time::Duration;

use crate::models::{QueryRequest, QueryResponse};
use crate::rag::{RAGSystem, StreamEvent};
use crate::AppState;
use crate::search::WebSearch;

pub async fn handle_query_stream(
    State(state): State<AppState>,
    Json(request): Json<QueryRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    tracing::info!(
        "Received streaming query: '{}' (web_search: {}, model: {:?})",
        request.query,
        request.web_search_enabled,
        request.model
    );

    let (tx, rx) = mpsc::channel(100);
    
    // Spawn background task to run the query
    tokio::spawn(async move {
        // Determine model to use
        let requested_model = request.model.clone().unwrap_or_else(|| "auto".to_string());
        
        let model = if requested_model == "auto" {
            // Smart auto-selection
            let models = state.llm_manager.get_models().await;
            
            // Priority list of "smart" models
            let priority_patterns = [
                "deepseek-r1",
                "llama-3.3-70b",
                "qwen-2.5-72b", 
                "mixtral-8x22b",
                "claude-3-opus",
                "gpt-4"
            ];
            
            let mut selected = None;
            for pattern in priority_patterns {
                if let Some(m) = models.iter().find(|m| m.id.to_lowercase().contains(pattern)) {
                    selected = Some(m.id.clone());
                    break;
                }
            }
            
            // Fallback to default if no smart model found
            selected.unwrap_or(state.default_model.clone())
        } else if state.llm_manager.get_model(&requested_model).await.is_some() {
            requested_model
        } else {
             tracing::warn!(
                "Requested model '{}' not found; using default '{}'",
                requested_model,
                state.default_model
            );
            state.default_model.clone()
        };

        let search_provider = request.search_provider
            .filter(|s| s != "auto");

        tracing::info!("Using model '{}' and search provider '{:?}'", model, search_provider);
        let _ = tx.send(Ok(StreamEvent::Status(format!("Using model: {}", model)))).await;

        let rag = RAGSystem::new(state.db.clone(), state.llm_manager.clone(), model, search_provider);
        
        match rag.query(&request.query, request.web_search_enabled, Some(tx.clone())).await {
            Ok((answer, _)) => {
                let _ = tx.send(Ok(StreamEvent::Answer(answer))).await;
            }
            Err(e) => {
                tracing::error!("Query error: {}", e);
                let _ = tx.send(Ok(StreamEvent::Error(e.to_string()))).await;
            }
        }
        
        let _ = tx.send(Ok(StreamEvent::Done)).await;
    });

    // Create stream from channel
    let stream = ReceiverStream::new(rx).map(|result| {
        match result {
            Ok(event) => {
                Ok(Event::default()
                    .json_data(event)
                    .unwrap_or_else(|_| Event::default().data("Serialization error")))
            },
            Err(_) => Ok(Event::default().event("error").data("Internal channel error")),
        }
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(10)))
}

pub async fn handle_query(
    State(state): State<AppState>,
    Json(request): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, impl IntoResponse> {
    tracing::info!(
        "Received query: '{}' (web_search: {}, model: {:?})",
        request.query,
        request.web_search_enabled,
        request.model
    );
    
    // Determine model to use
    let requested_model = request.model.clone().unwrap_or_else(|| state.default_model.clone());
    
    // Verify model exists in manager
    let model = if state.llm_manager.get_model(&requested_model).await.is_some() {
        requested_model
    } else {
        tracing::warn!(
            "Requested model '{}' not found; using default '{}'",
            requested_model,
            state.default_model
        );
        state.default_model.clone()
    };
    
    let search_provider = request.search_provider
        .filter(|s| s != "auto");

    tracing::info!("Using model '{}' and search provider '{:?}' for this query", model, search_provider);
    
    // Pass llm_manager instead of api_key
    let rag = RAGSystem::new(state.db.clone(), state.llm_manager.clone(), model, search_provider);
    
    match rag.query(&request.query, request.web_search_enabled, None).await {
        Ok((answer, sources)) => {
            tracing::info!("Query successful, answer length: {}, sources: {}", answer.len(), sources.len());
            Ok(Json(QueryResponse { answer, sources }))
        }
        Err(e) => {
            tracing::error!("Query error: {}", e);
            tracing::error!("Error chain: {:?}", e);
            eprintln!("Query error: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error: {}", e),
            ))
        }
    }
}

pub async fn get_sources(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::models::Source>>, impl IntoResponse> {
    match state.db.get_sources(20).await {
        Ok(sources) => Ok(Json(sources)),
        Err(e) => {
            tracing::error!("Get sources error: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Error: {}", e),
            ))
        }
    }
}

pub async fn sync_limits(
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Sync Tavily
    if let Err(e) = WebSearch::sync_tavily_usage(&state.db).await {
        tracing::error!("Sync Tavily limits error: {}", e);
    }
    
    // Sync LLM Providers (OpenRouter, Pollinations, etc.)
    if let Err(e) = state.llm_manager.refresh_llm_limits().await {
        tracing::error!("Sync LLM limits error: {}", e);
    }
    
    StatusCode::OK
}
