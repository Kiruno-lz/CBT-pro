pub mod config;
pub mod backtest;
pub mod strategy;

pub use config::EngineConfig;
pub use backtest::{BacktestEngine, EngineSnapshot, BacktestResult, EngineError, Signal};
pub use strategy::{Strategy, AlwaysLong, EmaCrossover, RsiMacd, BollingerBands as StrategyBollingerBands, Breakout};
