pub mod base;
pub mod error;

// Re-export strategy implementations
pub mod always_long;
pub mod ema_crossover;
pub mod rsi_macd;
pub mod bollinger_bands;
pub mod breakout;

pub use base::{Strategy, StrategyContext, Signal, SignalAction};
pub use error::StrategyError;
pub use always_long::AlwaysLong;
pub use ema_crossover::EmaCrossover;
pub use rsi_macd::RsiMacd;
pub use bollinger_bands::BollingerBands;
pub use breakout::Breakout;