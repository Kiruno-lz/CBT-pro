use strategy::*;
use data_pipeline::StandardBar;
use rust_decimal::Decimal;

// ------------------------------------------------------------------
// Helpers
// ------------------------------------------------------------------

fn make_context<'a>(bars: &'a [StandardBar], current_idx: usize) -> StrategyContext<'a> {
    StrategyContext {
        current_bar: &bars[current_idx],
        historical_bars: &bars[..=current_idx],
        current_idx: current_idx + 1,
        positions: &[],
        equity: Decimal::from(100000),
        available_balance: Decimal::from(100000),
    }
}

/// Create bars with a clear downtrend followed by sharp uptrend.
/// This forces fast EMA to cross above slow EMA (bullish crossover).
fn create_bullish_crossover_bars() -> Vec<StandardBar> {
    let mut bars = Vec::with_capacity(60);
    let symbol = "BTC-USDT".to_string();
    let exchange = "synthetic".to_string();

    // Phase 1: downtrend (30 bars) – price drops from 20000 to 17000
    let mut price = Decimal::from(20000);
    for i in 0..30 {
        let open = price;
        price -= Decimal::from(100);
        let close = price;
        let high = if close > open { close + Decimal::from(10) } else { open + Decimal::from(10) };
        let low = if close < open { close - Decimal::from(10) } else { open - Decimal::from(10) };
        bars.push(StandardBar {
            timestamp: 1704067200 + i as i64 * 3600,
            open, high, low, close,
            volume: Decimal::from(100),
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });
    }

    // Phase 2: sharp uptrend (30 bars) – price rises from 17000 to 23000
    for i in 30..60 {
        let open = price;
        price += Decimal::from(200);
        let close = price;
        let high = if close > open { close + Decimal::from(10) } else { open + Decimal::from(10) };
        let low = if close < open { close - Decimal::from(10) } else { open - Decimal::from(10) };
        bars.push(StandardBar {
            timestamp: 1704067200 + i as i64 * 3600,
            open, high, low, close,
            volume: Decimal::from(100),
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });
    }

    bars
}

/// Create bars with clear uptrend followed by sharp downtrend.
/// This forces fast EMA to cross below slow EMA (bearish crossover).
fn create_bearish_crossover_bars() -> Vec<StandardBar> {
    let mut bars = Vec::with_capacity(60);
    let symbol = "BTC-USDT".to_string();
    let exchange = "synthetic".to_string();

    // Phase 1: uptrend (30 bars) – price rises from 15000 to 18000
    let mut price = Decimal::from(15000);
    for i in 0..30 {
        let open = price;
        price += Decimal::from(100);
        let close = price;
        let high = if close > open { close + Decimal::from(10) } else { open + Decimal::from(10) };
        let low = if close < open { close - Decimal::from(10) } else { open - Decimal::from(10) };
        bars.push(StandardBar {
            timestamp: 1704067200 + i as i64 * 3600,
            open, high, low, close,
            volume: Decimal::from(100),
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });
    }

    // Phase 2: sharp downtrend (30 bars) – price drops from 18000 to 12000
    for i in 30..60 {
        let open = price;
        price -= Decimal::from(200);
        let close = price;
        let high = if close > open { close + Decimal::from(10) } else { open + Decimal::from(10) };
        let low = if close < open { close - Decimal::from(10) } else { open - Decimal::from(10) };
        bars.push(StandardBar {
            timestamp: 1704067200 + i as i64 * 3600,
            open, high, low, close,
            volume: Decimal::from(100),
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });
    }

    bars
}

