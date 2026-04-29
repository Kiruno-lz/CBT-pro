pub mod backtest;
pub mod config;

pub use backtest::{BacktestEngine, BacktestResult, EngineError, EngineSnapshot};
pub use config::EngineConfig;
// Re-export strategy types for backward compatibility
pub use strategy::{AlwaysLong, BollingerBands, Breakout, EmaCrossover, RsiMacd};
pub use strategy::{Signal, SignalAction, Strategy, StrategyContext};
