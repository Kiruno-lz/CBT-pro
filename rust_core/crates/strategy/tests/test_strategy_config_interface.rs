use strategy::*;
use rust_decimal::Decimal;

// ------------------------------------------------------------------
// 1. available_strategies() Tests
// ------------------------------------------------------------------

#[test]
fn test_available_strategies_returns_all_five() {
    let strategies = available_strategies();
    assert_eq!(strategies.len(), 5, "Should return exactly 5 strategies");

    let ids: Vec<&str> = strategies.iter().map(|s| s.id).collect();
    assert!(ids.contains(&"always_long"), "Should include always_long");
    assert!(ids.contains(&"ema_crossover"), "Should include ema_crossover");
    assert!(ids.contains(&"rsi_macd"), "Should include rsi_macd");
    assert!(ids.contains(&"bollinger_bands"), "Should include bollinger_bands");
    assert!(ids.contains(&"breakout"), "Should include breakout");
}

// ------------------------------------------------------------------
// 2. Default Params Tests
// ------------------------------------------------------------------

#[test]
fn test_always_long_default_params() {
    let strategies = available_strategies();
    let always_long = strategies.iter().find(|s| s.id == "always_long").unwrap();
    assert_eq!(always_long.default_params, serde_json::json!({}), "AlwaysLong should have empty default params");
}

#[test]
fn test_ema_crossover_default_params() {
    let strategies = available_strategies();
    let ema = strategies.iter().find(|s| s.id == "ema_crossover").unwrap();
    assert_eq!(ema.default_params.get("fast_period").unwrap().as_u64().unwrap(), 9);
    assert_eq!(ema.default_params.get("slow_period").unwrap().as_u64().unwrap(), 21);
}

#[test]
fn test_rsi_macd_default_params() {
    let strategies = available_strategies();
    let rsi_macd = strategies.iter().find(|s| s.id == "rsi_macd").unwrap();
    assert_eq!(rsi_macd.default_params.get("rsi_period").unwrap().as_u64().unwrap(), 14);
    assert_eq!(rsi_macd.default_params.get("macd_fast").unwrap().as_u64().unwrap(), 12);
    assert_eq!(rsi_macd.default_params.get("macd_slow").unwrap().as_u64().unwrap(), 26);
    assert_eq!(rsi_macd.default_params.get("macd_signal").unwrap().as_u64().unwrap(), 9);
}

#[test]
fn test_bollinger_bands_default_params() {
    let strategies = available_strategies();
    let bb = strategies.iter().find(|s| s.id == "bollinger_bands").unwrap();
    assert_eq!(bb.default_params.get("period").unwrap().as_u64().unwrap(), 20);
    assert_eq!(bb.default_params.get("std_dev").unwrap().as_str().unwrap(), "2.0");
}

#[test]
fn test_breakout_default_params() {
    let strategies = available_strategies();
    let breakout = strategies.iter().find(|s| s.id == "breakout").unwrap();
    assert_eq!(breakout.default_params.get("lookback").unwrap().as_u64().unwrap(), 20);
    assert_eq!(breakout.default_params.get("threshold_pct").unwrap().as_str().unwrap(), "2.0");
}

// ------------------------------------------------------------------
// 3. Param Definitions Tests
// ------------------------------------------------------------------

#[test]
fn test_always_long_param_definitions() {
    let strategies = available_strategies();
    let always_long = strategies.iter().find(|s| s.id == "always_long").unwrap();
    assert!(always_long.param_definitions.is_empty(), "AlwaysLong should have no param definitions");
}