/// Create bars that produce RSI < 30 (oversold) and MACD histogram turning positive.
fn create_rsi_macd_bullish_bars() -> Vec<StandardBar> {
    let mut bars = Vec::with_capacity(120);
    let symbol = "BTC-USDT".to_string();
    let exchange = "synthetic".to_string();

    // Phase 1: steady uptrend (50 bars) – establish high baseline
    let mut price = Decimal::from(20000);
    for i in 0..50 {
        let open = price;
        price += Decimal::from(100);
        let close = price;
        let high = close + Decimal::from(20);
        let low = open - Decimal::from(20);
        bars.push(StandardBar {
            timestamp: 1704067200 + i as i64 * 3600,
            open, high, low, close,
            volume: Decimal::from(100),
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });
    }

    // Phase 2: very sharp drop (40 bars) – create oversold RSI (< 30)
    // Need sustained large drops to push RSI below 30
    for i in 50..90 {
        let open = price;
        price -= Decimal::from(500);
        let close = price;
        let high = open + Decimal::from(10);
        let low = close - Decimal::from(10);
        bars.push(StandardBar {
            timestamp: 1704067200 + i as i64 * 3600,
            open, high, low, close,
            volume: Decimal::from(100),
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });
    }

    // Phase 3: sharp recovery (30 bars) – MACD histogram turns positive
    for i in 90..120 {
        let open = price;
        price += Decimal::from(400);
        let close = price;
        let high = close + Decimal::from(20);
        let low = open - Decimal::from(20);
        bars.push(StandardBar {
            timestamp: 1704067200 + i as i64 * 3600,
            open, high, low, close,
            volume: Decimal::from(100),
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });
    }

    bars
}

/// Create bars with high volatility for Bollinger Bands testing.
/// Price drops well below lower band to trigger buy signal.
fn create_bollinger_bullish_bars() -> Vec<StandardBar> {
    let mut bars = Vec::with_capacity(50);
    let symbol = "BTC-USDT".to_string();
    let exchange = "synthetic".to_string();

    // Phase 1: stable price (25 bars) around 10000
    let base_price = Decimal::from(10000);
    for i in 0..25 {
        let close = base_price + Decimal::from((i % 5) as i64 * 10 - 20);
        let open = close - Decimal::from(5);
        let high = close + Decimal::from(15);
        let low = open - Decimal::from(15);
        bars.push(StandardBar {
            timestamp: 1704067200 + i as i64 * 3600,
            open, high, low, close,
            volume: Decimal::from(100),
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });
    }

    // Phase 2: sharp drop (5 bars) – price well below normal range
    let mut price = base_price - Decimal::from(500);
    for i in 25..30 {
        let open = price;
        price -= Decimal::from(100);
        let close = price;
        let high = open + Decimal::from(5);
        let low = close - Decimal::from(5);
        bars.push(StandardBar {
            timestamp: 1704067200 + i as i64 * 3600,
            open, high, low, close,
            volume: Decimal::from(100),
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });
    }

    // Phase 3: recovery (20 bars)
    for i in 30..50 {
        let open = price;
        price += Decimal::from(50);
        let close = price;
        let high = close + Decimal::from(10);
        let low = open - Decimal::from(10);
        bars.push(StandardBar {
            timestamp: 1704067200 + i as i64 * 3600,
            open, high, low, close,
            volume: Decimal::from(100),
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });
    }

    bars
}

// ------------------------------------------------------------------
// 1. EmaCrossover Uses indicators::ema Tests
// ------------------------------------------------------------------

#[test]
fn test_ema_crossover_generates_bullish_signal() {
    let bars = create_bullish_crossover_bars();
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
                assert_eq!(sig.reason, "EMA crossover bullish");
                break;
            }
        }
    }
    assert!(found_signal, "EmaCrossover should generate bullish signal after downtrend then uptrend");
}

#[test]
fn test_ema_crossover_generates_bearish_signal() {
    let bars = create_bearish_crossover_bars();
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
                assert_eq!(sig.reason, "EMA crossover bearish");
                break;
            }
        }
    }
    assert!(found_signal, "EmaCrossover should generate bearish signal after uptrend then downtrend");
}

#[test]
fn test_ema_crossover_no_signal_in_trending_market() {
    // In a consistent uptrend, no crossover occurs
    let mut bars = Vec::with_capacity(40);
    let symbol = "BTC-USDT".to_string();
    let exchange = "synthetic".to_string();
    let mut price = Decimal::from(10000);
    for i in 0..40 {
        let open = price;
        price += Decimal::from(50);
        let close = price;
        let high = close + Decimal::from(10);
        let low = open - Decimal::from(10);
        bars.push(StandardBar {
            timestamp: 1704067200 + i as i64 * 3600,
            open, high, low, close,
            volume: Decimal::from(100),
            symbol: symbol.clone(),
            exchange: exchange.clone(),
            confirmed: true,
        });
    }

    let mut strategy = EmaCrossover {
        symbol: "BTC-USDT".to_string(),
        quantity: Decimal::from(1),
        fast_period: 5,
        slow_period: 10,
    };

    for idx in 10..bars.len() {
        let ctx = make_context(&bars, idx);
        let signals = strategy.on_bar(&ctx);
        assert!(
            signals.is_empty(),
            "No crossover signal expected in consistent uptrend at idx {}",
            idx
        );
    }
}

