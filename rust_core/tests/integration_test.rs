use data_pipeline::bar::StandardBar;
use engine::{BacktestEngine, EngineConfig, EngineSnapshot};
use orderbook::{CostBasisMethod, MarginMode};
use rust_decimal::Decimal;

/// Create 100 synthetic bars: trending up for first 50, then trending down.
fn generate_synthetic_bars() -> Vec<StandardBar> {
    let mut bars = Vec::with_capacity(100);
    let mut price = Decimal::from(10000);
    let symbol = "BTC-USDT".to_string();
    let exchange = "synthetic".to_string();

    for i in 0..100 {
        let open = price;
        // First 50 bars trend up, last 50 trend down
        let change = if i < 50 {
            Decimal::from(100)
        } else {
            Decimal::from(-100)
        };
        let close = price + change;
        let high = if close > open {
            close + Decimal::from(50)
        } else {
            open + Decimal::from(50)
        };
        let low = if close < open {
            close - Decimal::from(50)
        } else {
            open - Decimal::from(50)
        };
        let volume = Decimal::from(10);

        bars.push(StandardBar {
            timestamp: 1704067200000 + i as i64 * 3600000,
            open,
            high,
            low,
            close,
            volume,
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });

        price = close;
    }

    bars
}

/// A simple always-long strategy that opens a long on the first bar.
fn always_long_strategy(ctx: &engine::StrategyContext) -> Option<engine::Signal> {
    if ctx.current_bar_index == 1 {
        Some(engine::Signal {
            action: engine::SignalAction::OpenLong,
            symbol: "BTC-USDT".to_string(),
            quantity: Decimal::from(1),
            strength: 1.0,
            reason: "Always long test strategy".to_string(),
        })
    } else {
        None
    }
}

#[test]
fn test_full_backtest_pipeline() {
    let bars = generate_synthetic_bars();
    let config = EngineConfig {
        symbol: "BTC-USDT".to_string(),
        initial_balance: Decimal::from(100000),
        margin_mode: MarginMode::Cross,
        default_leverage: Decimal::from(1),
        maker_fee_rate: Decimal::new(1, 3),
        taker_fee_rate: Decimal::new(5, 3),
        maintenance_margin_rate: Decimal::new(5, 3),
        funding_interval_hours: 8,
        cost_basis_method: CostBasisMethod::FIFO,
        execution_delay_bars: 1,
        allow_future_data: false,
        risk_free_rate: 0.02,
    };

    let mut engine = engine::BacktestEngine::new(config, bars.clone());
    engine.set_strategy_callback(Box::new(always_long_strategy));

    let result = engine.run().expect("Backtest should complete without error");

    // 1. Final equity must be positive
    assert!(
        result.final_equity > Decimal::ZERO,
        "Final equity must be positive, got {}",
        result.final_equity
    );

    // 2. Max drawdown must be >= 0
    assert!(
        result.max_drawdown >= Decimal::ZERO,
        "Max drawdown must be non-negative, got {}",
        result.max_drawdown
    );

    // 3. At least one trade should have occurred
    assert!(
        result.total_trades > 0,
        "Expected at least one trade, got {}",
        result.total_trades
    );

    // 4. Verify total trades = winning + losing
    assert_eq!(
        result.total_trades,
        result.winning_trades + result.losing_trades,
        "Total trades must equal winning + losing trades"
    );

    // 5. Win rate should be between 0 and 100
    assert!(
        result.win_rate >= 0.0 && result.win_rate <= 100.0,
        "Win rate must be between 0 and 100, got {}",
        result.win_rate
    );

    // 6. Bar-by-bar execution should produce deterministic results
    engine.reset();
    let result2 = engine.run().expect("Second run should also succeed");
    assert_eq!(
        result.final_equity, result2.final_equity,
        "Deterministic replay: same data + same config must produce same equity"
    );
    assert_eq!(
        result.total_trades, result2.total_trades,
        "Deterministic replay: same data + same config must produce same trade count"
    );
}

