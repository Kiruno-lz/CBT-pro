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
    let strategy: Option<Box<dyn engine::Strategy>> = match strategy_id {
        "always_long" => Some(Box::new(engine::AlwaysLong::new(
            config.symbol.clone(),
            Decimal::from_str("0.1").unwrap(),
        ))),
        "ema_crossover" => Some(Box::new(engine::EmaCrossover {
            symbol: config.symbol.clone(),
            quantity: Decimal::from_str("0.1").unwrap(),
            fast_period: 9,
            slow_period: 21,
        })),
        "rsi_macd" => Some(Box::new(engine::RsiMacd {
            symbol: config.symbol.clone(),
            quantity: Decimal::from_str("0.1").unwrap(),
        })),
        "bollinger_bands" => Some(Box::new(engine::StrategyBollingerBands {
            symbol: config.symbol.clone(),
            quantity: Decimal::from_str("0.1").unwrap(),
        })),
        "breakout" => Some(Box::new(engine::Breakout {
            symbol: config.symbol.clone(),
            quantity: Decimal::from_str("0.1").unwrap(),
        })),
        _ => None,
    };

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
    State(state): State<AppState>,
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
}

async fn get_indicators(
    Query(params): Query<IndicatorQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let indicator_names: Vec<&str> = params.indicators.split(',').collect();
    let mut result = serde_json::Map::new();

    // For MVP, return placeholder values
    for name in indicator_names {
        let val = match name {
            "ema_9" => Value::String("42350.50".to_string()),
            "ema_21" => Value::String("42100.00".to_string()),
            "rsi_14" => Value::Number(serde_json::Number::from_f64(62.5).unwrap()),
            "atr_14" => Value::String("850.00".to_string()),
            _ => Value::Null,
        };
        result.insert(name.to_string(), val);
    }

    Ok(Json(Value::Object(result)))
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
}