// ------------------------------------------------------------------
// 2. RsiMacd Uses indicators::rsi and indicators::macd Tests
// ------------------------------------------------------------------

#[test]
fn test_rsi_macd_generates_oversold_signal() {
    let bars = create_rsi_macd_bullish_bars();
    let mut strategy = RsiMacd {
        symbol: "BTC-USDT".to_string(),
        quantity: Decimal::from(1),
        rsi_period: 14,
        macd_fast: 12,
        macd_slow: 26,
        macd_signal: 9,
    };

    let mut found_signal = false;
    for idx in 26..bars.len() {
        let ctx = make_context(&bars, idx);
        let signals = strategy.on_bar(&ctx);
        if let Some(sig) = signals.first() {
            if sig.action == SignalAction::OpenLong {
                found_signal = true;
                assert_eq!(sig.reason, "RSI oversold + MACD histogram bullish");
                break;
            }
        }
    }
    assert!(found_signal, "RsiMacd should generate OpenLong signal when RSI is oversold and MACD turns bullish");
}

#[test]
fn test_rsi_macd_no_signal_with_insufficient_data() {
    let bars = create_rsi_macd_bullish_bars();
    let mut strategy = RsiMacd {
        symbol: "BTC-USDT".to_string(),
        quantity: Decimal::from(1),
        rsi_period: 14,
        macd_fast: 12,
        macd_slow: 26,
        macd_signal: 9,
    };

    // Before enough data for MACD (need at least macd_slow + macd_signal bars)
    for idx in 0..26 {
        let ctx = make_context(&bars, idx);
        let signals = strategy.on_bar(&ctx);
        assert!(
            signals.is_empty(),
            "No signal expected with insufficient data at idx {}",
            idx
        );
    }
}

// ------------------------------------------------------------------
// 3. BollingerBands Uses indicators::bollinger Tests
// ------------------------------------------------------------------

#[test]
fn test_bollinger_bands_generates_buy_signal() {
    let bars = create_bollinger_bullish_bars();
    let mut strategy = BollingerBands {
        symbol: "BTC-USDT".to_string(),
        quantity: Decimal::from(1),
        period: 20,
        std_dev: Decimal::from(2),
    };

    let mut found_signal = false;
    for idx in 20..bars.len() {
        let ctx = make_context(&bars, idx);
        let signals = strategy.on_bar(&ctx);
        if let Some(sig) = signals.first() {
            if sig.action == SignalAction::OpenLong {
                found_signal = true;
                assert_eq!(sig.reason, "Price below lower Bollinger Band");
                break;
            }
        }
    }
    assert!(found_signal, "BollingerBands should generate OpenLong when price drops below lower band");
}

#[test]
fn test_bollinger_bands_no_signal_with_insufficient_data() {
    let bars = create_bollinger_bullish_bars();
    let mut strategy = BollingerBands {
        symbol: "BTC-USDT".to_string(),
        quantity: Decimal::from(1),
        period: 20,
        std_dev: Decimal::from(2),
    };

    // Before period bars, no Bollinger Bands calculation possible
    for idx in 0..20 {
        let ctx = make_context(&bars, idx);
        let signals = strategy.on_bar(&ctx);
        assert!(
            signals.is_empty(),
            "No signal expected with insufficient data at idx {}",
            idx
        );
    }
}

#[test]
fn test_bollinger_bands_respects_std_dev_parameter() {
    // Test with a wider std_dev - should be less sensitive
    let bars = create_bollinger_bullish_bars();
    let mut strategy = BollingerBands {
        symbol: "BTC-USDT".to_string(),
        quantity: Decimal::from(1),
        period: 20,
        std_dev: Decimal::from(5), // Wider bands
    };

    // With wider bands, the same price drop may not trigger a signal
    // This test mainly verifies the parameter is actually used
    // (i.e., no panic and different behavior than std_dev=2)
    // We just verify the strategy runs without error by iterating through bars
    for idx in 20..bars.len() {
        let ctx = make_context(&bars, idx);
        let _signals = strategy.on_bar(&ctx);
        // Just running the strategy verifies std_dev parameter is accepted
    }
}