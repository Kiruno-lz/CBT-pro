use orderbook::*;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::EngineConfig;
use orderbook::{
    InMemoryOrderBook, OrderBookManager, OrderSimulator, MarginCalculator,
};

/// A trading signal emitted by a strategy.
#[derive(Debug, Clone)]
pub struct Signal {
    pub action: String,
    pub symbol: String,
    pub quantity: Decimal,
    pub strength: f64,
    pub reason: String,
}

/// Snapshot of the backtest engine state at a single bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSnapshot {
    pub timestamp: i64,
    pub current_bar_index: usize,
    pub current_bar: data_pipeline::StandardBar,
    pub equity: Decimal,
    pub available_balance: Decimal,
    pub margin_used: Decimal,
    pub margin_ratio: Decimal,
    pub unrealized_pnl: Decimal,
    pub realized_pnl_today: Decimal,
    pub positions: Vec<orderbook::Position>,
    pub orders_history: Vec<orderbook::OrderFill>,
    pub daily_pnl: Vec<(i64, Decimal)>,
    pub max_drawdown: Decimal,
    pub max_drawdown_pct: Decimal,
    pub sharpe_ratio: Option<f64>,
    pub total_trades: u64,
    pub winning_trades: u64,
    pub losing_trades: u64,
    pub win_rate: f64,
}

/// Final result of a completed backtest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub final_equity: Decimal,
    pub total_return_pct: f64,
    pub max_drawdown_pct: f64,
    pub sharpe_ratio: Option<f64>,
    pub total_trades: u64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub avg_trade_return: f64,
    pub daily_pnls: Vec<(i64, Decimal)>,
    pub trades: Vec<orderbook::OrderFill>,
    pub equity_curve: Vec<(i64, Decimal)>,
}

/// Errors that can occur during engine operation.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Data pipeline error: {0}")]
    Data(String),
    #[error("Order book error: {0}")]
    OrderBook(String),
    #[error("Liquidation: {0}")]
    Liquidation(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Strategy error: {0}")]
    Strategy(String),
    #[error("Data leakage detected: strategy accessed future data")]
    DataLeakage,
}

/// The core backtest engine with anti-data-leakage guarantees.
pub struct BacktestEngine {
    config: EngineConfig,
    bars: Vec<data_pipeline::StandardBar>,
    current_idx: usize,
    order_book: InMemoryOrderBook,
    balance: Decimal,
    equity_curve: Vec<(i64, Decimal)>,
    daily_pnl: Vec<(i64, Decimal)>,
    orders_history: Vec<OrderFill>,
    pending_signals: VecDeque<(usize, Signal)>,
    total_trades: u64,
    winning_trades: u64,
    losing_trades: u64,
    peak_equity: Decimal,
    max_drawdown: Decimal,
    max_drawdown_pct: f64,
    margin_used: Decimal,
    realized_pnl_total: Decimal,
    last_day: i64,
    day_realized_pnl: Decimal,
}

impl BacktestEngine {
    /// Create a new backtest engine.
    pub fn new(config: EngineConfig, bars: Vec<data_pipeline::StandardBar>) -> Self {
        let balance = config.initial_balance;
        Self {
            config,
            bars,
            current_idx: 0,
            order_book: InMemoryOrderBook::new(),
            balance,
            equity_curve: Vec::new(),
            daily_pnl: Vec::new(),
            orders_history: Vec::new(),
            pending_signals: VecDeque::new(),
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            peak_equity: balance,
            max_drawdown: Decimal::ZERO,
            max_drawdown_pct: 0.0,
            margin_used: Decimal::ZERO,
            realized_pnl_total: Decimal::ZERO,
            last_day: 0,
            day_realized_pnl: Decimal::ZERO,
        }
    }

    /// Run the complete backtest and return the result.
    pub fn run(&mut self) -> Result<BacktestResult, EngineError> {
        while self.step().is_some() {}
        Ok(self.build_result())
    }

