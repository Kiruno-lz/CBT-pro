use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use orderbook::Position;
use data_pipeline::StandardBar;
use crate::error::StrategyError;

/// Trading signal action types (replacing string-based actions)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalAction {
    OpenLong,
    OpenShort,
    CloseLong,
    CloseShort,
    CloseAll,
    ReduceLong(Decimal),
    ReduceShort(Decimal),
}

/// A trading signal emitted by a strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub action: SignalAction,
    pub symbol: String,
    pub quantity: Option<Decimal>,
    pub strength: f64,
    pub reason: String,
    pub stop_loss: Option<Decimal>,
    pub take_profit: Option<Decimal>,
}

/// Context provided to strategies on each bar.
pub struct StrategyContext<'a> {
    pub current_bar: &'a StandardBar,
    pub historical_bars: &'a [StandardBar],
    pub current_idx: usize,
    pub positions: &'a [Position],
    pub equity: Decimal,
    pub available_balance: Decimal,
}

/// Base trait for all trading strategies.
pub trait Strategy: Send + Sync {
    /// Called on each bar to generate trading signals.
    /// Returns a vector of signals (empty if no action).
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal>;
    
    /// Save internal state for persistence (optional).
    fn save_state(&self) -> Option<Vec<u8>> { None }
    
    /// Load internal state from persisted data (optional).
    fn load_state(&mut self, _state: &[u8]) -> Result<(), StrategyError> { Ok(()) }
}