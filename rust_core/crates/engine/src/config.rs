use orderbook::{MarginMode, CostBasisMethod};
use rust_decimal::Decimal;
use serde::Serialize;

/// Configuration for a backtest engine run.
#[derive(Debug, Clone, Serialize)]
pub struct EngineConfig {
    pub symbol: String,
    pub initial_balance: Decimal,
    pub margin_mode: MarginMode,
    pub default_leverage: Decimal,
    pub maker_fee_rate: Decimal,
    pub taker_fee_rate: Decimal,
    pub maintenance_margin_rate: Decimal,
    pub funding_interval_hours: u32,
    pub cost_basis_method: CostBasisMethod,
    pub execution_delay_bars: u32,
    pub allow_future_data: bool,
    pub risk_free_rate: f64,
    pub default_quantity: Decimal,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            symbol: "BTC-USDT".to_string(),
            initial_balance: Decimal::from(100000),
            margin_mode: MarginMode::Cross,
            default_leverage: Decimal::from(10),
            maker_fee_rate: Decimal::new(1, 3), // 0.001
            taker_fee_rate: Decimal::new(5, 3), // 0.005
            maintenance_margin_rate: Decimal::new(5, 3), // 0.005
            funding_interval_hours: 8,
            cost_basis_method: CostBasisMethod::FIFO,
            execution_delay_bars: 1,
            allow_future_data: false,
            risk_free_rate: 0.02,
            default_quantity: Decimal::from(1),
        }
    }
}
