use data_pipeline::StandardBar;
use rust_decimal::Decimal;
use strategy::*;

// ------------------------------------------------------------------
// Helpers
// ------------------------------------------------------------------

/// Create synthetic bars for testing.
///
/// * `trend = "up"`    – close increases by `+10` each bar.
/// * `trend = "down"`  – close decreases by `-10` each bar.
/// * `trend = "sideways"` – close oscillates around a fixed price.
fn create_trending_bars(trend: &str, count: usize) -> Vec<StandardBar> {
    let mut bars = Vec::with_capacity(count);
    let symbol = "BTC-USDT".to_string();
    let exchange = "synthetic".to_string();

    for i in 0..count {
        let base = Decimal::from(10000);
        let change = match trend {
            "up" => Decimal::from(100 * i as i64),
            "down" => Decimal::from(-100 * i as i64),
            _ => {
                // sideways: completely flat (no variation)
                Decimal::ZERO
            }
        };
        let close = base + change;
        let open = close - Decimal::from(5);
        let high = close + Decimal::from(10);
        let low = open - Decimal::from(10);
        let volume = Decimal::from(100);

        bars.push(StandardBar {
            timestamp: 1704067200 + i as i64 * 3600,
            open,
            high,
            low,
            close,
            volume,
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });
    }
    bars
}

/// Build a `StrategyContext` pointing at `current_idx` inside `bars`.
fn make_context<'a>(bars: &'a [StandardBar], current_idx: usize) -> StrategyContext<'a> {
    StrategyContext {
        current_bar: &bars[current_idx],
        historical_bars: &bars[..=current_idx],
        current_idx: current_idx + 1, // current_idx in context is 1-based
        positions: &[],
        equity: Decimal::from(100000),
        available_balance: Decimal::from(100000),
    }
}

/// Create bars with three phases: down, up, down.
/// Ensures both bullish and bearish EMA crossovers occur.
fn create_three_phase_bars() -> Vec<StandardBar> {
    let mut bars = Vec::with_capacity(120);
    let symbol = "BTC-USDT".to_string();
    let exchange = "synthetic".to_string();

    // Phase 1: downtrend (30 bars) – fast EMA goes below slow EMA
    let mut price = Decimal::from(15000);
    for i in 0..30 {
        let open = price;
        price -= Decimal::from(100);
        let close = price;
        let high = if close > open {
            close + Decimal::from(10)
        } else {
            open + Decimal::from(10)
        };
        let low = if close < open {
            close - Decimal::from(10)
        } else {
            open - Decimal::from(10)
        };
        bars.push(StandardBar {
            timestamp: 1704067200 + i as i64 * 3600,
            open,
            high,
            low,
            close,
            volume: Decimal::from(100),
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });
    }

    // Phase 2: uptrend (40 bars) – bullish crossover
    for i in 30..70 {
        let open = price;
        price += Decimal::from(100);
        let close = price;
        let high = if close > open {
            close + Decimal::from(10)
        } else {
            open + Decimal::from(10)
        };
        let low = if close < open {
            close - Decimal::from(10)
        } else {
            open - Decimal::from(10)
        };
        bars.push(StandardBar {
            timestamp: 1704067200 + i as i64 * 3600,
            open,
            high,
            low,
            close,
            volume: Decimal::from(100),
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });
    }

    // Phase 3: downtrend (50 bars) – bearish crossover
    for i in 70..120 {
        let open = price;
        price -= Decimal::from(100);
        let close = price;
        let high = if close > open {
            close + Decimal::from(10)
        } else {
            open + Decimal::from(10)
        };
        let low = if close < open {
            close - Decimal::from(10)
        } else {
            open - Decimal::from(10)
        };
        bars.push(StandardBar {
            timestamp: 1704067200 + i as i64 * 3600,
            open,
            high,
            low,
            close,
            volume: Decimal::from(100),
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });
    }

    bars
}

// ------------------------------------------------------------------
// 1. AlwaysLong Strategy Tests
// ------------------------------------------------------------------

#[test]
fn test_always_long_emits_signal_once() {
    let bars = create_trending_bars("up", 5);
    let mut strategy = AlwaysLong::new("BTC-USDT".to_string(), Decimal::from(1));
    let ctx = make_context(&bars, 0);

    let signals = strategy.on_bar(&ctx);
    assert_eq!(
        signals.len(),
        1,
        "AlwaysLong should emit exactly one signal on first call"
    );
    assert_eq!(
        signals[0].action,
        SignalAction::OpenLong,
        "Signal should be OpenLong"
    );
}