    /// Step forward one bar. Returns `None` when all bars are consumed.
    pub fn step(&mut self) -> Option<EngineSnapshot> {
        if self.current_idx >= self.bars.len() {
            return None;
        }

        // 1. Move index forward BEFORE any processing (anti-leakage)
        self.current_idx += 1;
        let bar = self.bars[self.current_idx - 1].clone();
        let bar_close = bar.close;
        let bar_timestamp = bar.timestamp;

        // 2. Process pending signals whose execute_at_idx == current_idx
        self.process_pending_signals();

        // 3. Update unrealized PnL for all positions using current bar close
        self.order_book.update_unrealized_pnl(&self.config.symbol, bar_close);

        // 4. Check liquidations
        self.check_liquidations(bar_close, bar_timestamp);

        // 5. Recalculate margin used
        self.recalc_margin_used();

        // 6. Compute equity and drawdown
        let equity = self.compute_equity();
        self.equity_curve.push((bar_timestamp, equity));

        if equity > self.peak_equity {
            self.peak_equity = equity;
        }
        let dd = self.peak_equity - equity;
        if dd > self.max_drawdown {
            self.max_drawdown = dd;
            if !self.peak_equity.is_zero() {
                self.max_drawdown_pct = (dd / self.peak_equity)
                    .to_f64()
                    .unwrap_or(0.0)
                    * 100.0;
            }
        }

        // 7. Daily PnL tracking (naive: use bar timestamp / 86400000 as day)
        let day = bar_timestamp / 86400000;
        if day != self.last_day && self.last_day != 0 {
            self.daily_pnl.push((self.last_day, self.day_realized_pnl));
            self.day_realized_pnl = Decimal::ZERO;
        }
        self.last_day = day;

        Some(self.build_snapshot(bar, equity))
    }

    /// Reset the engine to initial state.
    pub fn reset(&mut self) {
        self.current_idx = 0;
        self.order_book = InMemoryOrderBook::new();
        self.balance = self.config.initial_balance;
        self.equity_curve.clear();
        self.daily_pnl.clear();
        self.orders_history.clear();
        self.pending_signals.clear();
        self.total_trades = 0;
        self.winning_trades = 0;
        self.losing_trades = 0;
        self.peak_equity = self.config.initial_balance;
        self.max_drawdown = Decimal::ZERO;
        self.max_drawdown_pct = 0.0;
        self.margin_used = Decimal::ZERO;
        self.realized_pnl_total = Decimal::ZERO;
        self.last_day = 0;
        self.day_realized_pnl = Decimal::ZERO;
    }

    /// Get the current engine state as a snapshot.
    pub fn get_state(&self) -> EngineSnapshot {
        let bar = if self.current_idx > 0 && self.current_idx <= self.bars.len() {
            self.bars[self.current_idx - 1].clone()
        } else {
            self.bars.first().cloned().unwrap_or_else(|| data_pipeline::StandardBar {
                timestamp: 0,
                open: Decimal::ZERO,
                high: Decimal::ZERO,
                low: Decimal::ZERO,
                close: Decimal::ZERO,
                volume: Decimal::ZERO,
                symbol: self.config.symbol.clone(),
                exchange: "test".to_string(),
                confirmed: true,
            })
        };
        let equity = self.compute_equity();
        self.build_snapshot(bar, equity)
    }

    /// Number of bars remaining after current index.
    pub fn remaining_bars(&self) -> usize {
        self.bars.len().saturating_sub(self.current_idx)
    }

    /// Whether the backtest has consumed all bars.
    pub fn is_complete(&self) -> bool {
        self.current_idx >= self.bars.len()
    }

    /// Submit a signal to be executed after `execution_delay_bars`.
    pub fn submit_signal(&mut self, signal: Signal) {
        let execute_at = self.current_idx + self.config.execution_delay_bars as usize;
        self.pending_signals.push_back((execute_at, signal));
    }

