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
    // Initialize logging first - ensure it writes to stderr for Docker logs
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();
    
    // Set panic hook to log panics
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("PANIC: {:?}", panic_info);
        tracing::error!("PANIC: {:?}", panic_info);
    }));
    
    // Load .env file (ignore errors if it doesn't exist)
    dotenv::dotenv().ok();
    
    eprintln!("=== W9 Search Starting ===");
    tracing::info!("=== W9 Search Starting ===");
    
    if let Err(e) = run().await {
        eprintln!("=== FATAL ERROR ===");
        eprintln!("Fatal error: {}", e);
        eprintln!("Error chain: {:?}", e);
        eprintln!("==================");
        tracing::error!("Fatal error: {}", e);
        tracing::error!("Error chain: {:?}", e);
        
        // Flush logs before exiting
        std::io::Write::flush(&mut std::io::stderr()).ok();
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    eprintln!("Starting W9 Search application...");
    tracing::info!("Starting W9 Search application...");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:/app/data/w9_search.db".to_string());
    
    tracing::info!("Database URL: {}", database_url);
    
    // Ensure database directory exists if path contains directories
    if let Some(path) = database_url.strip_prefix("sqlite:") {
        let db_path = std::path::Path::new(path);
        
        if let Some(parent) = db_path.parent() {
            // Only create directory if parent is not empty (i.e., path contains directories)
            if !parent.as_os_str().is_empty() {
                tracing::info!("Creating database directory: {:?}", parent);
                std::fs::create_dir_all(parent)?;
                
                // Verify directory is writable
                let metadata = std::fs::metadata(parent)?;
                tracing::info!("Directory permissions: {:?}", metadata.permissions());
                
                // Test write access by creating a temp file
                let test_file = parent.join(".write_test");
                match std::fs::File::create(&test_file) {
                    Ok(_) => {
                        std::fs::remove_file(&test_file)?;
                        tracing::info!("Directory is writable");
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!(
                            "Database directory {:?} is not writable: {}. \
                            Please ensure the directory exists and has write permissions.",
                            parent, e
                        ));
                    }
                }
            } else {
                tracing::info!("Database file is in current directory, no parent directory to create");
            }
        }
        
        // Also try to create an empty database file to ensure the path is accessible
        tracing::info!("Database file path: {:?}", db_path);
        if !db_path.exists() {
            tracing::info!("Database file does not exist, SQLite will create it");
        } else {
            tracing::info!("Database file already exists");
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

    eprintln!("Binding to 0.0.0.0:3000...");
    tracing::info!("Binding to 0.0.0.0:3000...");
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .map_err(|e| {
            eprintln!("Failed to bind to 0.0.0.0:3000: {}", e);
            anyhow::anyhow!("Failed to bind to port 3000: {}", e)
        })?;
    
    eprintln!("Server listening on http://0.0.0.0:3000");
    eprintln!("Application ready to accept connections");
    tracing::info!("Server listening on http://0.0.0.0:3000");
    tracing::info!("Application ready to accept connections");
    
    // Flush stderr to ensure logs are visible
    std::io::Write::flush(&mut std::io::stderr()).ok();
    
    axum::serve(listener, app).await
        .map_err(|e| {
            eprintln!("Server error: {}", e);
            anyhow::anyhow!("Server error: {}", e)
        })?;

    Ok(())
}
