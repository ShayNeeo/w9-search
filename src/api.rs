use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use crate::models::{QueryRequest, QueryResponse};
use crate::rag::RAGSystem;
use crate::AppState;
use crate::search::WebSearch;

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
    
    match rag.query(&request.query, request.web_search_enabled).await {
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
