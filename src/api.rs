use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use crate::models::{QueryRequest, QueryResponse};
use crate::rag::RAGSystem;
use crate::AppState;

pub async fn handle_query(
    State(state): State<AppState>,
    Json(request): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, impl IntoResponse> {
    let rag = RAGSystem::new(state.db.clone(), state.openrouter_api_key.clone());
    
    match rag.query(&request.query, request.web_search_enabled).await {
        Ok((answer, sources)) => {
            Ok(Json(QueryResponse { answer, sources }))
        }
        Err(e) => {
            tracing::error!("Query error: {}", e);
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
