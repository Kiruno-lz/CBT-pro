pub mod config;
pub mod backtest;

pub use config::EngineConfig;
pub use backtest::{BacktestEngine, EngineSnapshot, BacktestResult, EngineError};
// Re-export strategy types for backward compatibility
pub use strategy::{Strategy, Signal, SignalAction, StrategyContext};
pub use strategy::{AlwaysLong, EmaCrossover, RsiMacd, BollingerBands, Breakout};
