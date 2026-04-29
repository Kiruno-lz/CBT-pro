use crate::always_long::AlwaysLong;
use crate::base::Strategy;
use crate::bollinger_bands::BollingerBands;
use crate::breakout::Breakout;
use crate::ema_crossover::EmaCrossover;
use crate::rsi_macd::RsiMacd;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParamType {
    Integer {
        min: i64,
        max: i64,
        default: i64,
    },
    Decimal {
        min: String,
        max: String,
        default: String,
    },
    String {
        default: String,
        options: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDefinition {
    pub name: String,
    pub description: String,
    pub param_type: ParamType,
}

pub struct StrategyInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub default_params: serde_json::Value,
    pub param_definitions: Vec<ParamDefinition>,
    pub create: fn(String, Decimal, serde_json::Value) -> Result<Box<dyn Strategy>, String>,
}

fn create_always_long(
    symbol: String,
    quantity: Decimal,
    _params: serde_json::Value,
) -> Result<Box<dyn Strategy>, String> {
    Ok(Box::new(AlwaysLong::new(symbol, quantity)))
}

fn create_ema_crossover(
    symbol: String,
    quantity: Decimal,
    params: serde_json::Value,
) -> Result<Box<dyn Strategy>, String> {
    let fast_period = params
        .get("fast_period")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(9);
    let slow_period = params
        .get("slow_period")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(21);
    Ok(Box::new(EmaCrossover {
        symbol,
        quantity,
        fast_period,
        slow_period,
    }))
}

fn create_rsi_macd(
    symbol: String,
    quantity: Decimal,
    params: serde_json::Value,
) -> Result<Box<dyn Strategy>, String> {
    let rsi_period = params
        .get("rsi_period")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(14);
    let macd_fast = params
        .get("macd_fast")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(12);
    let macd_slow = params
        .get("macd_slow")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(26);
    let macd_signal = params
        .get("macd_signal")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(9);
    Ok(Box::new(RsiMacd {
        symbol,
        quantity,
        rsi_period,
        macd_fast,
        macd_slow,
        macd_signal,
    }))
}

fn create_bollinger_bands(
    symbol: String,
    quantity: Decimal,
    params: serde_json::Value,
) -> Result<Box<dyn Strategy>, String> {
    let period = params
        .get("period")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(20);
    let std_dev = params
        .get("std_dev")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Decimal>().ok())
        .unwrap_or_else(|| Decimal::from(2));
    Ok(Box::new(BollingerBands {
        symbol,
        quantity,
        period,
        std_dev,
    }))
}

fn create_breakout(
    symbol: String,
    quantity: Decimal,
    params: serde_json::Value,
) -> Result<Box<dyn Strategy>, String> {
    let lookback = params
        .get("lookback")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(20);
    let threshold_pct = params
        .get("threshold_pct")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Decimal>().ok())
        .unwrap_or_else(|| Decimal::from(2));
    Ok(Box::new(Breakout {
        symbol,
        quantity,
        lookback,
        threshold_pct,
    }))
}

pub fn available_strategies() -> Vec<StrategyInfo> {
    vec![
        StrategyInfo {
            id: "always_long",
            name: "Always Long",
            description: "A simple strategy that always opens a long position and never closes it.",
            default_params: serde_json::json!({}),
            param_definitions: vec![],
            create: create_always_long,
        },
        StrategyInfo {
            id: "ema_crossover",
            name: "EMA Crossover",
            description:
                "A trend-following strategy based on exponential moving average crossovers.",
            default_params: serde_json::json!({
                "fast_period": 9,
                "slow_period": 21
            }),
            param_definitions: vec![
                ParamDefinition {
                    name: "fast_period".to_string(),
                    description: "Period for the fast EMA".to_string(),
                    param_type: ParamType::Integer {
                        min: 2,
                        max: 100,
                        default: 9,
                    },
                },
                ParamDefinition {
                    name: "slow_period".to_string(),
                    description: "Period for the slow EMA".to_string(),
                    param_type: ParamType::Integer {
                        min: 2,
                        max: 200,
                        default: 21,
                    },
                },
            ],
            create: create_ema_crossover,
        },
        StrategyInfo {
            id: "rsi_macd",
            name: "RSI + MACD",
            description: "A momentum strategy combining RSI and MACD indicators.",
            default_params: serde_json::json!({
                "rsi_period": 14,
                "macd_fast": 12,
                "macd_slow": 26,
                "macd_signal": 9
            }),
            param_definitions: vec![
                ParamDefinition {
                    name: "rsi_period".to_string(),
                    description: "Period for RSI calculation".to_string(),
                    param_type: ParamType::Integer {
                        min: 2,
                        max: 100,
                        default: 14,
                    },
                },
                ParamDefinition {
                    name: "macd_fast".to_string(),
                    description: "Fast period for MACD".to_string(),
                    param_type: ParamType::Integer {
                        min: 2,
                        max: 100,
                        default: 12,
                    },
                },
                ParamDefinition {
                    name: "macd_slow".to_string(),
                    description: "Slow period for MACD".to_string(),
                    param_type: ParamType::Integer {
                        min: 2,
                        max: 200,
                        default: 26,
                    },
                },
                ParamDefinition {
                    name: "macd_signal".to_string(),
                    description: "Signal period for MACD".to_string(),
                    param_type: ParamType::Integer {
                        min: 2,
                        max: 100,
                        default: 9,
                    },
                },
            ],
            create: create_rsi_macd,
        },
        StrategyInfo {
            id: "bollinger_bands",
            name: "Bollinger Bands",
            description: "A mean-reversion strategy based on Bollinger Bands.",
            default_params: serde_json::json!({
                "period": 20,
                "std_dev": "2.0"
            }),
            param_definitions: vec![
                ParamDefinition {
                    name: "period".to_string(),
                    description: "Period for Bollinger Bands calculation".to_string(),
                    param_type: ParamType::Integer {
                        min: 2,
                        max: 200,
                        default: 20,
                    },
                },
                ParamDefinition {
                    name: "std_dev".to_string(),
                    description: "Number of standard deviations".to_string(),
                    param_type: ParamType::Decimal {
                        min: "0.1".to_string(),
                        max: "10.0".to_string(),
                        default: "2.0".to_string(),
                    },
                },
            ],
            create: create_bollinger_bands,
        },
        StrategyInfo {
            id: "breakout",
            name: "Breakout",
            description: "A volatility strategy that trades breakouts from price ranges.",
            default_params: serde_json::json!({
                "lookback": 20,
                "threshold_pct": "2.0"
            }),
            param_definitions: vec![
                ParamDefinition {
                    name: "lookback".to_string(),
                    description: "Lookback period for breakout detection".to_string(),
                    param_type: ParamType::Integer {
                        min: 2,
                        max: 200,
                        default: 20,
                    },
                },
                ParamDefinition {
                    name: "threshold_pct".to_string(),
                    description: "Percentage threshold for breakout".to_string(),
                    param_type: ParamType::Decimal {
                        min: "0.1".to_string(),
                        max: "50.0".to_string(),
                        default: "2.0".to_string(),
                    },
                },
            ],
            create: create_breakout,
        },
    ]
}
