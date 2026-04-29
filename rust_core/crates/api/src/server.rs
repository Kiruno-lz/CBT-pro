//! Axum REST API server for CBT-Pro engine.
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use engine::{BacktestEngine, EngineConfig, EngineSnapshot, BacktestResult};
use orderbook::{OrderRequest, OrderFill, OrderSide, Direction, OrderType, MarginMode};
use data_pipeline::{StandardBar, TimeFrame};
use rust_decimal::Decimal;
use std::str::FromStr;
use rand::rngs::SmallRng;
use rand::SeedableRng;
use rand::Rng;
use indicators;

/// Application state shared across handlers.
pub type AppState = Arc<Mutex<HashMap<String, BacktestEngine>>>;

#[derive(Serialize)]
struct StartBacktestRequest {
    config: EngineConfig,
    strategy_id: String,
    timeframe: String,
    start_time: i64,
    end_time: i64,
}

#[derive(Serialize)]
struct StartBacktestResponse {
    backtest_id: String,
    status: String,
    total_bars: usize,
}

#[derive(Serialize)]
struct GenericResponse {
    status: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

pub fn create_rest_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/backtest/start", post(start_backtest))
        .route("/api/v1/backtest/:id/pause", post(pause_backtest))
        .route("/api/v1/backtest/:id/resume", post(resume_backtest))
        .route("/api/v1/backtest/:id/state", get(get_backtest_state))
        .route("/api/v1/backtest/:id/result", get(get_backtest_result))
        .route("/api/v1/order", post(submit_order))
        .route("/api/v1/indicators", get(get_indicators))
        .route("/api/v1/strategies", get(list_strategies))
        .route("/api/v1/strategies/:id/defaults", get(get_strategy_defaults))
}

async fn health_check() -> &'static str {
    "ok"
}

async fn start_backtest(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<StartBacktestResponse>, (StatusCode, Json<ErrorResponse>)> {
    let backtest_id = Uuid::new_v4().to_string();

    // Parse config from payload
    let config = match parse_engine_config(&payload) {
        Ok(c) => c,
        Err(e) => return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e }))),
    };

    // Parse timeframe
    let timeframe_str = payload.get("timeframe").and_then(|v| v.as_str()).unwrap_or("1m");
    let timeframe = TimeFrame::from_string(timeframe_str).unwrap_or(TimeFrame::M1);

    // Determine bar count from date range if provided
    let start_time = payload.get("start_time").and_then(|v| v.as_i64()).unwrap_or(1704067200000);
    let end_time = payload.get("end_time").and_then(|v| v.as_i64()).unwrap_or(1735689600000);
    let duration_ms = (end_time - start_time).max(0);
    let step_ms = timeframe.as_seconds() * 1000;
    let count = ((duration_ms / step_ms) as usize).max(100).min(10000);

    let bars = generate_synthetic_bars(&config.symbol, count, timeframe);
    let total_bars = bars.len();

    // Build strategy
    let strategy_id = payload.get("strategy_id").and_then(|v| v.as_str()).unwrap_or("always_long");
    let strategy_params = payload.get("strategy_params").cloned().unwrap_or(serde_json::json!({}));
    let strategy = strategy::available_strategies()
        .into_iter()
        .find(|s| s.id == strategy_id)
        .and_then(|info| {
            let quantity = Decimal::from_str("0.1").unwrap();
            (info.create)(config.symbol.clone(), quantity, strategy_params).ok()
        });

    let engine = BacktestEngine::new(config, bars, strategy);
    state.lock().await.insert(backtest_id.clone(), engine);

    Ok(Json(StartBacktestResponse {
        backtest_id,
        status: "running".to_string(),
        total_bars,
    }))
}

async fn pause_backtest(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<GenericResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut engines = state.lock().await;
    match engines.get_mut(&id) {
        Some(_engine) => {
            // Pause logic would go here - for MVP, just acknowledge
            Ok(Json(GenericResponse { status: "paused".to_string() }))
        }
        None => Err((StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: format!("Backtest {} not found", id),
        }))),
    }
}

async fn resume_backtest(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<GenericResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut engines = state.lock().await;
    match engines.get_mut(&id) {
        Some(_engine) => {
            Ok(Json(GenericResponse { status: "running".to_string() }))
        }
        None => Err((StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: format!("Backtest {} not found", id),
        }))),
    }
}

async fn get_backtest_state(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<EngineSnapshot>, (StatusCode, Json<ErrorResponse>)> {
    let engines = state.lock().await;
    match engines.get(&id) {
        Some(engine) => {
            let snapshot = engine.get_state();
            Ok(Json(snapshot))
        }
        None => Err((StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: format!("Backtest {} not found", id),
        }))),
    }
}

async fn get_backtest_result(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<BacktestResult>, (StatusCode, Json<ErrorResponse>)> {
    let mut engines = state.lock().await;
    match engines.remove(&id) {
        Some(mut engine) => {
            match engine.run() {
                Ok(result) => Ok(Json(result)),
                Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Engine error: {}", e),
                }))),
            }
        }
        None => Err((StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: format!("Backtest {} not found", id),
        }))),
    }
}

async fn submit_order(
    State(_state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<OrderFill>, (StatusCode, Json<ErrorResponse>)> {
    // Parse order request
    let order = match parse_order_request(payload) {
        Ok(o) => o,
        Err(e) => return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e }))),
    };

    // For MVP, return a simulated fill
    let fill = OrderFill {
        order_id: order.order_id,
        position_id: Some(Uuid::new_v4()),
        symbol: order.symbol.clone(),
        side: order.side,
        direction: order.direction,
        filled_price: Decimal::from(42000),
        filled_quantity: order.quantity,
        fee: Decimal::from(1),
        fee_asset: order.symbol.clone(),
        timestamp: order.timestamp,
        realized_pnl: None,
    };

    Ok(Json(fill))
}

#[derive(Deserialize)]
struct IndicatorQuery {
    symbol: String,
    timeframe: String,
    indicators: String,
    backtest_id: Option<String>,
    full: Option<bool>,
}

