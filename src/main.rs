mod api;
mod db;
mod llm;
mod models;
mod rag;
mod search;
mod templates;
mod tools;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use crate::db::Database;
use crate::llm::LLMManager;
use crate::search::WebSearch;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub llm_manager: Arc<LLMManager>,
    /// Default model ID (first in models)
    pub default_model: String,
}

#[tokio::main]
async fn main() {
    // Initialize logging first - ensure it writes to stderr for Docker logs
    let log_level = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "info".to_string())
        .parse::<tracing::Level>()
        .unwrap_or(tracing::Level::INFO);
    
    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .with_writer(std::io::stderr)
        .with_ansi(false) // Disable ANSI colors for Docker logs
        .init();
    
    // Set panic hook to log panics with full backtrace
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = std::backtrace::Backtrace::capture();
        eprintln!("═══════════════════════════════════════════════════════════");
        eprintln!("PANIC OCCURRED!");
        eprintln!("═══════════════════════════════════════════════════════════");
        eprintln!("Location: {:?}", panic_info.location());
        eprintln!("Message: {:?}", panic_info.payload().downcast_ref::<&str>());
        eprintln!("Backtrace:\n{}", backtrace);
        eprintln!("═══════════════════════════════════════════════════════════");
        tracing::error!("PANIC: {:?}", panic_info);
        tracing::error!("Backtrace: {}", backtrace);
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
    
    tracing::info!("Connecting to database...");
    let db = Arc::new(Database::new(&database_url).await?);
    tracing::info!("Database connected successfully");
    
    tracing::info!("Running database migrations...");
    db.migrate().await?;
    tracing::info!("Database migrations completed");

    // Initialize LLM Manager
    let llm_manager = Arc::new(LLMManager::new(db.clone()));
    tracing::info!("Fetching available models...");
    llm_manager.fetch_available_models().await?;
    
    // Sync Tavily usage
    tracing::info!("Syncing Tavily usage limits...");
    WebSearch::sync_tavily_usage(&db).await.ok();
    
    let models = llm_manager.get_models().await;
    if models.is_empty() {
        tracing::warn!("No models found available from any provider!");
    } else {
        tracing::info!("Loaded {} models", models.len());
        for m in models.iter().take(5) {
             tracing::info!("- {} ({})", m.id, m.provider);
        }
    }

    let default_model = models.first()
        .map(|m| m.id.clone())
        .unwrap_or_else(|| "no-models-available".to_string());

    let state = AppState {
        db,
        llm_manager,
        default_model,
    };

    // Check if static directory exists
    if !std::path::Path::new("static").exists() {
        tracing::warn!("Static directory not found, creating it...");
        std::fs::create_dir_all("static")?;
    }

    // Health check endpoint
    async fn health_check() -> &'static str {
        "OK"
    }
    
    let app = Router::new()
        .route("/", get(templates::index))
        .route("/health", get(health_check))
        .route("/api/query", post(api::handle_query))
        .route("/api/sources", get(api::get_sources))
        .nest_service("/static", ServeDir::new("static"))
        .layer(CorsLayer::permissive())
        .with_state(state);
    
    tracing::info!("Router configured with routes: /, /health, /api/query, /api/sources, /static");

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
    
    tracing::info!("Starting Axum server...");
    eprintln!("Starting Axum server...");
    eprintln!("Server will run until interrupted (CTRL+C)");
    
    // Use a signal handler to gracefully shutdown
    let shutdown = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        tracing::info!("Received shutdown signal");
        eprintln!("Received shutdown signal");
    };
    
    // Start server with error handling
    match axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
    {
        Ok(_) => {
            tracing::info!("Server shutdown gracefully");
            eprintln!("Server shutdown gracefully");
            Ok(())
        },
        Err(e) => {
            eprintln!("═══════════════════════════════════════════════════════════");
            eprintln!("SERVER ERROR!");
            eprintln!("═══════════════════════════════════════════════════════════");
            eprintln!("Error: {}", e);
            eprintln!("═══════════════════════════════════════════════════════════");
            tracing::error!("Server error: {}", e);
            tracing::error!("Error details: {:?}", e);
            std::io::Write::flush(&mut std::io::stderr()).ok();
            Err(anyhow::anyhow!("Server error: {}", e))
        }
    }
}