#[test]
fn test_ema_crossover_param_definitions() {
    let strategies = available_strategies();
    let ema = strategies.iter().find(|s| s.id == "ema_crossover").unwrap();
    assert_eq!(ema.param_definitions.len(), 2, "EMA Crossover should have 2 param definitions");

    let fast_period = &ema.param_definitions[0];
    assert_eq!(fast_period.name, "fast_period");
    assert_eq!(fast_period.description, "Period for the fast EMA");
    match &fast_period.param_type {
        ParamType::Integer { min, max, default } => {
            assert_eq!(*min, 2);
            assert_eq!(*max, 100);
            assert_eq!(*default, 9);
        }
        _ => panic!("fast_period should be Integer type"),
    }

    let slow_period = &ema.param_definitions[1];
    assert_eq!(slow_period.name, "slow_period");
    assert_eq!(slow_period.description, "Period for the slow EMA");
    match &slow_period.param_type {
        ParamType::Integer { min, max, default } => {
            assert_eq!(*min, 2);
            assert_eq!(*max, 200);
            assert_eq!(*default, 21);
        }
        _ => panic!("slow_period should be Integer type"),
    }
}

#[test]
fn test_rsi_macd_param_definitions() {
    let strategies = available_strategies();
    let rsi_macd = strategies.iter().find(|s| s.id == "rsi_macd").unwrap();
    assert_eq!(rsi_macd.param_definitions.len(), 4, "RSI MACD should have 4 param definitions");

    let names: Vec<&str> = rsi_macd.param_definitions.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"rsi_period"));
    assert!(names.contains(&"macd_fast"));
    assert!(names.contains(&"macd_slow"));
    assert!(names.contains(&"macd_signal"));
}

#[test]
fn test_bollinger_bands_param_definitions() {
    let strategies = available_strategies();
    let bb = strategies.iter().find(|s| s.id == "bollinger_bands").unwrap();
    assert_eq!(bb.param_definitions.len(), 2, "Bollinger Bands should have 2 param definitions");

    let period = bb.param_definitions.iter().find(|p| p.name == "period").unwrap();
    match &period.param_type {
        ParamType::Integer { min, max, default } => {
            assert_eq!(*min, 2);
            assert_eq!(*max, 200);
            assert_eq!(*default, 20);
        }
        _ => panic!("period should be Integer type"),
    }

    let std_dev = bb.param_definitions.iter().find(|p| p.name == "std_dev").unwrap();
    match &std_dev.param_type {
        ParamType::Decimal { min, max, default } => {
            assert_eq!(min, "0.1");
            assert_eq!(max, "10.0");
            assert_eq!(default, "2.0");
        }
        _ => panic!("std_dev should be Decimal type"),
    }
}

#[test]
fn test_breakout_param_definitions() {
    let strategies = available_strategies();
    let breakout = strategies.iter().find(|s| s.id == "breakout").unwrap();
    assert_eq!(breakout.param_definitions.len(), 2, "Breakout should have 2 param definitions");

    let lookback = breakout.param_definitions.iter().find(|p| p.name == "lookback").unwrap();
    match &lookback.param_type {
        ParamType::Integer { min, max, default } => {
            assert_eq!(*min, 2);
            assert_eq!(*max, 200);
            assert_eq!(*default, 20);
        }
        _ => panic!("lookback should be Integer type"),
    }
}

// ------------------------------------------------------------------
// 4. Factory Function Tests
// ------------------------------------------------------------------

#[test]
fn test_create_always_long() {
    let strategies = available_strategies();
    let always_long = strategies.iter().find(|s| s.id == "always_long").unwrap();

    let mut strategy = (always_long.create)("BTC-USDT".to_string(), Decimal::from(1), serde_json::json!({})).unwrap();
    // Verify it's AlwaysLong by behavior: emits signal on first bar
    use data_pipeline::StandardBar;
    let bar = StandardBar {
        timestamp: 1704067200,
        open: Decimal::from(10000),
        high: Decimal::from(10100),
        low: Decimal::from(9900),
        close: Decimal::from(10050),
        volume: Decimal::from(100),
        symbol: "BTC-USDT".to_string(),
        exchange: "test".to_string(),
        confirmed: true,
    };
    let ctx = StrategyContext {
        current_bar: &bar,
        historical_bars: &[bar.clone()],
        current_idx: 1,
        positions: &[],
        equity: Decimal::from(100000),
        available_balance: Decimal::from(100000),
    };
    let signals = strategy.on_bar(&ctx);
    assert_eq!(signals.len(), 1, "AlwaysLong should emit exactly one signal");
    assert_eq!(signals[0].action, SignalAction::OpenLong);
}