#[derive(Debug, Clone)]
enum IndicatorType {
    Ema(usize),
    Rsi(usize),
    Macd(usize, usize, usize),
    Bollinger(usize, Decimal),
    Atr(usize),
    Vwap,
}

fn parse_indicator_name(name: &str) -> Result<IndicatorType, String> {
    let parts: Vec<&str> = name.split('_').collect();
    
    match parts.as_slice() {
        ["ema", period] => {
            let p = period.parse::<usize>().map_err(|e| format!("Invalid EMA period: {}", e))?;
            Ok(IndicatorType::Ema(p))
        }
        ["rsi", period] => {
            let p = period.parse::<usize>().map_err(|e| format!("Invalid RSI period: {}", e))?;
            Ok(IndicatorType::Rsi(p))
        }
        ["macd", fast, slow, signal] => {
            let f = fast.parse::<usize>().map_err(|e| format!("Invalid MACD fast period: {}", e))?;
            let s = slow.parse::<usize>().map_err(|e| format!("Invalid MACD slow period: {}", e))?;
            let sig = signal.parse::<usize>().map_err(|e| format!("Invalid MACD signal period: {}", e))?;
            Ok(IndicatorType::Macd(f, s, sig))
        }
        ["bollinger", period, rest @ ..] if !rest.is_empty() => {
            let p = period.parse::<usize>().map_err(|e| format!("Invalid Bollinger period: {}", e))?;
            let sd_str = rest.join(".");
            let sd = Decimal::from_str(&sd_str).map_err(|e| format!("Invalid Bollinger std_dev: {}", e))?;
            Ok(IndicatorType::Bollinger(p, sd))
        }
        ["atr", period] => {
            let p = period.parse::<usize>().map_err(|e| format!("Invalid ATR period: {}", e))?;
            Ok(IndicatorType::Atr(p))
        }
        ["vwap"] => Ok(IndicatorType::Vwap),
        _ => Err(format!("Unknown indicator: {}", name)),
    }
}

fn calculate_indicator_last(
    name: &str,
    closes: &[Decimal],
    highs: &[Decimal],
    lows: &[Decimal],
    volumes: &[Decimal],
) -> Result<Value, String> {
    let indicator = parse_indicator_name(name)?;
    
    let value = match indicator {
        IndicatorType::Ema(period) => {
            match indicators::ema::ema(period, closes) {
                Ok(vals) => vals.last().map(|v| {
                    serde_json::json!({
                        "value": v.value.to_string(),
                        "timestamp": v.timestamp
                    })
                }),
                Err(e) => return Err(format!("EMA calculation error: {}", e)),
            }
        }
        IndicatorType::Rsi(period) => {
            match indicators::rsi::rsi(period, closes) {
                Ok(vals) => vals.last().map(|v| {
                    serde_json::json!({
                        "value": v.value.to_string(),
                        "timestamp": v.timestamp
                    })
                }),
                Err(e) => return Err(format!("RSI calculation error: {}", e)),
            }
        }
        IndicatorType::Bollinger(period, std_dev) => {
            match indicators::bollinger::bollinger(period, std_dev, closes) {
                Ok(vals) => vals.last().map(|(ir, bb)| {
                    serde_json::json!({
                        "upper": bb.upper.to_string(),
                        "middle": bb.middle.to_string(),
                        "lower": bb.lower.to_string(),
                        "timestamp": ir.timestamp
                    })
                }),
                Err(e) => return Err(format!("Bollinger Bands calculation error: {}", e)),
            }
        }
        IndicatorType::Macd(fast, slow, signal) => {
            match indicators::macd::macd(fast, slow, signal, closes) {
                Ok(vals) => vals.last().map(|(ir, mr)| {
                    serde_json::json!({
                        "macd": mr.macd.to_string(),
                        "signal": mr.signal.to_string(),
                        "histogram": mr.histogram.to_string(),
                        "timestamp": ir.timestamp
                    })
                }),
                Err(e) => return Err(format!("MACD calculation error: {}", e)),
            }
        }
        IndicatorType::Atr(period) => {
            match indicators::atr::atr(period, highs, lows, closes) {
                Ok(vals) => vals.last().map(|v| {
                    serde_json::json!({
                        "value": v.value.to_string(),
                        "timestamp": v.timestamp
                    })
                }),
                Err(e) => return Err(format!("ATR calculation error: {}", e)),
            }
        }
        IndicatorType::Vwap => {
            match indicators::vwap::vwap(closes, volumes) {
                Ok(vals) => vals.last().map(|v| {
                    serde_json::json!({
                        "value": v.value.to_string(),
                        "timestamp": v.timestamp
                    })
                }),
                Err(e) => return Err(format!("VWAP calculation error: {}", e)),
            }
        }
    };

    Ok(value.unwrap_or(Value::Null))
}

