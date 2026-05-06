use thiserror::Error;

/// Unified error type for the data pipeline.
#[derive(Error, Debug)]
pub enum DataError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Aggregation error: {0}")]
    Aggregation(String),
    #[error("Exchange API error: {0}")]
    Exchange(String),
    #[error("Parquet error: {0}")]
    Parquet(String),
    #[error("Data not found: {0}")]
    NotFound(String),
    #[error("Invalid timeframe: {0}")]
    InvalidTimeFrame(String),
    #[error("SQL error: {0}")]
    Sql(#[from] sqlx::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Unknown exchange: {0}")]
    UnknownExchange(String),
    #[error("Unsupported interval: {0} seconds")]
    UnsupportedInterval(u64),
    #[error("Rate limited: retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Invalid interval: {0}")]
    InvalidInterval(String),
}