#[test]
fn test_create_ema_crossover_with_defaults() {
    let strategies = available_strategies();
    let ema = strategies.iter().find(|s| s.id == "ema_crossover").unwrap();

    let mut strategy = (ema.create)("BTC-USDT".to_string(), Decimal::from(1), ema.default_params.clone()).unwrap();
    // Verify by behavior with custom params that produce known signals
    let signals = strategy.on_bar(&make_minimal_context());
    // With minimal data, no signal expected
    assert!(signals.is_empty(), "EMA Crossover with insufficient data should return empty");
}

#[test]
fn test_create_ema_crossover_with_custom_params() {
    let strategies = available_strategies();
    let ema = strategies.iter().find(|s| s.id == "ema_crossover").unwrap();

    let custom_params = serde_json::json!({
        "fast_period": 5,
        "slow_period": 15
    });

    let mut strategy = (ema.create)("ETH-USDT".to_string(), Decimal::from(2), custom_params).unwrap();
    let signals = strategy.on_bar(&make_minimal_context());
    assert!(signals.is_empty(), "EMA Crossover with insufficient data should return empty");
}

#[test]
fn test_create_rsi_macd_with_defaults() {
    let strategies = available_strategies();
    let rsi_macd = strategies.iter().find(|s| s.id == "rsi_macd").unwrap();

    let mut strategy = (rsi_macd.create)("BTC-USDT".to_string(), Decimal::from(1), rsi_macd.default_params.clone()).unwrap();
    let signals = strategy.on_bar(&make_minimal_context());
    assert!(signals.is_empty(), "RSI MACD with insufficient data should return empty");
}

#[test]
fn test_create_bollinger_bands_with_defaults() {
    let strategies = available_strategies();
    let bb = strategies.iter().find(|s| s.id == "bollinger_bands").unwrap();

    let mut strategy = (bb.create)("BTC-USDT".to_string(), Decimal::from(1), bb.default_params.clone()).unwrap();
    let signals = strategy.on_bar(&make_minimal_context());
    assert!(signals.is_empty(), "Bollinger Bands with insufficient data should return empty");
}

#[test]
fn test_create_breakout_with_defaults() {
    let strategies = available_strategies();
    let breakout = strategies.iter().find(|s| s.id == "breakout").unwrap();

    let mut strategy = (breakout.create)("BTC-USDT".to_string(), Decimal::from(1), breakout.default_params.clone()).unwrap();
    let signals = strategy.on_bar(&make_minimal_context());
    assert!(signals.is_empty(), "Breakout with insufficient data should return empty");
}

// Helper function
fn make_minimal_context() -> StrategyContext<'static> {
    use data_pipeline::StandardBar;
    // Create a single static bar for minimal context testing
    let bar = StandardBar {
        timestamp: 1704067200,
        open: Decimal::from(10000),
        high: Decimal::from(10100),
        low: Decimal::from(9900),
        close: Decimal::from(10050),
        volume: Decimal::from(100),
        symbol: "BTC-USDT".to_string(),
        exchange: "test".to_string(),
        confirmed: true,
    };
    // Leak to get 'static lifetime for testing
    let bars: &'static [StandardBar] = vec![bar].leak();
    StrategyContext {
        current_bar: &bars[0],
        historical_bars: bars,
        current_idx: 1,
        positions: &[],
        equity: Decimal::from(100000),
        available_balance: Decimal::from(100000),
    }
}