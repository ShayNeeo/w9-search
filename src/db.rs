use sqlx::sqlite::{SqlitePool, SqliteConnectOptions};
use crate::models::Source;

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        // Parse the connection string and add create_if_missing option
        let options = if database_url.starts_with("sqlite:") {
            let path = database_url.strip_prefix("sqlite:").unwrap();
            
            // Try to resolve to absolute path for better error messages
            let abs_path = std::path::Path::new(path)
                .canonicalize()
                .or_else(|_| {
                    // If canonicalize fails, try to get absolute path from current dir
                    std::env::current_dir()
                        .map(|cwd| cwd.join(path))
                })
                .unwrap_or_else(|_| std::path::PathBuf::from(path));
            
            tracing::info!("Connecting to SQLite database at: {:?}", abs_path);
            
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
        } else {
            // Fallback to parsing the full URL
            database_url.parse::<SqliteConnectOptions>()?
                .create_if_missing(true)
        };
        
        let pool = SqlitePool::connect_with(options)
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to connect to SQLite database: {}. \
                    Database URL: {}. \
                    Make sure the directory exists and is writable.",
                    e, database_url
                )
            })?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn insert_source(&self, url: &str, title: &str, content: &str) -> anyhow::Result<i64> {
        let id = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO sources (url, title, content)
            VALUES (?, ?, ?)
            ON CONFLICT(url) DO UPDATE SET title = excluded.title, content = excluded.content
            RETURNING id
            "#,
        )
        .bind(url)
        .bind(title)
        .bind(content)
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    pub async fn get_sources(&self, limit: i64) -> anyhow::Result<Vec<Source>> {
        let sources = sqlx::query_as::<_, Source>(
            r#"
            SELECT id, url, title, content, created_at
            FROM sources
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(sources)
    }

    pub async fn search_sources(&self, query: &str, limit: i64) -> anyhow::Result<Vec<Source>> {
        let sources = sqlx::query_as::<_, Source>(
            r#"
            SELECT id, url, title, content, created_at
            FROM sources
            WHERE content LIKE ? OR title LIKE ?
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(format!("%{}%", query))
        .bind(format!("%{}%", query))
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(sources)
    }
}
