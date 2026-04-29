pub mod atr;
pub mod bollinger;
pub mod ema;
pub mod macd;
pub mod rsi;
pub mod vwap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorResult {
    pub value: Decimal,
    pub timestamp: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum IndicatorError {
    #[error("Insufficient data: need {required}, got {got}")]
    InsufficientData { required: usize, got: usize },
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
    #[error("Calculation error: {0}")]
    CalculationError(String),
}
