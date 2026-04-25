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

    // For MVP, generate synthetic bars if no data source is configured
    let bars = generate_synthetic_bars(&config.symbol, 1000);
    let total_bars = bars.len();

    let engine = BacktestEngine::new(config, bars);
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
    // Simplified config parsing for MVP
    let symbol = payload.get("config").and_then(|c| c.get("symbol")).and_then(|s| s.as_str()).unwrap_or("BTC-USDT").to_string();
    let initial_balance = Decimal::from_str(payload.get("config").and_then(|c| c.get("initial_balance")).and_then(|v| v.as_str()).unwrap_or("100000")).unwrap_or_else(|_| Decimal::from(100000));

    Ok(EngineConfig {
        symbol,
        initial_balance,
        margin_mode: MarginMode::Cross,
        default_leverage: Decimal::from(10),
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

fn generate_synthetic_bars(symbol: &str, count: usize) -> Vec<StandardBar> {
    let mut bars = Vec::with_capacity(count);
    let base = Decimal::from(40000);
    for i in 0..count {
        let open = base + Decimal::from(i as i64 * 10);
        let close = open + Decimal::from((i % 5) as i64 * 5 - 10);
        let high = if close > open { close + Decimal::from(20) } else { open + Decimal::from(20) };
        let low = if close < open { close - Decimal::from(20) } else { open - Decimal::from(20) };
        bars.push(StandardBar {
            timestamp: 1704067200000 + i as i64 * 60000,
            open,
            high,
            low,
            close,
            volume: Decimal::from(10 + (i % 5) as i64),
            symbol: symbol.to_string(),
            exchange: "binance".to_string(),
            confirmed: true,
        });
    }
    bars
}
