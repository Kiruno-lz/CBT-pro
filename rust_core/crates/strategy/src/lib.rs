pub mod base;
pub mod config;
pub mod error;

// Re-export strategy implementations
pub mod always_long;
pub mod bollinger_bands;
pub mod breakout;
pub mod ema_crossover;
pub mod rsi_macd;

pub use always_long::AlwaysLong;
pub use base::{Signal, SignalAction, Strategy, StrategyContext};
pub use bollinger_bands::BollingerBands;
pub use breakout::Breakout;
pub use config::{available_strategies, ParamDefinition, ParamType, StrategyInfo};
pub use ema_crossover::EmaCrossover;
pub use error::StrategyError;
pub use rsi_macd::RsiMacd;
