# CBT-Pro Agent Responsibilities & Interface Summary

## Agent Roles

| Agent | Module | Must NOT Touch |
|-------|--------|---------------|
| Data_Pipeline | `data_pipeline/` | `engine/` internals, `orderbook/` logic |
| Backtest_Engine | `engine/`, `orderbook/`, `indicators/` | `data_pipeline/` storage internals |
| Strategy | `api/` | `engine/` internals, `orderbook/` direct access |
| Frontend | `frontend/` | Any backend logic |
| DevOps | `.github/`, `docs/`, `docker-compose.yml` | Application code |

## Interface Summary

### Data Pipeline -> Engine

```rust
pub trait BarStream: Stream<Item = Result<StandardBar, DataError>> {}

pub struct AggregationEngine;
impl AggregationEngine {
    pub async fn get_bars(&self, symbol: &str, timeframe: TimeFrame, start: i64, end: i64) -> Result<Vec<StandardBar>, DataError>;
    pub async fn get_latest_bar(&self, symbol: &str, timeframe: TimeFrame) -> Result<Option<StandardBar>, DataError>;
}
```

### Engine -> OrderBook

```rust
pub trait OrderBookManager {
    fn open_position(&mut self, req: &OrderRequest, fill: &OrderFill) -> Result<Position, OrderBookError>;
    fn add_to_position(&mut self, pos_id: PositionId, fill: &OrderFill) -> Result<Position, OrderBookError>;
    fn reduce_position(&mut self, pos_id: PositionId, fill: &OrderFill, method: CostBasisMethod) -> Result<(Position, Decimal), OrderBookError>;
    fn close_position(&mut self, pos_id: PositionId, fill: &OrderFill, method: CostBasisMethod) -> Result<(Position, Decimal), OrderBookError>;
    fn get_position(&self, pos_id: PositionId) -> Option<&Position>;
    fn get_positions_by_symbol(&self, symbol: &str) -> Vec<&Position>;
    fn get_all_positions(&self) -> Vec<&Position>;
    fn update_unrealized_pnl(&mut self, symbol: &str, mark_price: Decimal);
    fn check_liquidation(&self, pos_id: PositionId, mark_price: Decimal) -> bool;
}
```

### API -> Frontend (WebSocket)

```typescript
type WsMessage =
    | { type: 'snapshot'; data: EngineSnapshot }
    | { type: 'bar_update'; bar: StandardBar }
    | { type: 'trade'; fill: OrderFill }
    | { type: 'signal'; signal: Signal }
    | { type: 'complete'; result: BacktestResult }
    | { type: 'error'; message: string };
```

## Communication Rules

1. **No Direct Access**: Agents must NOT directly access internal data structures of other agents' modules.
2. **Interface Only**: All communication must go through the defined traits/ABC contracts.
3. **Deterministic**: Engine output must be 100% reproducible given the same inputs.
4. **Error Propagation**: Errors must use `thiserror` (Rust) or exceptions (Python), never silently fail.

## Testing Responsibilities

| Agent | Tests |
|-------|-------|
| Data_Pipeline | Aggregation M1->M5, Parquet roundtrip, PostgreSQL queries |
| Backtest_Engine | Position math, FIFO/LIFO, margin calc, liquidation trigger, data leakage detection |
| Strategy | EMA Cross known signals |
| Frontend | WASM data conversion, chart 10k bars at 60FPS, playback controls |
