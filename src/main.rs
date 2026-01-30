mod api;
mod db;
mod models;
mod rag;
mod search;
mod templates;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use crate::db::Database;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub openrouter_api_key: String,
}

#[tokio::main]
async fn main() {
    // Initialize logging first
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();
    
    // Load .env file (ignore errors if it doesn't exist)
    dotenv::dotenv().ok();
    
    if let Err(e) = run().await {
        eprintln!("Fatal error: {}", e);
        eprintln!("Error chain: {:?}", e);
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    tracing::info!("Starting W9 Search application...");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:w9_search.db".to_string());
    
    tracing::info!("Database URL: {}", database_url);
    
    // Ensure database directory exists if path contains directories
    if let Some(path) = database_url.strip_prefix("sqlite:") {
        if let Some(parent) = std::path::Path::new(path).parent() {
            tracing::info!("Creating database directory: {:?}", parent);
            std::fs::create_dir_all(parent)?;
        }
    }
    
    let openrouter_api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| anyhow::anyhow!("OPENROUTER_API_KEY environment variable is required but not set"))?;
    
    tracing::info!("OpenRouter API key configured (length: {})", openrouter_api_key.len());

    tracing::info!("Connecting to database...");
    let db = Database::new(&database_url).await?;
    tracing::info!("Database connected successfully");
    
    tracing::info!("Running database migrations...");
    db.migrate().await?;
    tracing::info!("Database migrations completed");

    let state = AppState {
        db: Arc::new(db),
        openrouter_api_key,
    };

    // Check if static directory exists
    if !std::path::Path::new("static").exists() {
        tracing::warn!("Static directory not found, creating it...");
        std::fs::create_dir_all("static")?;
    }

    let app = Router::new()
        .route("/", get(templates::index))
        .route("/api/query", post(api::handle_query))
        .route("/api/sources", get(api::get_sources))
        .nest_service("/static", ServeDir::new("static"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    tracing::info!("Binding to 0.0.0.0:3000...");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tracing::info!("Server listening on http://0.0.0.0:3000");
    tracing::info!("Application ready to accept connections");
    
    axum::serve(listener, app).await?;

    Ok(())
}
