pub mod position;
pub mod order;
pub mod margin;

pub use position::{InMemoryOrderBook, OrderBookManager};
pub use order::OrderSimulator;
pub use margin::MarginCalculator;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
pub use data_pipeline::StandardBar;
use uuid::Uuid;

pub type PositionId = Uuid;
pub type OrderId = Uuid;

/// Trading direction: Long (betting on price increase) or Short (betting on price decrease).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Long,
    Short,
}

/// Current status of a position in the book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionStatus {
    Open,
    PartiallyClosed,
    Closed,
}

/// Side of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Type of order and any associated price parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit(Decimal),
    StopMarket(Decimal),
}

/// Margin mode for a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarginMode {
    Isolated,
    Cross,
}

/// Cost basis method for realized PnL calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CostBasisMethod {
    FIFO,
    LIFO,
    WeightedAverage,
}

/// A single entry leg into a position, tracking the price, quantity, and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionLeg {
    pub entry_price: Decimal,
    pub quantity: Decimal,
    pub timestamp: i64,
    pub order_id: OrderId,
}

/// A trading position with full lifecycle tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: PositionId,
    pub symbol: String,
    pub direction: Direction,
    pub status: PositionStatus,
    pub entries: Vec<PositionLeg>,
    pub current_size: Decimal,
    pub average_entry_price: Decimal,
    pub realized_pnl: Decimal,
    pub unrealized_pnl: Decimal,
    pub opened_at: i64,
    pub updated_at: i64,
}

/// The full position book state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionBook {
    pub positions: HashMap<PositionId, Position>,
    pub closed_positions: Vec<Position>,
    pub margin_used: Decimal,
    pub unrealized_pnl: Decimal,
}


/// Request to create a new order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    pub order_id: OrderId,
    pub symbol: String,
    pub side: OrderSide,
    pub direction: Direction,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub margin_mode: MarginMode,
    pub leverage: Decimal,
    pub timestamp: i64,
    pub strategy_id: String,
    pub signal_strength: f64,
    pub signal_reason: String,
}

/// Record of a filled order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFill {
    pub order_id: OrderId,
    pub position_id: Option<PositionId>,
    pub symbol: String,
    pub side: OrderSide,
    pub direction: Direction,
    pub filled_price: Decimal,
    pub filled_quantity: Decimal,
    pub fee: Decimal,
    pub fee_asset: String,
    pub timestamp: i64,
    pub realized_pnl: Option<Decimal>,
}

/// Errors that can occur during order book operations.
#[derive(Debug, thiserror::Error)]
pub enum OrderBookError {
    #[error("Position not found: {0}")]
    PositionNotFound(PositionId),
    #[error("Insufficient position size: have {have}, need {need}")]
    InsufficientSize { have: Decimal, need: Decimal },
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    #[error("Liquidation triggered for position {0}")]
    LiquidationTriggered(PositionId),
    #[error("Invalid order: {0}")]
    InvalidOrder(String),
    #[error("Margin call: required {required}, available {available}")]
    MarginCall { required: Decimal, available: Decimal },
}