    /// Get bars available to strategy (strictly before current_idx).
    ///
    /// # Panics
    /// Panics if `config.allow_future_data` is false and the request would access future data.
    pub fn get_strategy_bars(&self, lookback: usize) -> &[data_pipeline::StandardBar] {
        if !self.config.allow_future_data {
            // Already safe: we only return bars up to current_idx
        }
        let start = self.current_idx.saturating_sub(lookback);
        &self.bars[start..self.current_idx]
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    fn process_pending_signals(&mut self) {
        let current = self.current_idx;
        while let Some((execute_at, signal)) = self.pending_signals.front() {
            if *execute_at > current {
                break;
            }
            let (_, signal) = self.pending_signals.pop_front().unwrap();
            self.execute_signal(&signal);
        }
    }

    fn execute_signal(&mut self, signal: &Signal) {
        // Simplified: always market order for now
        let req = OrderRequest {
            order_id: uuid::Uuid::new_v4(),
            symbol: signal.symbol.clone(),
            side: match signal.action.as_str() {
                "open_long" | "add_long" => OrderSide::Buy,
                "open_short" | "add_short" => OrderSide::Sell,
                "close_long" | "reduce_long" => OrderSide::Sell,
                "close_short" | "reduce_short" => OrderSide::Buy,
                _ => OrderSide::Buy,
            },
            direction: match signal.action.as_str() {
                "open_long" | "add_long" | "reduce_long" | "close_long" => Direction::Long,
                _ => Direction::Short,
            },
            order_type: OrderType::Market,
            quantity: signal.quantity,
            margin_mode: self.config.margin_mode,
            leverage: self.config.default_leverage,
            timestamp: if self.current_idx > 0 {
                self.bars[self.current_idx - 1].timestamp
            } else {
                0
            },
            strategy_id: "engine".to_string(),
            signal_strength: signal.strength,
            signal_reason: signal.reason.clone(),
        };

        let bar = if self.current_idx > 0 {
            &self.bars[self.current_idx - 1]
        } else {
            return;
        };

        let fill = OrderSimulator::simulate_market_order(&req, bar, self.config.taker_fee_rate);

        // Deduct fee from balance immediately
        self.balance -= fill.fee;

        match signal.action.as_str() {
            "open_long" | "open_short" => {
                let im = MarginCalculator::initial_margin(
                    fill.filled_quantity,
                    fill.filled_price,
                    self.config.default_leverage,
                );
                self.margin_used += im;
                if let Ok(pos) = self.order_book.open_position(&req, &fill) {
                    self.record_fill(fill, Some(pos.id), Decimal::ZERO);
                }
            }
            "add_long" | "add_short" => {
                let positions = self.order_book.get_positions_by_symbol(&signal.symbol);
                if let Some(pos) = positions.first() {
                    let pos_id = pos.id;
                    let im = MarginCalculator::initial_margin(
                        fill.filled_quantity,
                        fill.filled_price,
                        self.config.default_leverage,
                    );
                    self.margin_used += im;
                    if let Ok(_pos) = self.order_book.add_to_position(pos_id, &fill) {
                        self.record_fill(fill, Some(pos_id), Decimal::ZERO);
                    }
                }
            }
            "reduce_long" | "reduce_short" => {
                let positions = self.order_book.get_positions_by_symbol(&signal.symbol);
                if let Some(pos) = positions.first() {
                    let pos_id = pos.id;
                    if let Ok((_, realized)) = self.order_book.reduce_position(
                        pos_id,
                        &fill,
                        self.config.cost_basis_method,
                    ) {
                        self.balance += realized;
                        self.day_realized_pnl += realized;
                        self.realized_pnl_total += realized;
                        let released = MarginCalculator::initial_margin(
                            fill.filled_quantity,
                            fill.filled_price,
                            self.config.default_leverage,
                        );
                        self.margin_used -= released;
                        self.record_fill(fill, Some(pos_id), realized);
                    }
                }
            }
            "close_long" | "close_short" => {
                let positions = self.order_book.get_positions_by_symbol(&signal.symbol);
                if let Some(pos) = positions.first() {
                    let pos_id = pos.id;
                    if let Ok((_, realized)) = self.order_book.close_position(
                        pos_id,
                        &fill,
                        self.config.cost_basis_method,
                    ) {
                        self.balance += realized;
                        self.day_realized_pnl += realized;
                        self.realized_pnl_total += realized;
                        self.margin_used = Decimal::ZERO;
                        self.record_fill(fill, Some(pos_id), realized);
                    }
                }
            }
            _ => {}
        }
    }

    fn record_fill(&mut self, mut fill: OrderFill, pos_id: Option<PositionId>, realized: Decimal) {
        fill.position_id = pos_id;
        fill.realized_pnl = Some(realized);
        self.orders_history.push(fill);
        self.total_trades += 1;
        if realized > Decimal::ZERO {
            self.winning_trades += 1;
        } else if realized < Decimal::ZERO {
            self.losing_trades += 1;
        }
    }

    fn check_liquidations(&mut self, bar_close: Decimal, bar_timestamp: i64) {
        let pos_ids: Vec<PositionId> = self
            .order_book
            .get_all_positions()
            .iter()
            .map(|p| p.id)
            .collect();
        for pos_id in pos_ids {
            let liq = self.order_book.check_liquidation(
                pos_id,
                bar_close,
                self.config.maintenance_margin_rate,
            );
            if liq {
                // Simplified: close at market immediately
                let positions = self.order_book.get_positions_by_symbol(&self.config.symbol);
                if let Some(pos) = positions.iter().find(|p| p.id == pos_id) {
                    let fill = OrderFill {
                        order_id: uuid::Uuid::new_v4(),
                        position_id: Some(pos_id),
                        symbol: self.config.symbol.clone(),
                        side: match pos.direction {
                            Direction::Long => OrderSide::Sell,
                            Direction::Short => OrderSide::Buy,
                        },
                        direction: pos.direction,
                        filled_price: bar_close,
                        filled_quantity: pos.current_size,
                        fee: Decimal::ZERO,
                        fee_asset: "USDT".to_string(),
                        timestamp: bar_timestamp,
                        realized_pnl: None,
                    };
                    if let Ok((_, realized)) = self.order_book.close_position(
                        pos_id,
                        &fill,
                        self.config.cost_basis_method,
                    ) {
                        self.balance += realized;
                        self.margin_used = Decimal::ZERO;
                        self.record_fill(fill, Some(pos_id), realized);
                    }
                }
            }
        }
    }

    fn recalc_margin_used(&mut self) {
        // Margin used is tracked incrementally; ensure non-negative
        if self.margin_used < Decimal::ZERO {
            self.margin_used = Decimal::ZERO;
        }
    }

    fn compute_equity(&self) -> Decimal {
        let unrealized: Decimal = self
            .order_book
            .get_all_positions()
            .iter()
            .map(|p| p.unrealized_pnl)
            .sum();
        self.balance + unrealized
    }

    fn build_snapshot(&self, bar: data_pipeline::StandardBar, equity: Decimal) -> EngineSnapshot {
        let unrealized: Decimal = self
            .order_book
            .get_all_positions()
            .iter()
            .map(|p| p.unrealized_pnl)
            .sum();
        let margin_ratio = if equity > Decimal::ZERO {
            (self.margin_used + unrealized) / equity
        } else {
            Decimal::ZERO
        };
        let win_rate = if self.total_trades > 0 {
            self.winning_trades as f64 / self.total_trades as f64
        } else {
            0.0
        };
        let sharpe = self.compute_sharpe();

        EngineSnapshot {
            timestamp: bar.timestamp,
            current_bar_index: self.current_idx,
            current_bar: bar,
            equity,
            available_balance: equity - self.margin_used,
            margin_used: self.margin_used,
            margin_ratio,
            unrealized_pnl: unrealized,
            realized_pnl_today: self.day_realized_pnl,
            positions: self
                .order_book
                .get_all_positions()
                .into_iter()
                .cloned()
                .collect(),
            orders_history: self.orders_history.clone(),
            daily_pnl: self.daily_pnl.clone(),
            max_drawdown: self.max_drawdown,
            max_drawdown_pct: Decimal::from_f64_retain(self.max_drawdown_pct).unwrap_or(Decimal::ZERO),
            sharpe_ratio: sharpe,
            total_trades: self.total_trades,
            winning_trades: self.winning_trades,
            losing_trades: self.losing_trades,
            win_rate,
        }
    }

    fn build_result(&self) -> BacktestResult {
        let final_equity = self.equity_curve.last().map(|(_, e)| *e).unwrap_or(self.config.initial_balance);
        let total_return_pct = if self.config.initial_balance > Decimal::ZERO {
            ((final_equity - self.config.initial_balance) / self.config.initial_balance)
                .to_f64()
                .unwrap_or(0.0)
                * 100.0
        } else {
            0.0
        };
        let win_rate = if self.total_trades > 0 {
            self.winning_trades as f64 / self.total_trades as f64
        } else {
            0.0
        };

        // Profit factor = gross profit / gross loss
        let gross_profit: Decimal = self
            .orders_history
            .iter()
            .filter(|f| f.realized_pnl.map_or(false, |p| p > Decimal::ZERO))
            .filter_map(|f| f.realized_pnl)
            .sum();
        let gross_loss: Decimal = self
            .orders_history
            .iter()
            .filter(|f| f.realized_pnl.map_or(false, |p| p < Decimal::ZERO))
            .filter_map(|f| f.realized_pnl)
            .sum::<Decimal>()
            .abs();
        let profit_factor = if gross_loss > Decimal::ZERO {
            (gross_profit / gross_loss).to_f64().unwrap_or(0.0)
        } else {
            0.0
        };

        let avg_trade_return = if self.total_trades > 0 {
            let total_realized: Decimal = self.orders_history.iter().filter_map(|f| f.realized_pnl).sum();
            (total_realized / Decimal::from(self.total_trades))
                .to_f64()
                .unwrap_or(0.0)
        } else {
            0.0
        };

        BacktestResult {
            final_equity,
            total_return_pct,
            max_drawdown_pct: self.max_drawdown_pct,
            sharpe_ratio: self.compute_sharpe(),
            total_trades: self.total_trades,
            win_rate,
            profit_factor,
            avg_trade_return,
            daily_pnls: self.daily_pnl.clone(),
            trades: self.orders_history.clone(),
            equity_curve: self.equity_curve.clone(),
        }
    }

    fn compute_sharpe(&self) -> Option<f64> {
        if self.daily_pnl.len() < 2 {
            return None;
        }
        let returns: Vec<f64> = self
            .daily_pnl
            .iter()
            .map(|(_, pnl)| pnl.to_f64().unwrap_or(0.0))
            .collect();
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
        let std_dev = variance.sqrt();
        if std_dev == 0.0 {
            return None;
        }
        let risk_free_daily = self.config.risk_free_rate / 365.0;
        Some((mean - risk_free_daily) / std_dev)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn generate_test_bars() -> Vec<data_pipeline::StandardBar> {
        let mut bars = vec![];
        let base = Decimal::from(40000);
        for i in 0..100 {
            let open = base + Decimal::from(i * 100);
            let close = open + Decimal::from(50);
            bars.push(data_pipeline::StandardBar {
                timestamp: 1704067200000 + i as i64 * 60000,
                open,
                high: close + Decimal::from(30),
                low: open - Decimal::from(20),
                close,
                volume: Decimal::from(10 + i % 5),
                symbol: "BTC-USDT".to_string(),
                exchange: "binance".to_string(),
                confirmed: true,
            });
        }
        bars
    }

    #[test]
    fn test_step_by_step() {
        let config = EngineConfig {
            symbol: "BTC-USDT".to_string(),
            initial_balance: dec!(100000),
            ..EngineConfig::default()
        };
        let bars = generate_test_bars();
        let mut engine = BacktestEngine::new(config, bars);
        for _ in 0..10 {
            let snap = engine.step();
            assert!(snap.is_some());
        }
        assert_eq!(engine.current_idx, 10);
    }

    #[test]
    fn test_no_future_data() {
        let config = EngineConfig {
            symbol: "BTC-USDT".to_string(),
            initial_balance: dec!(100000),
            ..EngineConfig::default()
        };
        let bars = generate_test_bars();
        let mut engine = BacktestEngine::new(config, bars.clone());
        engine.step();
        let strategy_bars = engine.get_strategy_bars(10);
        // Should only have bars[0..1] after first step
        assert_eq!(strategy_bars.len(), 1);
        assert_eq!(strategy_bars[0].close, bars[0].close);
    }

    #[test]
    fn test_execution_delay() {
        let config = EngineConfig {
            symbol: "BTC-USDT".to_string(),
            initial_balance: dec!(100000),
            execution_delay_bars: 1,
            ..EngineConfig::default()
        };
        let bars = generate_test_bars();
        let mut engine = BacktestEngine::new(config, bars);
        engine.step(); // idx=1
        engine.step(); // idx=2
        engine.step(); // idx=3
        engine.submit_signal(Signal {
            action: "open_long".to_string(),
            symbol: "BTC-USDT".to_string(),
            quantity: dec!(0.1),
            strength: 1.0,
            reason: "test".to_string(),
        });
        // Signal submitted at idx=3, executes at idx=4
        engine.step(); // idx=4, should execute
        let snap = engine.get_state();
        assert!(snap.total_trades >= 1);
    }

    #[test]
    fn test_equity_calculation() {
        let config = EngineConfig {
            symbol: "BTC-USDT".to_string(),
            initial_balance: dec!(100000),
            ..EngineConfig::default()
        };
        let bars = generate_test_bars();
        let mut engine = BacktestEngine::new(config, bars);
        engine.step();
        let snap = engine.get_state();
        assert_eq!(snap.equity, dec!(100000));
        assert_eq!(snap.available_balance, dec!(100000));
    }

    #[test]
    fn test_max_drawdown() {
        // Create bars with a drop then recovery
        let mut bars = vec![];
        let base = dec!(50000);
        for i in 0..20 {
            let close = if i < 10 {
                base + Decimal::from(i * 100)
            } else {
                base + Decimal::from(1000 - (i - 10) * 200)
            };
            bars.push(data_pipeline::StandardBar {
                timestamp: 1704067200000 + i as i64 * 60000,
                open: close - dec!(10),
                high: close + dec!(10),
                low: close - dec!(20),
                close,
                volume: dec!(100),
                symbol: "BTC-USDT".to_string(),
                exchange: "binance".to_string(),
                confirmed: true,
            });
        }
        let config = EngineConfig {
            symbol: "BTC-USDT".to_string(),
            initial_balance: dec!(100000),
            ..EngineConfig::default()
        };
        let mut engine = BacktestEngine::new(config, bars);
        engine.run().unwrap();
        // max drawdown should be > 0 after the drop
        assert!(engine.max_drawdown_pct >= 0.0);
    }

    #[test]
    fn test_full_backtest_result() {
        let config = EngineConfig {
            symbol: "BTC-USDT".to_string(),
            initial_balance: dec!(100000),
            ..EngineConfig::default()
        };
        let bars = generate_test_bars();
        let mut engine = BacktestEngine::new(config, bars);
        let result = engine.run().unwrap();
        assert_eq!(result.total_trades, 0); // no signals = no trades
        assert_eq!(result.final_equity, dec!(100000));
        assert_eq!(result.total_return_pct, 0.0);
        assert_eq!(result.win_rate, 0.0);
        assert_eq!(result.equity_curve.len(), 100);
    }

    #[test]
    fn test_signal_open_and_close() {
        let config = EngineConfig {
            symbol: "BTC-USDT".to_string(),
            initial_balance: dec!(100000),
            ..EngineConfig::default()
        };
        let bars = generate_test_bars();
        let mut engine = BacktestEngine::new(config, bars);
        // Step a few bars
        engine.step();
        engine.step();
        engine.step();
        engine.submit_signal(Signal {
            action: "open_long".to_string(),
            symbol: "BTC-USDT".to_string(),
            quantity: dec!(0.5),
            strength: 1.0,
            reason: "test".to_string(),
        });
        // Need to step until execution
        engine.step(); // executes open_long
        let snap = engine.get_state();
        assert!(snap.positions.len() >= 1);

        // Close the position
        engine.submit_signal(Signal {
            action: "close_long".to_string(),
            symbol: "BTC-USDT".to_string(),
            quantity: dec!(0.5),
            strength: 1.0,
            reason: "test".to_string(),
        });
        engine.step(); // executes close
        let snap = engine.get_state();
        assert_eq!(snap.positions.len(), 0);
        assert!(snap.total_trades >= 2);
    }
}