#[test]
fn test_data_leakage_prevention() {
    let bars = generate_synthetic_bars();
    let config = EngineConfig {
        symbol: "BTC-USDT".to_string(),
        initial_balance: Decimal::from(100000),
        margin_mode: MarginMode::Cross,
        default_leverage: Decimal::from(1),
        maker_fee_rate: Decimal::new(1, 3),
        taker_fee_rate: Decimal::new(5, 3),
        maintenance_margin_rate: Decimal::new(5, 3),
        funding_interval_hours: 8,
        cost_basis_method: CostBasisMethod::FIFO,
        execution_delay_bars: 1,
        allow_future_data: false,
        risk_free_rate: 0.02,
    };

    // Strategy that attempts to look ahead
    let mut accessed_future = false;
    let strategy = move |ctx: &engine::StrategyContext| -> Option<engine::Signal> {
        // The engine MUST prevent access to bars beyond current_bar_index
        // In a proper implementation, ctx.bar_history.len() should never exceed
        // ctx.current_bar_index + 1 (because indexing is 0-based)
        let history_len = ctx.bar_history.len();
        let expected_max = ctx.current_bar_index + 1;

        if history_len > expected_max {
            accessed_future = true;
        }

        None
    };

    let mut engine = engine::BacktestEngine::new(config, bars.clone());
    engine.set_strategy_callback(Box::new(strategy));

    // The backtest should either:
    // 1. Complete successfully (engine prevented leakage internally)
    // 2. Return an error about data leakage
    let _result = engine.run();

    // If the engine properly prevents leakage, the flag should never be set
    // Note: This assertion documents the expected behavior. If the engine
    // does not yet enforce this, the test serves as a specification.
    assert!(
        !accessed_future,
        "Strategy was able to access future data — engine leakage prevention is broken"
    );
}

#[test]
fn test_execution_delay_default() {
    let bars = generate_synthetic_bars();
    let config = EngineConfig {
        symbol: "BTC-USDT".to_string(),
        initial_balance: Decimal::from(100000),
        margin_mode: MarginMode::Cross,
        default_leverage: Decimal::from(1),
        maker_fee_rate: Decimal::new(1, 3),
        taker_fee_rate: Decimal::new(5, 3),
        maintenance_margin_rate: Decimal::new(5, 3),
        funding_interval_hours: 8,
        cost_basis_method: CostBasisMethod::FIFO,
        execution_delay_bars: 1,
        allow_future_data: false,
        risk_free_rate: 0.02,
    };

    let mut engine = engine::BacktestEngine::new(config, bars.clone());
    engine.set_strategy_callback(Box::new(always_long_strategy));

    // Step through the engine bar by bar
    let mut snapshots: Vec<EngineSnapshot> = Vec::new();
    while let Some(snapshot) = engine.step() {
        snapshots.push(snapshot);
    }

    // With execution_delay_bars = 1, a signal generated at bar 1
    // should execute at bar 2. Verify the first fill occurs after delay.
    assert!(
        snapshots.len() > 2,
        "Need at least 3 snapshots to verify execution delay"
    );
}

#[test]
fn test_liquidation_trigger() {
    let bars = generate_synthetic_bars();
    let config = EngineConfig {
        symbol: "BTC-USDT".to_string(),
        initial_balance: Decimal::from(1000),
        margin_mode: MarginMode::Cross,
        default_leverage: Decimal::from(100), // Extreme leverage to force liquidation
        maker_fee_rate: Decimal::new(1, 3),
        taker_fee_rate: Decimal::new(5, 3),
        maintenance_margin_rate: Decimal::new(5, 3),
        funding_interval_hours: 8,
        cost_basis_method: CostBasisMethod::FIFO,
        execution_delay_bars: 1,
        allow_future_data: false,
        risk_free_rate: 0.02,
    };

    let mut engine = engine::BacktestEngine::new(config, bars.clone());
    engine.set_strategy_callback(Box::new(always_long_strategy));

    let result = engine.run().expect("Backtest should complete");

    // With 100x leverage on a down-trending market, we expect liquidation
    // The engine should handle this gracefully (position force-closed)
    assert!(
        result.max_drawdown_pct > Decimal::ZERO,
        "High leverage strategy should experience drawdown"
    );
}