fn calculate_indicator_series(
    name: &str,
    closes: &[Decimal],
    highs: &[Decimal],
    lows: &[Decimal],
    volumes: &[Decimal],
    bars: &[StandardBar],
) -> Result<Value, String> {
    let indicator = parse_indicator_name(name)?;
    
    let series = match indicator {
        IndicatorType::Ema(period) => {
            match indicators::ema::ema(period, closes) {
                Ok(vals) => {
                    let arr: Vec<Value> = vals.iter().map(|v| {
                        let ts = bars.get(v.timestamp as usize).map(|b| b.timestamp).unwrap_or(v.timestamp);
                        serde_json::json!({
                            "value": v.value.to_string(),
                            "timestamp": ts
                        })
                    }).collect();
                    Value::Array(arr)
                }
                Err(e) => return Err(format!("EMA calculation error: {}", e)),
            }
        }
        IndicatorType::Rsi(period) => {
            match indicators::rsi::rsi(period, closes) {
                Ok(vals) => {
                    let arr: Vec<Value> = vals.iter().map(|v| {
                        let ts = bars.get(v.timestamp as usize).map(|b| b.timestamp).unwrap_or(v.timestamp);
                        serde_json::json!({
                            "value": v.value.to_string(),
                            "timestamp": ts
                        })
                    }).collect();
                    Value::Array(arr)
                }
                Err(e) => return Err(format!("RSI calculation error: {}", e)),
            }
        }
        IndicatorType::Bollinger(period, std_dev) => {
            match indicators::bollinger::bollinger(period, std_dev, closes) {
                Ok(vals) => {
                    let arr: Vec<Value> = vals.iter().map(|(ir, bb)| {
                        let ts = bars.get(ir.timestamp as usize).map(|b| b.timestamp).unwrap_or(ir.timestamp);
                        serde_json::json!({
                            "upper": bb.upper.to_string(),
                            "middle": bb.middle.to_string(),
                            "lower": bb.lower.to_string(),
                            "timestamp": ts
                        })
                    }).collect();
                    Value::Array(arr)
                }
                Err(e) => return Err(format!("Bollinger Bands calculation error: {}", e)),
            }
        }
        IndicatorType::Macd(fast, slow, signal) => {
            match indicators::macd::macd(fast, slow, signal, closes) {
                Ok(vals) => {
                    let arr: Vec<Value> = vals.iter().map(|(ir, mr)| {
                        let ts = bars.get(ir.timestamp as usize).map(|b| b.timestamp).unwrap_or(ir.timestamp);
                        serde_json::json!({
                            "macd": mr.macd.to_string(),
                            "signal": mr.signal.to_string(),
                            "histogram": mr.histogram.to_string(),
                            "timestamp": ts
                        })
                    }).collect();
                    Value::Array(arr)
                }
                Err(e) => return Err(format!("MACD calculation error: {}", e)),
            }
        }
        IndicatorType::Atr(period) => {
            match indicators::atr::atr(period, highs, lows, closes) {
                Ok(vals) => {
                    let arr: Vec<Value> = vals.iter().map(|v| {
                        let ts = bars.get(v.timestamp as usize).map(|b| b.timestamp).unwrap_or(v.timestamp);
                        serde_json::json!({
                            "value": v.value.to_string(),
                            "timestamp": ts
                        })
                    }).collect();
                    Value::Array(arr)
                }
                Err(e) => return Err(format!("ATR calculation error: {}", e)),
            }
        }
        IndicatorType::Vwap => {
            match indicators::vwap::vwap(closes, volumes) {
                Ok(vals) => {
                    let arr: Vec<Value> = vals.iter().map(|v| {
                        let ts = bars.get(v.timestamp as usize).map(|b| b.timestamp).unwrap_or(v.timestamp);
                        serde_json::json!({
                            "value": v.value.to_string(),
                            "timestamp": ts
                        })
                    }).collect();
                    Value::Array(arr)
                }
                Err(e) => return Err(format!("VWAP calculation error: {}", e)),
            }
        }
    };

    Ok(series)
}

async fn get_indicators(
    State(state): State<AppState>,
    Query(params): Query<IndicatorQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let indicator_names: Vec<&str> = params.indicators.split(',').collect();
    let mut result = serde_json::Map::new();

    let bars: Vec<StandardBar>;
    if let Some(ref backtest_id) = params.backtest_id {
        let engines = state.lock().await;
        match engines.get(backtest_id) {
            Some(engine) => {
                let processed = engine.processed_bars();
                if processed.is_empty() {
                    // No bars processed yet, return empty result
                    return Ok(Json(Value::Object(result)));
                }
                bars = processed.to_vec();
            }
            None => {
                return Err((StatusCode::NOT_FOUND, Json(ErrorResponse {
                    error: format!("Backtest {} not found", backtest_id),
                })));
            }
        }
    } else {
        let timeframe = TimeFrame::from_string(&params.timeframe).unwrap_or(TimeFrame::M1);
        bars = generate_synthetic_bars(&params.symbol, 200, timeframe);
    }

    let closes: Vec<Decimal> = bars.iter().map(|b| b.close).collect();
    let highs: Vec<Decimal> = bars.iter().map(|b| b.high).collect();
    let lows: Vec<Decimal> = bars.iter().map(|b| b.low).collect();
    let volumes: Vec<Decimal> = bars.iter().map(|b| b.volume).collect();

    let full = params.full.unwrap_or(false);

    for name in indicator_names {
        let indicator_value = if full {
            match calculate_indicator_series(name, &closes, &highs, &lows, &volumes, &bars) {
                Ok(v) => v,
                Err(e) => return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e }))),
            }
        } else {
            match calculate_indicator_last(name, &closes, &highs, &lows, &volumes) {
                Ok(v) => v,
                Err(e) => return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e }))),
            }
        };

        result.insert(name.to_string(), indicator_value);
    }

    Ok(Json(Value::Object(result)))
}

async fn list_strategies() -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let strategies = strategy::available_strategies();
    let mut result = Vec::new();

    for info in strategies {
        result.push(serde_json::json!({
            "id": info.id,
            "name": info.name,
            "description": info.description,
            "default_params": info.default_params,
        }));
    }

    Ok(Json(Value::Array(result)))
}

async fn get_strategy_defaults(
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let strategies = strategy::available_strategies();

    match strategies.into_iter().find(|s| s.id == id) {
        Some(info) => {
            Ok(Json(serde_json::json!({
                "id": info.id,
                "name": info.name,
                "description": info.description,
                "default_params": info.default_params,
                "param_definitions": info.param_definitions,
            })))
        }
        None => Err((StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: format!("Strategy {} not found", id),
        }))),
    }
}