#[test]
fn test_always_long_no_second_signal() {
    let bars = create_trending_bars("up", 5);
    let mut strategy = AlwaysLong::new("BTC-USDT".to_string(), Decimal::from(1));

    let ctx1 = make_context(&bars, 0);
    let _ = strategy.on_bar(&ctx1);

    let ctx2 = make_context(&bars, 1);
    let signals = strategy.on_bar(&ctx2);
    assert!(
        signals.is_empty(),
        "After first signal, AlwaysLong should return empty vec"
    );
}

#[test]
fn test_always_long_signal_fields() {
    let bars = create_trending_bars("up", 5);
    let mut strategy = AlwaysLong::new("BTC-USDT".to_string(), Decimal::from(1));
    let ctx = make_context(&bars, 0);

    let signals = strategy.on_bar(&ctx);
    assert_eq!(signals.len(), 1);
    let sig = &signals[0];

    assert_eq!(
        sig.action,
        SignalAction::OpenLong,
        "action should be OpenLong"
    );
    assert_eq!(sig.symbol, "BTC-USDT", "symbol should match");
    assert_eq!(
        sig.quantity,
        Some(Decimal::from(1)),
        "quantity should be Some(1)"
    );
    assert_eq!(sig.strength, 1.0, "strength should be 1.0");
    assert_eq!(sig.reason, "AlwaysLong", "reason should be 'AlwaysLong'");
    assert_eq!(sig.stop_loss, None, "stop_loss should be None");
    assert_eq!(sig.take_profit, None, "take_profit should be None");
}

// ------------------------------------------------------------------
// 2. EmaCrossover Strategy Tests
// ------------------------------------------------------------------

#[test]
fn test_ema_crossover_no_signal_early() {
    let bars = create_trending_bars("up", 30);
    let mut strategy = EmaCrossover {
        symbol: "BTC-USDT".to_string(),
        quantity: Decimal::from(1),
        fast_period: 5,
        slow_period: 10,
    };

    // Before slow_period + 1 bars (idx < 10), no signal expected
    for idx in 0..10 {
        let ctx = make_context(&bars, idx);
        let signals = strategy.on_bar(&ctx);
        assert!(
            signals.is_empty(),
            "No signal expected before enough data at idx {}",
            idx
        );
    }
}

#[test]
fn test_ema_crossover_bullish_signal() {
    // Create bars that trend down then sharply up to force a bullish crossover
    let mut bars = create_trending_bars("down", 20);
    let up_bars = create_trending_bars("up", 20);
    bars.extend(up_bars);

    let mut strategy = EmaCrossover {
        symbol: "BTC-USDT".to_string(),
        quantity: Decimal::from(1),
        fast_period: 5,
        slow_period: 10,
    };

    let mut found_signal = false;
    for idx in 10..bars.len() {
        let ctx = make_context(&bars, idx);
        let signals = strategy.on_bar(&ctx);
        if let Some(sig) = signals.first() {
            if sig.action == SignalAction::OpenLong {
                found_signal = true;
                assert_eq!(sig.symbol, "BTC-USDT");
                assert_eq!(sig.quantity, Some(Decimal::from(1)));
                assert_eq!(sig.strength, 1.0);
                assert_eq!(sig.reason, "EMA crossover bullish");
                break;
            }
        }
    }
    assert!(
        found_signal,
        "Expected at least one bullish EMA crossover signal"
    );
}

#[test]
fn test_ema_crossover_bearish_signal() {
    // Create bars that trend up then sharply down to force a bearish crossover
    let mut bars = create_trending_bars("up", 20);
    let down_bars = create_trending_bars("down", 20);
    bars.extend(down_bars);

    let mut strategy = EmaCrossover {
        symbol: "BTC-USDT".to_string(),
        quantity: Decimal::from(1),
        fast_period: 5,
        slow_period: 10,
    };

    let mut found_signal = false;
    for idx in 10..bars.len() {
        let ctx = make_context(&bars, idx);
        let signals = strategy.on_bar(&ctx);
        if let Some(sig) = signals.first() {
            if sig.action == SignalAction::CloseLong {
                found_signal = true;
                assert_eq!(sig.symbol, "BTC-USDT");
                assert_eq!(sig.quantity, Some(Decimal::from(1)));
                assert_eq!(sig.strength, 1.0);
                assert_eq!(sig.reason, "EMA crossover bearish");
                break;
            }
        }
    }
    assert!(
        found_signal,
        "Expected at least one bearish EMA crossover signal"
    );
}

