/// Errors that can occur in strategy operations.
#[derive(Debug, thiserror::Error)]
pub enum StrategyError {
    #[error("Invalid strategy state: {0}")]
    InvalidState(String),
    #[error("Strategy parameter error: {0}")]
    ParameterError(String),
    #[error("State serialization error: {0}")]
    SerializationError(String),
}