fn parse_engine_config(payload: &Value) -> Result<EngineConfig, String> {
    let config_obj = payload.get("config").ok_or("missing config field")?;

    let symbol = config_obj
        .get("symbol")
        .and_then(|s| s.as_str())
        .unwrap_or("BTC-USDT")
        .to_string();

    let initial_balance = config_obj
        .get("initial_balance")
        .and_then(|v| v.as_str())
        .and_then(|s| Decimal::from_str(s).ok())
        .unwrap_or_else(|| Decimal::from(100000));

    let margin_mode = match config_obj.get("margin_mode").and_then(|v| v.as_str()) {
        Some("Isolated") => MarginMode::Isolated,
        Some("Cross") => MarginMode::Cross,
        _ => MarginMode::Cross,
    };

    let default_leverage = config_obj
        .get("default_leverage")
        .and_then(|v| v.as_str())
        .and_then(|s| Decimal::from_str(s).ok())
        .unwrap_or_else(|| Decimal::from(10));

    Ok(EngineConfig {
        symbol,
        initial_balance,
        margin_mode,
        default_leverage,
        default_quantity: Decimal::from_str("0.1").unwrap(),
        maker_fee_rate: Decimal::from_str("0.0002").unwrap(),
        taker_fee_rate: Decimal::from_str("0.0005").unwrap(),
        maintenance_margin_rate: Decimal::from_str("0.004").unwrap(),
        funding_interval_hours: 8,
        cost_basis_method: orderbook::CostBasisMethod::FIFO,
        execution_delay_bars: 1,
        allow_future_data: false,
        risk_free_rate: 0.02,
    })
}