#[test]
fn test_ema_crossover_no_crossover() {
    // Sideways market should not produce crossovers
    let bars = create_trending_bars("sideways", 50);
    let mut strategy = EmaCrossover {
        symbol: "BTC-USDT".to_string(),
        quantity: Decimal::from(1),
        fast_period: 5,
        slow_period: 10,
    };

    // Allow a warm-up period for EMAs to stabilise before asserting
    for idx in 25..bars.len() {
        let ctx = make_context(&bars, idx);
        let signals = strategy.on_bar(&ctx);
        assert!(
            signals.is_empty(),
            "Sideways market should not produce crossover signals at idx {}",
            idx
        );
    }
}

// ------------------------------------------------------------------
// 3. SignalAction Tests
// ------------------------------------------------------------------

#[test]
fn test_signal_action_variants() {
    let open_long = SignalAction::OpenLong;
    let open_short = SignalAction::OpenShort;
    let close_long = SignalAction::CloseLong;
    let close_short = SignalAction::CloseShort;
    let close_all = SignalAction::CloseAll;
    let reduce_long = SignalAction::ReduceLong(Decimal::new(5, 1));
    let reduce_short = SignalAction::ReduceShort(Decimal::new(5, 1));

    // Verify each variant can be matched
    match open_long {
        SignalAction::OpenLong => {}
        _ => panic!("Expected OpenLong"),
    }
    match open_short {
        SignalAction::OpenShort => {}
        _ => panic!("Expected OpenShort"),
    }
    match close_long {
        SignalAction::CloseLong => {}
        _ => panic!("Expected CloseLong"),
    }
    match close_short {
        SignalAction::CloseShort => {}
        _ => panic!("Expected CloseShort"),
    }
    match close_all {
        SignalAction::CloseAll => {}
        _ => panic!("Expected CloseAll"),
    }
    match reduce_long {
        SignalAction::ReduceLong(q) => assert_eq!(q, Decimal::new(5, 1)),
        _ => panic!("Expected ReduceLong(0.5)"),
    }
    match reduce_short {
        SignalAction::ReduceShort(q) => assert_eq!(q, Decimal::new(5, 1)),
        _ => panic!("Expected ReduceShort(0.5)"),
    }
}

#[test]
fn test_signal_serialization() {
    let signal = Signal {
        action: SignalAction::OpenLong,
        symbol: "BTC-USDT".to_string(),
        quantity: Some(Decimal::from(1)),
        strength: 0.85,
        reason: "Test signal".to_string(),
        stop_loss: Some(Decimal::from(9500)),
        take_profit: Some(Decimal::from(11000)),
    };

    let json = serde_json::to_string(&signal).expect("Should serialize");
    let deserialized: Signal = serde_json::from_str(&json).expect("Should deserialize");

    assert_eq!(signal.action, deserialized.action);
    assert_eq!(signal.symbol, deserialized.symbol);
    assert_eq!(signal.quantity, deserialized.quantity);
    assert_eq!(signal.strength, deserialized.strength);
    assert_eq!(signal.reason, deserialized.reason);
    assert_eq!(signal.stop_loss, deserialized.stop_loss);
    assert_eq!(signal.take_profit, deserialized.take_profit);
}

// ------------------------------------------------------------------
// 4. StrategyContext Tests
// ------------------------------------------------------------------

#[test]
fn test_context_creation() {
    let bars = create_trending_bars("up", 10);
    let ctx = StrategyContext {
        current_bar: &bars[0],
        historical_bars: &bars[..1],
        current_idx: 1,
        positions: &[],
        equity: Decimal::from(100000),
        available_balance: Decimal::from(50000),
    };

    assert_eq!(ctx.current_bar.symbol, "BTC-USDT");
    assert_eq!(ctx.historical_bars.len(), 1);
    assert_eq!(ctx.current_idx, 1);
    assert!(ctx.positions.is_empty());
    assert_eq!(ctx.equity, Decimal::from(100000));
    assert_eq!(ctx.available_balance, Decimal::from(50000));
}

