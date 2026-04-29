use data_pipeline::StandardBar;
use engine::{BacktestEngine, EngineConfig, EngineSnapshot};
use orderbook::{CostBasisMethod, MarginMode};
use rust_decimal::Decimal;
use strategy::{Signal, SignalAction};

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

#[test]
fn test_full_backtest_pipeline() {
    let bars = generate_synthetic_bars();
    let config = EngineConfig {
        symbol: "BTC-USDT".to_string(),
        initial_balance: Decimal::from(100000),
        margin_mode: MarginMode::Cross,
        default_leverage: Decimal::from(1),
        default_quantity: Decimal::from(1),
        maker_fee_rate: Decimal::new(1, 3),
        taker_fee_rate: Decimal::new(5, 3),
        maintenance_margin_rate: Decimal::new(5, 3),
        funding_interval_hours: 8,
        cost_basis_method: CostBasisMethod::FIFO,
        execution_delay_bars: 1,
        allow_future_data: false,
        risk_free_rate: 0.02,
    };

    let mut engine = BacktestEngine::new(config, bars.clone(), None);

    // Simulate the original "always long" strategy by submitting a signal
    // after the first bar (mirrors old behaviour: signal at current_bar_index == 1).
    engine.step();
    engine.submit_signal(Signal {
        action: SignalAction::OpenLong,
        symbol: "BTC-USDT".to_string(),
        quantity: Some(Decimal::from(1)),
        strength: 1.0,
        reason: "Always long test strategy".to_string(),
        stop_loss: None,
        take_profit: None,
    });

    let result = engine
        .run()
        .expect("Backtest should complete without error");

    // 1. Final equity must be positive
    assert!(
        result.final_equity > Decimal::ZERO,
        "Final equity must be positive, got {}",
        result.final_equity
    );

    // 2. Max drawdown must be >= 0
    assert!(
        result.max_drawdown_pct >= 0.0,
        "Max drawdown must be non-negative, got {}",
        result.max_drawdown_pct
    );

    // 3. At least one trade should have occurred
    assert!(
        result.total_trades > 0,
        "Expected at least one trade, got {}",
        result.total_trades
    );

    // 4. Win rate should be between 0 and 1
    assert!(
        result.win_rate >= 0.0 && result.win_rate <= 1.0,
        "Win rate must be between 0 and 1, got {}",
        result.win_rate
    );

    // 5. Bar-by-bar execution should produce deterministic results
    engine.reset();
    engine.step();
    engine.submit_signal(Signal {
        action: SignalAction::OpenLong,
        symbol: "BTC-USDT".to_string(),
        quantity: Some(Decimal::from(1)),
        strength: 1.0,
        reason: "Always long test strategy".to_string(),
        stop_loss: None,
        take_profit: None,
    });
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
        default_quantity: Decimal::from(1),
        maker_fee_rate: Decimal::new(1, 3),
        taker_fee_rate: Decimal::new(5, 3),
        maintenance_margin_rate: Decimal::new(5, 3),
        funding_interval_hours: 8,
        cost_basis_method: CostBasisMethod::FIFO,
        execution_delay_bars: 1,
        allow_future_data: false,
        risk_free_rate: 0.02,
    };

    let mut engine = BacktestEngine::new(config, bars.clone(), None);

    // Step through several bars
    for _ in 0..10 {
        engine.step();
    }

    // get_strategy_bars should never return more bars than current_idx
    let strategy_bars = engine.get_strategy_bars(100);
    assert_eq!(
        strategy_bars.len(),
        10,
        "Strategy should only see past bars (10), got {}",
        strategy_bars.len()
    );

    // Verify the most recent accessible bar is the 9th bar (0-indexed)
    assert_eq!(
        strategy_bars.last().unwrap().timestamp,
        bars[9].timestamp,
        "Strategy should not see future data"
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
        default_quantity: Decimal::from(1),
        maker_fee_rate: Decimal::new(1, 3),
        taker_fee_rate: Decimal::new(5, 3),
        maintenance_margin_rate: Decimal::new(5, 3),
        funding_interval_hours: 8,
        cost_basis_method: CostBasisMethod::FIFO,
        execution_delay_bars: 1,
        allow_future_data: false,
        risk_free_rate: 0.02,
    };

    let mut engine = BacktestEngine::new(config, bars.clone(), None);

    // Step through the engine bar by bar
    let mut snapshots: Vec<EngineSnapshot> = Vec::new();
    snapshots.push(engine.step().unwrap());
    snapshots.push(engine.step().unwrap());
    snapshots.push(engine.step().unwrap());

    // Submit a signal at current state (after 3 steps, current_idx == 3)
    engine.submit_signal(Signal {
        action: SignalAction::OpenLong,
        symbol: "BTC-USDT".to_string(),
        quantity: Some(Decimal::from(1)),
        strength: 1.0,
        reason: "Test execution delay".to_string(),
        stop_loss: None,
        take_profit: None,
    });

    // With execution_delay_bars = 1, signal should execute at bar 4
    while let Some(snapshot) = engine.step() {
        snapshots.push(snapshot);
    }

    assert!(
        snapshots.len() > 2,
        "Need at least 3 snapshots to verify execution delay"
    );

    // Verify the first trade occurred at bar index 4 (3 + 1 delay)
    let first_trade = snapshots.iter().find(|s| s.total_trades > 0);
    assert!(first_trade.is_some(), "Expected a trade to occur");
    assert_eq!(
        first_trade.unwrap().current_bar_index,
        4,
        "Signal submitted after bar 3 should execute at bar 4 with delay=1"
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
        default_quantity: Decimal::from(1),
        maker_fee_rate: Decimal::new(1, 3),
        taker_fee_rate: Decimal::new(5, 3),
        maintenance_margin_rate: Decimal::new(5, 3),
        funding_interval_hours: 8,
        cost_basis_method: CostBasisMethod::FIFO,
        execution_delay_bars: 1,
        allow_future_data: false,
        risk_free_rate: 0.02,
    };

    let mut engine = BacktestEngine::new(config, bars.clone(), None);

    // Step once then submit a long signal to open a position
    engine.step();
    engine.submit_signal(Signal {
        action: SignalAction::OpenLong,
        symbol: "BTC-USDT".to_string(),
        quantity: Some(Decimal::from(1)),
        strength: 1.0,
        reason: "Liquidation test".to_string(),
        stop_loss: None,
        take_profit: None,
    });

    let result = engine.run().expect("Backtest should complete");

    // With 100x leverage on a down-trending market, we expect drawdown
    // The engine should handle this gracefully (position force-closed)
    assert!(
        result.max_drawdown_pct > 0.0,
        "High leverage strategy should experience drawdown"
    );
}