fn parse_order_request(payload: Value) -> Result<OrderRequest, String> {
    let order_id = payload.get("order_id").and_then(|v| v.as_str()).map(|s| Uuid::parse_str(s).unwrap_or_else(|_| Uuid::new_v4())).unwrap_or_else(Uuid::new_v4);
    let symbol = payload.get("symbol").and_then(|v| v.as_str()).unwrap_or("BTC-USDT").to_string();
    let side = match payload.get("side").and_then(|v| v.as_str()) {
        Some("Sell") => OrderSide::Sell,
        _ => OrderSide::Buy,
    };
    let direction = match payload.get("direction").and_then(|v| v.as_str()) {
        Some("Short") => Direction::Short,
        _ => Direction::Long,
    };
    let quantity = Decimal::from_str(payload.get("quantity").and_then(|v| v.as_str()).unwrap_or("0")).map_err(|e| e.to_string())?;
    let leverage = Decimal::from_str(payload.get("leverage").and_then(|v| v.as_str()).unwrap_or("1")).unwrap_or_else(|_| Decimal::from(1));

    Ok(OrderRequest {
        order_id,
        symbol,
        side,
        direction,
        order_type: OrderType::Market,
        quantity,
        margin_mode: MarginMode::Cross,
        leverage,
        timestamp: chrono::Utc::now().timestamp_millis(),
        strategy_id: payload.get("strategy_id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
        signal_strength: payload.get("signal_strength").and_then(|v| v.as_f64()).unwrap_or(0.5),
        signal_reason: payload.get("signal_reason").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    })
}

fn generate_synthetic_bars(symbol: &str, count: usize, timeframe: TimeFrame) -> Vec<StandardBar> {
    let step_ms = timeframe.as_seconds() * 1000;
    let mut bars = Vec::with_capacity(count);
    let mut rng = SmallRng::seed_from_u64(42);
    let mut price = Decimal::from(40000);
    for i in 0..count {
        let open = price;
        let delta = Decimal::from(rng.gen_range(-50i64..=50i64));
        let close = open + delta;
        let high_offset = Decimal::from(rng.gen_range(5i64..=25i64));
        let low_offset = Decimal::from(rng.gen_range(5i64..=25i64));
        let high = open.max(close) + high_offset;
        let low = open.min(close) - low_offset;
        let volume = Decimal::from(rng.gen_range(50i64..=500i64));
        bars.push(StandardBar {
            timestamp: 1704067200000 + i as i64 * step_ms,
            open,
            high,
            low,
            close,
            volume,
            symbol: symbol.to_string(),
            exchange: "binance".to_string(),
            confirmed: true,
        });
        price = close;
    }
    bars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_synthetic_bars_no_short_cycle() {
        let bars = generate_synthetic_bars("BTC-USDT", 1000, TimeFrame::M1);
        let closes: Vec<Decimal> = bars.iter().map(|b| b.close).collect();
        let volumes: Vec<Decimal> = bars.iter().map(|b| b.volume).collect();

        // 检测5周期循环
        let has_5_cycle = detect_price_cycle(&closes, 5, 3);
        assert!(!has_5_cycle, "修复后的 generate_synthetic_bars 不应存在5周期价格循环");

        let has_5_volume_cycle = detect_volume_cycle(&volumes, 5, 3);
        assert!(!has_5_volume_cycle, "修复后的 generate_synthetic_bars 不应存在5周期volume循环");
    }

    #[test]
    fn test_generate_synthetic_bars_ohlcv_relationships() {
        let bars = generate_synthetic_bars("BTC-USDT", 1000, TimeFrame::M1);
        // 验证每根K线的OHLC关系
        for (i, bar) in bars.iter().enumerate() {
            assert!(bar.high >= bar.open, "Bar {}: high < open", i);
            assert!(bar.high >= bar.close, "Bar {}: high < close", i);
            assert!(bar.low <= bar.open, "Bar {}: low > open", i);
            assert!(bar.low <= bar.close, "Bar {}: low > close", i);
            assert!(bar.high >= bar.low, "Bar {}: high < low", i);
        }
    }

    #[test]
    fn test_generate_synthetic_bars_random_walk() {
        let bars = generate_synthetic_bars("BTC-USDT", 100, TimeFrame::M1);
        // 验证价格不是单调递增的（随机漫步应该有涨有跌）
        let mut has_up = false;
        let mut has_down = false;
        for window in bars.windows(2) {
            let prev = &window[0];
            let curr = &window[1];
            if curr.close > prev.close {
                has_up = true;
            } else if curr.close < prev.close {
                has_down = true;
            }
        }
        assert!(has_up && has_down, "随机漫步应该同时包含上涨和下跌的K线");
    }

    #[test]
    fn test_generate_synthetic_bars_deterministic() {
        // 使用相同seed应该生成完全相同的序列
        let bars1 = generate_synthetic_bars("BTC-USDT", 100, TimeFrame::M1);
        let bars2 = generate_synthetic_bars("BTC-USDT", 100, TimeFrame::M1);
        assert_eq!(bars1.len(), bars2.len());
        for (a, b) in bars1.iter().zip(bars2.iter()) {
            assert_eq!(a.timestamp, b.timestamp);
            assert_eq!(a.open, b.open);
            assert_eq!(a.high, b.high);
            assert_eq!(a.low, b.low);
            assert_eq!(a.close, b.close);
            assert_eq!(a.volume, b.volume);
        }
    }

    // 辅助函数（从 test_kline_repetition.rs 复制）
    fn detect_price_cycle(values: &[Decimal], cycle_len: usize, min_repetitions: usize) -> bool {
        if values.len() < cycle_len * min_repetitions + 1 {
            return false;
        }
        let diffs: Vec<Decimal> = values.windows(2).map(|w| w[1] - w[0]).collect();
        if diffs.len() < cycle_len * min_repetitions {
            return false;
        }
        for start in 0..=diffs.len().saturating_sub(cycle_len * min_repetitions) {
            let pattern = &diffs[start..start + cycle_len];
            let mut count = 1;
            for rep in 1..min_repetitions {
                let next_start = start + cycle_len * rep;
                let next_end = next_start + cycle_len;
                if &diffs[next_start..next_end] == pattern {
                    count += 1;
                } else {
                    break;
                }
            }
            if count >= min_repetitions {
                return true;
            }
        }
        false
    }

    fn detect_volume_cycle(volumes: &[Decimal], cycle_len: usize, min_repetitions: usize) -> bool {
        if volumes.len() < cycle_len * min_repetitions {
            return false;
        }
        for start in 0..=volumes.len().saturating_sub(cycle_len * min_repetitions) {
            let pattern = &volumes[start..start + cycle_len];
            let mut count = 1;
            for rep in 1..min_repetitions {
                let next_start = start + cycle_len * rep;
                let next_end = next_start + cycle_len;
                if &volumes[next_start..next_end] == pattern {
                    count += 1;
                } else {
                    break;
                }
            }
            if count >= min_repetitions {
                return true;
            }
        }
        false
    }

    #[test]
    fn test_generate_synthetic_bars_respects_timeframe() {
        let bars = generate_synthetic_bars("BTC-USDT", 100, TimeFrame::H1);
        assert_eq!(bars.len(), 100);
        for window in bars.windows(2) {
            let diff = window[1].timestamp - window[0].timestamp;
            assert_eq!(diff, 3600 * 1000, "H1 bars should be spaced by 3600s");
        }
    }

    #[test]
    fn test_start_backtest_with_timeframe() {
        let payload = serde_json::json!({
            "config": {
                "symbol": "ETH-USDT",
                "initial_balance": "5000",
                "margin_mode": "Isolated",
                "default_leverage": "20"
            },
            "strategy_id": "ema_crossover",
            "timeframe": "1h",
            "start_time": 1704067200000i64,
            "end_time": 1706659200000i64
        });

        let config = parse_engine_config(&payload).unwrap();
        assert_eq!(config.symbol, "ETH-USDT");
        assert_eq!(config.initial_balance, Decimal::from(5000));
        assert_eq!(config.margin_mode, MarginMode::Isolated);
        assert_eq!(config.default_leverage, Decimal::from(20));

        let tf = TimeFrame::from_string(payload.get("timeframe").unwrap().as_str().unwrap()).unwrap();
        assert_eq!(tf, TimeFrame::H1);

        let start_time = payload.get("start_time").unwrap().as_i64().unwrap();
        let end_time = payload.get("end_time").unwrap().as_i64().unwrap();
        let step_ms = tf.as_seconds() * 1000;
        let count = ((end_time - start_time) / step_ms) as usize;
        let bars = generate_synthetic_bars(&config.symbol, count, tf);
        assert!(!bars.is_empty());
        assert_eq!(bars[1].timestamp - bars[0].timestamp, step_ms);
    }

    // ------------------------------------------------------------------
    // Indicator API Tests
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_indicators_returns_real_values() {
        let state: AppState = Arc::new(Mutex::new(HashMap::new()));
        let query = IndicatorQuery {
            symbol: "BTC-USDT".to_string(),
            timeframe: "1m".to_string(),
            indicators: "ema_9,rsi_14".to_string(),
            backtest_id: None,
            full: None,
        };

        let result = get_indicators(State(state), Query(query)).await;
        assert!(result.is_ok(), "get_indicators should succeed");

        let json = match result {
            Ok(Json(v)) => v,
            Err(_) => panic!("Expected Ok result"),
        };
        let obj = json.as_object().expect("Response should be a JSON object");

        // Verify both indicators are present
        assert!(obj.contains_key("ema_9"), "Response should contain ema_9");
        assert!(obj.contains_key("rsi_14"), "Response should contain rsi_14");

        // Verify values are not null (real calculated values)
        let ema_value = &obj["ema_9"];
        assert!(!ema_value.is_null(), "ema_9 should have a real calculated value");
        assert!(ema_value.get("value").is_some(), "ema_9 should have a value field");

        let rsi_value = &obj["rsi_14"];
        assert!(!rsi_value.is_null(), "rsi_14 should have a real calculated value");
        assert!(rsi_value.get("value").is_some(), "rsi_14 should have a value field");
    }

    #[tokio::test]
    async fn test_get_indicators_with_backtest_id() {
        let state: AppState = Arc::new(Mutex::new(HashMap::new()));
        let payload = serde_json::json!({
            "config": {
                "symbol": "BTC-USDT",
                "initial_balance": "100000",
                "margin_mode": "Cross",
                "default_leverage": "10"
            },
            "strategy_id": "ema_crossover",
            "timeframe": "1m",
            "start_time": 1704067200000i64,
            "end_time": 1704153600000i64
        });

        let start_result = start_backtest(State(state.clone()), Json(payload)).await;
        assert!(start_result.is_ok(), "start_backtest should succeed");

        let response = match start_result {
            Ok(Json(v)) => v,
            Err(_) => panic!("Expected Ok result"),
        };
        let backtest_id = response.backtest_id;

        // Step the engine to process some bars
        {
            let mut engines = state.lock().await;
            let engine = engines.get_mut(&backtest_id).unwrap();
            for _ in 0..20 {
                engine.step();
            }
        }

        let query = IndicatorQuery {
            symbol: "BTC-USDT".to_string(),
            timeframe: "1m".to_string(),
            indicators: "ema_9,rsi_14".to_string(),
            backtest_id: Some(backtest_id),
            full: None,
        };

        let result = get_indicators(State(state), Query(query)).await;
        assert!(result.is_ok(), "get_indicators with backtest_id should succeed");

        let json = match result {
            Ok(Json(v)) => v,
            Err(_) => panic!("Expected Ok result"),
        };
        let obj = json.as_object().expect("Response should be a JSON object");

        assert!(obj.contains_key("ema_9"), "Response should contain ema_9");
        assert!(obj.contains_key("rsi_14"), "Response should contain rsi_14");

        let ema_value = &obj["ema_9"];
        assert!(!ema_value.is_null(), "ema_9 should have a real calculated value");
        assert!(ema_value.get("value").is_some(), "ema_9 should have a value field");
    }

    #[tokio::test]
    async fn test_get_indicators_with_full_series() {
        let state: AppState = Arc::new(Mutex::new(HashMap::new()));
        let query = IndicatorQuery {
            symbol: "BTC-USDT".to_string(),
            timeframe: "1m".to_string(),
            indicators: "ema_9,rsi_14".to_string(),
            backtest_id: None,
            full: Some(true),
        };

        let result = get_indicators(State(state), Query(query)).await;
        assert!(result.is_ok(), "get_indicators with full=true should succeed");

        let json = match result {
            Ok(Json(v)) => v,
            Err(_) => panic!("Expected Ok result"),
        };
        let obj = json.as_object().expect("Response should be a JSON object");

        assert!(obj.contains_key("ema_9"), "Response should contain ema_9");
        assert!(obj.contains_key("rsi_14"), "Response should contain rsi_14");

        let ema_series = obj["ema_9"].as_array().expect("ema_9 should be an array when full=true");
        assert!(!ema_series.is_empty(), "ema_9 series should not be empty");
        assert!(ema_series[0].get("value").is_some(), "ema_9 series item should have value");
        assert!(ema_series[0].get("timestamp").is_some(), "ema_9 series item should have timestamp");

        let rsi_series = obj["rsi_14"].as_array().expect("rsi_14 should be an array when full=true");
        assert!(!rsi_series.is_empty(), "rsi_14 series should not be empty");
    }

    #[tokio::test]
    async fn test_get_indicators_invalid_backtest_id() {
        let state: AppState = Arc::new(Mutex::new(HashMap::new()));
        let query = IndicatorQuery {
            symbol: "BTC-USDT".to_string(),
            timeframe: "1m".to_string(),
            indicators: "ema_9".to_string(),
            backtest_id: Some("non-existent-id".to_string()),
            full: None,
        };

        let result = get_indicators(State(state), Query(query)).await;
        assert!(result.is_err(), "get_indicators with invalid backtest_id should fail");

        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND, "Should return 404 for invalid backtest_id");
    }

    #[tokio::test]
    async fn test_get_indicators_custom_parameters() {
        let state: AppState = Arc::new(Mutex::new(HashMap::new()));
        let query = IndicatorQuery {
            symbol: "BTC-USDT".to_string(),
            timeframe: "1m".to_string(),
            indicators: "ema_5,rsi_10,macd_8_17_9,bollinger_15_2_5,atr_10".to_string(),
            backtest_id: None,
            full: None,
        };

        let result = get_indicators(State(state), Query(query)).await;
        assert!(result.is_ok(), "get_indicators with custom params should succeed");

        let json = match result {
            Ok(Json(v)) => v,
            Err(_) => panic!("Expected Ok result"),
        };
        let obj = json.as_object().expect("Response should be a JSON object");

        // Verify all custom indicators are present and have real values
        assert!(obj.contains_key("ema_5"), "Response should contain ema_5");
        assert!(!obj["ema_5"].is_null(), "ema_5 should have a real calculated value");

        assert!(obj.contains_key("rsi_10"), "Response should contain rsi_10");
        assert!(!obj["rsi_10"].is_null(), "rsi_10 should have a real calculated value");

        assert!(obj.contains_key("macd_8_17_9"), "Response should contain macd_8_17_9");
        assert!(!obj["macd_8_17_9"].is_null(), "macd_8_17_9 should have a real calculated value");

        assert!(obj.contains_key("bollinger_15_2_5"), "Response should contain bollinger_15_2_5");
        assert!(!obj["bollinger_15_2_5"].is_null(), "bollinger_15_2_5 should have a real calculated value");

        assert!(obj.contains_key("atr_10"), "Response should contain atr_10");
        assert!(!obj["atr_10"].is_null(), "atr_10 should have a real calculated value");
    }

    #[tokio::test]
    async fn test_get_indicators_bollinger_15_2() {
        let state: AppState = Arc::new(Mutex::new(HashMap::new()));
        let query = IndicatorQuery {
            symbol: "BTC-USDT".to_string(),
            timeframe: "1m".to_string(),
            indicators: "bollinger_15_2".to_string(),
            backtest_id: None,
            full: None,
        };

        let result = get_indicators(State(state), Query(query)).await;
        assert!(result.is_ok(), "get_indicators with bollinger_15_2 should succeed");

        let json = match result {
            Ok(Json(v)) => v,
            Err(_) => panic!("Expected Ok result"),
        };
        let obj = json.as_object().expect("Response should be a JSON object");

        // Verify bollinger_15_2 is present and has real values
        assert!(obj.contains_key("bollinger_15_2"), "Response should contain bollinger_15_2");
        assert!(!obj["bollinger_15_2"].is_null(), "bollinger_15_2 should have a real calculated value");
        
        // Verify it has the expected Bollinger Bands fields
        let bb_value = &obj["bollinger_15_2"];
        assert!(bb_value.get("upper").is_some(), "bollinger_15_2 should have upper field");
        assert!(bb_value.get("middle").is_some(), "bollinger_15_2 should have middle field");
        assert!(bb_value.get("lower").is_some(), "bollinger_15_2 should have lower field");
    }

    #[tokio::test]
    async fn test_get_indicators_backtest_progress_limiting() {
        let state: AppState = Arc::new(Mutex::new(HashMap::new()));
        let payload = serde_json::json!({
            "config": {
                "symbol": "BTC-USDT",
                "initial_balance": "100000",
                "margin_mode": "Cross",
                "default_leverage": "10"
            },
            "strategy_id": "ema_crossover",
            "timeframe": "1m",
            "start_time": 1704067200000i64,
            "end_time": 1704153600000i64
        });

        let start_result = start_backtest(State(state.clone()), Json(payload)).await;
        assert!(start_result.is_ok(), "start_backtest should succeed");

        let response = match start_result {
            Ok(Json(v)) => v,
            Err(_) => panic!("Expected Ok result"),
        };
        let backtest_id = response.backtest_id;
        let total_bars = response.total_bars;

        // Before stepping, no bars processed - should return empty
        let query_before = IndicatorQuery {
            symbol: "BTC-USDT".to_string(),
            timeframe: "1m".to_string(),
            indicators: "ema_9".to_string(),
            backtest_id: Some(backtest_id.clone()),
            full: Some(true),
        };

        let result_before = get_indicators(State(state.clone()), Query(query_before)).await;
        assert!(result_before.is_ok(), "get_indicators should succeed even with no bars processed");

        let json_before = match result_before {
            Ok(Json(v)) => v,
            Err(_) => panic!("Expected Ok result"),
        };
        let obj_before = json_before.as_object().expect("Response should be a JSON object");
        
        // Should return empty object since no bars processed yet
        assert!(obj_before.is_empty() || obj_before["ema_9"].is_null() || obj_before["ema_9"].as_array().map_or(true, |a| a.is_empty()),
            "Before stepping, indicator should be empty or null");

        // Step the engine a few times
        {
            let mut engines = state.lock().await;
            let engine = engines.get_mut(&backtest_id).unwrap();
            for _ in 0..20 {
                engine.step();
            }
        }

        // After stepping, should have processed bars
        let query_after = IndicatorQuery {
            symbol: "BTC-USDT".to_string(),
            timeframe: "1m".to_string(),
            indicators: "ema_9".to_string(),
            backtest_id: Some(backtest_id.clone()),
            full: Some(true),
        };

        let result_after = get_indicators(State(state.clone()), Query(query_after)).await;
        assert!(result_after.is_ok(), "get_indicators should succeed after stepping");

        let json_after = match result_after {
            Ok(Json(v)) => v,
            Err(_) => panic!("Expected Ok result"),
        };
        let obj_after = json_after.as_object().expect("Response should be a JSON object");

        assert!(obj_after.contains_key("ema_9"), "Response should contain ema_9 after stepping");
        let ema_series = obj_after["ema_9"].as_array().expect("ema_9 should be an array when full=true");
        
        // Should have at most 20 data points (only processed bars)
        assert!(ema_series.len() <= 20, 
            "Indicator series should be limited to processed bars (<=20), got {}", ema_series.len());
        
        // Should NOT have all bars
        assert!(ema_series.len() < total_bars,
            "Indicator series should not include all {} bars, only processed ones", total_bars);
    }

    #[test]
    fn test_parse_indicator_name() {
        // EMA
        let ema = parse_indicator_name("ema_5").unwrap();
        assert!(matches!(ema, IndicatorType::Ema(5)));
        
        let ema2 = parse_indicator_name("ema_21").unwrap();
        assert!(matches!(ema2, IndicatorType::Ema(21)));

        // RSI
        let rsi = parse_indicator_name("rsi_14").unwrap();
        assert!(matches!(rsi, IndicatorType::Rsi(14)));
        
        let rsi2 = parse_indicator_name("rsi_10").unwrap();
        assert!(matches!(rsi2, IndicatorType::Rsi(10)));

        // MACD
        let macd = parse_indicator_name("macd_12_26_9").unwrap();
        assert!(matches!(macd, IndicatorType::Macd(12, 26, 9)));
        
        let macd2 = parse_indicator_name("macd_8_17_9").unwrap();
        assert!(matches!(macd2, IndicatorType::Macd(8, 17, 9)));

        // Bollinger
        let bb = parse_indicator_name("bollinger_20_2").unwrap();
        assert!(matches!(bb, IndicatorType::Bollinger(20, d) if d == Decimal::from(2)));
        
        let bb2 = parse_indicator_name("bollinger_15_2_5").unwrap();
        let expected_2_5 = Decimal::from_str("2.5").unwrap();
        assert!(matches!(bb2, IndicatorType::Bollinger(15, d) if d == expected_2_5));

        // Test bollinger_15_2 specifically (integer stdDev)
        let bb3 = parse_indicator_name("bollinger_15_2").unwrap();
        assert!(matches!(bb3, IndicatorType::Bollinger(15, d) if d == Decimal::from(2)));

        // ATR
        let atr = parse_indicator_name("atr_14").unwrap();
        assert!(matches!(atr, IndicatorType::Atr(14)));
        
        let atr2 = parse_indicator_name("atr_10").unwrap();
        assert!(matches!(atr2, IndicatorType::Atr(10)));

        // VWAP
        let vwap = parse_indicator_name("vwap").unwrap();
        assert!(matches!(vwap, IndicatorType::Vwap));

        // Invalid
        assert!(parse_indicator_name("unknown").is_err());
        assert!(parse_indicator_name("ema").is_err());
        assert!(parse_indicator_name("rsi").is_err());
    }

    #[test]
    fn test_bollinger_15_2_calculation() {
        let bars = generate_synthetic_bars("BTC-USDT", 200, TimeFrame::M1);
        let closes: Vec<Decimal> = bars.iter().map(|b| b.close).collect();
        let highs: Vec<Decimal> = bars.iter().map(|b| b.high).collect();
        let lows: Vec<Decimal> = bars.iter().map(|b| b.low).collect();
        let volumes: Vec<Decimal> = bars.iter().map(|b| b.volume).collect();
        
        // Test series calculation
        let result = calculate_indicator_series("bollinger_15_2", &closes, &highs, &lows, &volumes, &bars);
        assert!(result.is_ok(), "bollinger_15_2 series calculation failed: {:?}", result.err());
        
        // Test last value calculation
        let result = calculate_indicator_last("bollinger_15_2", &closes, &highs, &lows, &volumes);
        assert!(result.is_ok(), "bollinger_15_2 last value calculation failed: {:?}", result.err());
    }

    // ------------------------------------------------------------------
    // Strategy API Tests
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_strategies() {
        let result = list_strategies().await;
        assert!(result.is_ok(), "list_strategies should succeed");

        let json = match result {
            Ok(Json(v)) => v,
            Err(_) => panic!("Expected Ok result"),
        };
        let strategies = json.as_array().expect("Response should be a JSON array");

        // Should return all 5 strategies
        assert_eq!(strategies.len(), 5, "Should return exactly 5 strategies");

        let ids: Vec<&str> = strategies
            .iter()
            .map(|s| s.get("id").unwrap().as_str().unwrap())
            .collect();

        assert!(ids.contains(&"always_long"), "Should include always_long");
        assert!(ids.contains(&"ema_crossover"), "Should include ema_crossover");
        assert!(ids.contains(&"rsi_macd"), "Should include rsi_macd");
        assert!(ids.contains(&"bollinger_bands"), "Should include bollinger_bands");
        assert!(ids.contains(&"breakout"), "Should include breakout");

        // Verify each strategy has required fields
        for strategy in strategies {
            assert!(strategy.get("id").is_some(), "Strategy should have id");
            assert!(strategy.get("name").is_some(), "Strategy should have name");
            assert!(strategy.get("description").is_some(), "Strategy should have description");
            assert!(strategy.get("default_params").is_some(), "Strategy should have default_params");
        }
    }

    #[tokio::test]
    async fn test_get_strategy_defaults_ema_crossover() {
        let result = get_strategy_defaults(Path("ema_crossover".to_string())).await;
        assert!(result.is_ok(), "get_strategy_defaults should succeed for ema_crossover");

        let json = match result {
            Ok(Json(v)) => v,
            Err(_) => panic!("Expected Ok result"),
        };
        assert_eq!(json.get("id").unwrap().as_str().unwrap(), "ema_crossover");
        assert_eq!(json.get("name").unwrap().as_str().unwrap(), "EMA Crossover");

        let param_defs = json.get("param_definitions").unwrap().as_array().unwrap();
        assert_eq!(param_defs.len(), 2, "EMA Crossover should have 2 param definitions");

        // Verify fast_period definition
        let fast_period = &param_defs[0];
        assert_eq!(fast_period.get("name").unwrap().as_str().unwrap(), "fast_period");
        let fast_type = fast_period.get("param_type").unwrap();
        assert_eq!(fast_type.get("Integer").unwrap().get("min").unwrap().as_i64().unwrap(), 2);
        assert_eq!(fast_type.get("Integer").unwrap().get("max").unwrap().as_i64().unwrap(), 100);
        assert_eq!(fast_type.get("Integer").unwrap().get("default").unwrap().as_i64().unwrap(), 9);

        // Verify slow_period definition
        let slow_period = &param_defs[1];
        assert_eq!(slow_period.get("name").unwrap().as_str().unwrap(), "slow_period");
        let slow_type = slow_period.get("param_type").unwrap();
        assert_eq!(slow_type.get("Integer").unwrap().get("min").unwrap().as_i64().unwrap(), 2);
        assert_eq!(slow_type.get("Integer").unwrap().get("max").unwrap().as_i64().unwrap(), 200);
        assert_eq!(slow_type.get("Integer").unwrap().get("default").unwrap().as_i64().unwrap(), 21);
    }

    // ------------------------------------------------------------------
    // Backtest API Tests
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_start_backtest_with_custom_params() {
        let state: AppState = Arc::new(Mutex::new(HashMap::new()));
        let payload = serde_json::json!({
            "config": {
                "symbol": "BTC-USDT",
                "initial_balance": "100000",
                "margin_mode": "Cross",
                "default_leverage": "10"
            },
            "strategy_id": "ema_crossover",
            "strategy_params": {
                "fast_period": 5,
                "slow_period": 15
            },
            "timeframe": "1m",
            "start_time": 1704067200000i64,
            "end_time": 1704153600000i64
        });

        let result = start_backtest(State(state.clone()), Json(payload)).await;
        assert!(result.is_ok(), "start_backtest with custom params should succeed");

        let response = match result {
            Ok(Json(v)) => v,
            Err(_) => panic!("Expected Ok result"),
        };
        assert_eq!(response.status, "running");
        assert!(!response.backtest_id.is_empty(), "backtest_id should not be empty");
        assert!(response.total_bars > 0, "total_bars should be positive");

        // Verify engine was stored in state
        let engines = state.lock().await;
        assert!(engines.contains_key(&response.backtest_id), "Engine should be stored in state");
    }

    #[tokio::test]
    async fn test_start_backtest_with_invalid_strategy() {
        let state: AppState = Arc::new(Mutex::new(HashMap::new()));
        let payload = serde_json::json!({
            "config": {
                "symbol": "BTC-USDT",
                "initial_balance": "100000",
                "margin_mode": "Cross",
                "default_leverage": "10"
            },
            "strategy_id": "nonexistent_strategy",
            "timeframe": "1m",
            "start_time": 1704067200000i64,
            "end_time": 1704153600000i64
        });

        let result = start_backtest(State(state.clone()), Json(payload)).await;
        // Even with invalid strategy, start_backtest currently creates an engine without strategy
        // and returns success. Verify that engine is created but with None strategy.
        assert!(result.is_ok(), "start_backtest should still succeed (engine created without strategy)");

        let response = match result {
            Ok(Json(v)) => v,
            Err(_) => panic!("Expected Ok result"),
        };
        assert_eq!(response.status, "running");

        // Verify engine was stored
        let engines = state.lock().await;
        assert!(engines.contains_key(&response.backtest_id));
    }
}