// ------------------------------------------------------------------
// 5. Edge Case Tests
// ------------------------------------------------------------------

#[test]
fn test_strategy_with_empty_bars() {
    let _bars: Vec<StandardBar> = vec![];

    // AlwaysLong doesn't reference bars, so it still emits one signal
    // (the context is invalid with empty bars, so we skip this specific case)
    // Instead, test EmaCrossover with minimal bars
    let mut ema = EmaCrossover {
        symbol: "BTC-USDT".to_string(),
        quantity: Decimal::from(1),
        fast_period: 5,
        slow_period: 10,
    };

    // With empty bars, the strategy can't compute EMAs
    // We verify it gracefully returns empty (can't call on_bar with empty bars
    // because current_bar reference would be invalid)
    // So we test with single bar instead
    let single_bar = create_trending_bars("up", 1);
    let ctx = make_context(&single_bar, 0);
    let signals = ema.on_bar(&ctx);
    assert!(
        signals.is_empty(),
        "EmaCrossover should return empty with insufficient data"
    );
}

#[test]
fn test_strategy_with_single_bar() {
    let bars = create_trending_bars("up", 1);
    let mut strategy = EmaCrossover {
        symbol: "BTC-USDT".to_string(),
        quantity: Decimal::from(1),
        fast_period: 5,
        slow_period: 10,
    };

    let ctx = make_context(&bars, 0);
    let signals = strategy.on_bar(&ctx);
    assert!(
        signals.is_empty(),
        "Single bar is not enough for EMA calculation"
    );
}

#[test]
fn test_signal_with_zero_quantity() {
    let signal = Signal {
        action: SignalAction::ReduceLong(Decimal::ZERO),
        symbol: "BTC-USDT".to_string(),
        quantity: Some(Decimal::ZERO),
        strength: 0.5,
        reason: "Zero quantity test".to_string(),
        stop_loss: None,
        take_profit: None,
    };

    assert_eq!(signal.quantity, Some(Decimal::ZERO));
    if let SignalAction::ReduceLong(q) = signal.action {
        assert_eq!(q, Decimal::ZERO);
    } else {
        panic!("Expected ReduceLong");
    }
}

// ------------------------------------------------------------------
// 6. Integration Tests with Engine
// ------------------------------------------------------------------

#[test]
fn test_always_long_backtest() {
    use engine::{BacktestEngine, EngineConfig};
    use orderbook::{CostBasisMethod, MarginMode};

    let bars = create_trending_bars("up", 100);
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

    let strategy = Box::new(AlwaysLong::new("BTC-USDT".to_string(), Decimal::from(1)));
    let mut engine = BacktestEngine::new(config, bars, Some(strategy));
    let result = engine.run().expect("Backtest should complete");

    assert_eq!(
        result.total_trades, 1,
        "AlwaysLong should produce exactly one trade"
    );
    assert!(
        result.final_equity > Decimal::ZERO,
        "Final equity should be positive"
    );
    assert!(
        result.max_drawdown_pct >= 0.0,
        "Max drawdown should be non-negative"
    );
}

#[test]
fn test_ema_crossover_backtest() {
    use engine::{BacktestEngine, EngineConfig};
    use orderbook::{CostBasisMethod, MarginMode};

    // Create bars with three phases (down, up, down) to ensure both
    // bullish and bearish crossovers occur.
    let bars = create_three_phase_bars();

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

    let strategy = Box::new(EmaCrossover {
        symbol: "BTC-USDT".to_string(),
        quantity: Decimal::from(1),
        fast_period: 5,
        slow_period: 10,
    });

    let mut engine = BacktestEngine::new(config, bars, Some(strategy));
    let result = engine.run().expect("EMA backtest should complete");

    // Should have at least one trade (open long)
    assert!(
        result.total_trades >= 1,
        "EMA crossover should produce at least one trade, got {}",
        result.total_trades
    );
    assert!(
        result.final_equity > Decimal::ZERO,
        "Final equity should be positive"
    );
    assert!(
        result.win_rate >= 0.0 && result.win_rate <= 1.0,
        "Win rate should be between 0 and 1"
    );
}
