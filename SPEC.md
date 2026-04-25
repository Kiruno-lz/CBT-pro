# CBT-Pro Specification Document (SPEC.md)

## 1. Project Overview

CBT-Pro (Crypto Backtester Professional) is an institutional-grade cryptocurrency quantitative backtesting system with a hybrid architecture:
- **Rust Core**: High-performance backtest engine, order book management, data pipeline, API gateway
- **Python Layer**: Strategy development, technical indicators, data analysis
- **TypeScript Frontend**: Visualization, playback controls, real-time signal tracking

---

## 2. Module Architecture

### 2.1 Rust Workspace (`rust_core/`)
```
workspace/
├── crates/
│   ├── data_pipeline/    # Data ingestion, storage, resampling
│   ├── indicators/         # Technical indicator calculations (Rust-native)
│   ├── orderbook/          # Position book management
│   ├── engine/             # Backtest engine core
│   └── api/                # Axum HTTP + WebSocket gateway
```

### 2.2 Python Package (`python_strategies/`)
```
cbt_pro/
├── interfaces/             # Abstract base classes
├── strategies/             # Strategy implementations
├── indicators/             # Python indicator wrappers
├── backtest_client/        # HTTP SDK for Rust engine
└── utils/                  # Utilities
```

### 2.3 Frontend (`frontend/`)
```
src/
├── components/             # React components
├── charting/               # lightweight-charts wrappers
├── stores/                 # Zustand state management
└── pages/                  # Route pages
```

---

## 3. Core Data Structures

### 3.1 StandardBar (Cross-Module)
```rust
pub struct StandardBar {
    pub timestamp: i64,           // Unix timestamp (ms)
    pub open: Decimal,            // rust_decimal::Decimal
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub symbol: String,           // "BTC-USDT"
    pub exchange: String,         // "binance", "okx"
    pub confirmed: bool,          // K-line closed
}
```

### 3.2 TimeFrame Enum
```rust
pub enum TimeFrame {
    M1, M5, M15, M30, H1, H4, D1, W1,
}
```

### 3.3 Position Structures
```rust
pub type PositionId = Uuid;
pub type OrderId = Uuid;

pub enum Direction {
    Long,
    Short,
}

pub enum PositionStatus {
    Open,
    PartiallyClosed,
    Closed,
}

pub struct PositionLeg {
    pub entry_price: Decimal,
    pub quantity: Decimal,
    pub timestamp: i64,
    pub order_id: OrderId,
}

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

pub struct PositionBook {
    pub positions: HashMap<PositionId, Position>,
    pub closed_positions: Vec<Position>,
    pub margin_used: Decimal,
    pub unrealized_pnl: Decimal,
}
```

### 3.4 Order Structures
```rust
pub enum OrderType {
    Market,
    Limit(Decimal),
    StopMarket(Decimal),
}

pub enum OrderSide {
    Buy,
    Sell,
}

pub enum MarginMode {
    Isolated,
    Cross,
}

pub enum CostBasisMethod {
    FIFO,
    LIFO,
    WeightedAverage,
}

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
```

### 3.5 Engine Snapshot (WebSocket + Frontend)
```rust
pub struct EngineSnapshot {
    pub timestamp: i64,
    pub current_bar: StandardBar,
    pub equity: Decimal,
    pub available_balance: Decimal,
    pub margin_used: Decimal,
    pub margin_ratio: Decimal,
    pub unrealized_pnl: Decimal,
    pub realized_pnl_today: Decimal,
    pub positions: Vec<Position>,
    pub orders_history: Vec<OrderFill>,
    pub daily_pnl: Vec<(i64, Decimal)>,
    pub max_drawdown: Decimal,
    pub max_drawdown_pct: Decimal,
    pub sharpe_ratio: Option<f64>,
    pub total_trades: u64,
    pub winning_trades: u64,
    pub losing_trades: u64,
    pub win_rate: f64,
}
```

### 3.6 Signal (Python ↔ Rust)
```python
class SignalAction(Enum):
    OPEN_LONG = "open_long"
    OPEN_SHORT = "open_short"
    ADD_LONG = "add_long"
    ADD_SHORT = "add_short"
    REDUCE_LONG = "reduce_long"
    REDUCE_SHORT = "reduce_short"
    CLOSE_LONG = "close_long"
    CLOSE_SHORT = "close_short"
    CLOSE_ALL = "close_all"

@dataclass
class Signal:
    action: SignalAction
    symbol: str
    quantity: Decimal
    strength: float          # 0.0 - 1.0
    reason: str
    metadata: Dict[str, Any]
    take_profit: Optional[Decimal] = None
    stop_loss: Optional[Decimal] = None
```

### 3.7 StrategyContext (Read-Only to Strategies)
```python
@dataclass
class PositionSnapshot:
    id: str
    symbol: str
    direction: str
    current_size: Decimal
    average_entry_price: Decimal
    unrealized_pnl: Decimal
    realized_pnl: Decimal
    leverage: Decimal
    margin_used: Decimal
    opened_at: int

@dataclass
class StrategyContext:
    current_price: Decimal
    open_orders: int
    positions: List[PositionSnapshot]
    equity: Decimal
    available_balance: Decimal
    unrealized_pnl: Decimal
    margin_ratio: Decimal
    bar_history: List[StandardBar]
    current_bar_index: int
    total_bars: int
    timestamp: int
```

---

## 4. Interface Contracts

### 4.1 Data Pipeline → Engine
```rust
// Async stream of bars — backpressure handled
pub trait BarStream: Stream<Item = Result<StandardBar, DataError>> {}

// Aggregation engine
pub struct AggregationEngine;
impl AggregationEngine {
    pub async fn get_bars(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        start: i64,
        end: i64,
    ) -> Result<Vec<StandardBar>, DataError>;

    pub async fn get_latest_bar(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
    ) -> Result<Option<StandardBar>, DataError>;
}
```

### 4.2 Engine Internal Interfaces
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

pub trait BacktestEngine {
    fn new(config: EngineConfig, data: Vec<StandardBar>) -> Self;
    fn run(&mut self) -> Result<BacktestResult, EngineError>;
    fn step(&mut self) -> Option<EngineSnapshot>;  // Returns None when done
    fn reset(&mut self);
    fn get_state(&self) -> EngineSnapshot;
    fn set_strategy_callback(&mut self, callback: Box<dyn Fn(&StrategyContext) -> Option<Signal>>);
}
```

### 4.3 Engine Config
```rust
pub struct EngineConfig {
    pub symbol: String,
    pub initial_balance: Decimal,
    pub margin_mode: MarginMode,
    pub default_leverage: Decimal,
    pub maker_fee_rate: Decimal,
    pub taker_fee_rate: Decimal,
    pub maintenance_margin_rate: Decimal,
    pub funding_interval_hours: u32,
    pub cost_basis_method: CostBasisMethod,
    pub execution_delay_bars: u32,      // Default: 1 (next bar execution)
    pub allow_future_data: bool,        // FALSE in production, TRUE only for testing
    pub risk_free_rate: f64,            // For Sharpe ratio, e.g., 0.02
}
```

---

## 5. REST API Specification (OpenAPI 3.0 Skeleton)

### 5.1 Base URL
- REST: `http://localhost:8080/api/v1`
- WebSocket: `ws://localhost:8081/ws`

### 5.2 Endpoints

#### POST /backtest/start
**Request**:
```json
{
  "config": { "symbol": "BTC-USDT", "initial_balance": "100000", ... },
  "strategy_id": "ema_cross_v1",
  "timeframe": "1h",
  "start_time": 1704067200000,
  "end_time": 1706745600000
}
```
**Response**:
```json
{
  "backtest_id": "bt_7f3a...",
  "status": "running",
  "total_bars": 4320
}
```

#### POST /backtest/{id}/pause
**Response**: `{ "status": "paused" }`

#### POST /backtest/{id}/resume
**Response**: `{ "status": "running" }`

#### GET /backtest/{id}/state
**Response**: `EngineSnapshot` JSON

#### GET /backtest/{id}/result
**Response** (when complete):
```json
{
  "backtest_id": "bt_7f3a...",
  "final_equity": "145230.50",
  "total_return_pct": 45.23,
  "max_drawdown_pct": 12.5,
  "sharpe_ratio": 1.85,
  "total_trades": 156,
  "win_rate": 58.3,
  "profit_factor": 2.1,
  "avg_trade_return": 1.2,
  "daily_pnls": [...],
  "trades": [...]
}
```

#### POST /order
**Request**: `OrderRequest` JSON
**Response**: `{ "order_id": "...", "status": "filled", "fill": OrderFill }`

#### GET /indicators
**Query**: `?symbol=BTC-USDT&timeframe=1h&indicators=ema_9,ema_21,rsi_14`
**Response**:
```json
{
  "ema_9": "42350.50",
  "ema_21": "42100.00",
  "rsi_14": 62.5
}
```

### 5.3 WebSocket Events

**Client → Server**:
```json
{ "type": "subscribe", "channel": "backtest_state", "backtest_id": "bt_7f3a..." }
{ "type": "control", "action": "play|pause|step_forward|step_backward", "backtest_id": "..." }
{ "type": "control", "action": "set_speed", "speed": 5.0 }
```

**Server → Client**:
```json
{ "type": "snapshot", "data": { ...EngineSnapshot... } }
{ "type": "bar_update", "bar": { ...StandardBar... } }
{ "type": "trade", "fill": { ...OrderFill... } }
{ "type": "signal", "signal": { ...Signal... } }
{ "type": "complete", "result": { ...BacktestResult... } }
```

---

## 6. Protocol Buffer Schema (proto/cbt_pro.proto)

```protobuf
syntax = "proto3";
package cbt_pro;

message StandardBar {
    int64 timestamp = 1;
    string open = 2;
    string high = 3;
    string low = 4;
    string close = 5;
    string volume = 6;
    string symbol = 7;
    string exchange = 8;
    bool confirmed = 9;
}

message Position {
    string id = 1;
    string symbol = 2;
    string direction = 3;
    string status = 4;
    repeated PositionLeg entries = 5;
    string current_size = 6;
    string average_entry_price = 7;
    string realized_pnl = 8;
    string unrealized_pnl = 9;
    int64 opened_at = 10;
    int64 updated_at = 11;
}

message PositionLeg {
    string entry_price = 1;
    string quantity = 2;
    int64 timestamp = 3;
    string order_id = 4;
}

message OrderRequest {
    string order_id = 1;
    string symbol = 2;
    string side = 3;
    string direction = 4;
    string order_type = 5;
    string limit_price = 6;
    string stop_price = 7;
    string quantity = 8;
    string margin_mode = 9;
    string leverage = 10;
    int64 timestamp = 11;
    string strategy_id = 12;
    double signal_strength = 13;
    string signal_reason = 14;
}

message EngineSnapshot {
    int64 timestamp = 1;
    StandardBar current_bar = 2;
    string equity = 3;
    string available_balance = 4;
    string margin_used = 5;
    string margin_ratio = 6;
    string unrealized_pnl = 7;
    string realized_pnl_today = 8;
    repeated Position positions = 9;
    int64 total_trades = 10;
    double win_rate = 11;
    double max_drawdown_pct = 12;
    double sharpe_ratio = 13;
}
```

---

## 7. Strategy Interface (Python)

```python
from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import List, Optional, Dict, Any
from decimal import Decimal

class BaseStrategy(ABC):
    @property
    @abstractmethod
    def name(self) -> str: ...

    @property
    @abstractmethod
    def version(self) -> str: ...

    @property
    @abstractmethod
    def required_indicators(self) -> List[str]: ...

    @abstractmethod
    def on_bar(self, bar: StandardBar, context: StrategyContext) -> Optional[Signal]:
        """Called on every bar close. Must NOT perform I/O."""
        ...

    @abstractmethod
    def on_position_update(self, position: PositionSnapshot, context: StrategyContext) -> Optional[Signal]:
        """Called when position state changes."""
        ...

    def get_parameters(self) -> Dict[str, Any]:
        """Return configurable parameters with defaults."""
        return {}
```

---

## 8. Frontend State Management

### 8.1 Zustand Store Structure
```typescript
interface AppState {
    // Connection
    wsConnected: boolean;
    engineOnline: boolean;

    // Playback
    playback: {
        status: 'idle' | 'playing' | 'paused' | 'stepping' | 'complete';
        currentBarIndex: number;
        totalBars: number;
        speed: number;
        currentTime: number;
    };

    // Data
    bars: StandardBar[];
    visibleRange: { from: number; to: number };

    // Engine State
    snapshot: EngineSnapshot | null;

    // Trading
    signals: Signal[];
    activeSignals: Signal[];
    tradeHistory: OrderFill[];

    // Chart
    chartTimeframe: TimeFrame;
    indicators: IndicatorConfig[];
    markerVisibility: boolean;
}
```

### 8.2 WebSocket Message Types
```typescript
type WsMessage =
    | { type: 'snapshot'; data: EngineSnapshot }
    | { type: 'bar_update'; bar: StandardBar }
    | { type: 'trade'; fill: OrderFill }
    | { type: 'signal'; signal: Signal }
    | { type: 'complete'; result: BacktestResult }
    | { type: 'error'; message: string };
```

---

## 9. Testing Requirements

### 9.1 Unit Tests (Rust)
- `orderbook`: Position open/add/reduce/close, FIFO/LIFO math, margin calc
- `engine`: Bar-by-bar execution, no data leakage, liquidation trigger
- `data_pipeline`: Aggregation M1→M5, Parquet roundtrip, PostgreSQL queries
- `indicators`: EMA, RSI, Bollinger math verified against known values

### 9.2 Integration Tests (Python + Rust)
- Full backtest pipeline: Data → Engine → Strategy → Result
- EMA Cross strategy on 2017 BTC data with known expected signals

### 9.3 Data Leakage Tests
- Strategy accessing `bars[current_idx+1]` must panic/error
- Signal generated on bar N must execute on bar N+1 (configurable)
- Lookahead check: verify strategy only sees `bars[..current_idx]`

### 9.4 Frontend Tests
- WASM module loads and converts data correctly
- Chart renders 10k bars at 60FPS
- Playback controls update state correctly

---

## 10. Build Requirements

### Rust
- Edition: 2021
- Key crates: tokio, axum, serde, rust_decimal, sqlx, parquet, arrow, uuid, chrono, thiserror, tracing
- Each crate must compile independently: `cargo check -p <crate>`

### Python
- Version: 3.11+
- Dependencies: pydantic, aiohttp, pandas, numpy, (ta-lib optional)
- Type checking: mypy strict mode

### Frontend
- Node: 20+
- React: 19+, TypeScript: 5.5+, Vite: 6+
- Key packages: lightweight-charts, zustand, msgpack-lite, react-use-websocket
- Build target: ES2022

---

## 11. Docker Services

```yaml
services:
  postgres:
    image: postgres:15-alpine
    environment:
      POSTGRES_USER: cbtpro
      POSTGRES_PASSWORD: cbtpro
      POSTGRES_DB: cbtpro
    volumes:
      - pg_data:/var/lib/postgresql/data
      - ./database/migrations:/docker-entrypoint-initdb.d
    ports: ["5432:5432"]

  rust_engine:
    build: ./rust_core
    depends_on: [postgres]
    environment:
      DATABASE_URL: postgres://cbtpro:cbtpro@postgres/cbtpro
      RUST_LOG: info
    ports: ["8080:8080", "8081:8081"]

  frontend:
    build: ./frontend
    depends_on: [rust_engine]
    ports: ["3000:3000"]
```

---

## 12. CI/CD Requirements

### GitHub Actions Workflow (`.github/workflows/ci.yml`)
1. **Rust**: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --workspace`
2. **Python**: `ruff check .`, `mypy cbt_pro/`, `pytest`
3. **Frontend**: `tsc --noEmit`, `eslint .`, `vitest run`
4. **Coverage**: Rust core >80%, Python >60%
5. **Triggers**: PR to `main` or `develop`

---

## 13. Anti-Data-Leakage Rules (CRITICAL)

1. **Lookahead Barrier**: The engine MUST maintain `current_idx` and NEVER expose `bars[current_idx..]` to the strategy during `on_bar()`.
2. **Execution Delay**: All signals generated at bar `N` execute at bar `N + execution_delay_bars` (default 1).
3. **No Close-Price Cheating**: Strategy cannot use `bar.close` of current bar to decide action on same bar. It sees the bar AFTER it closes.
4. **Deterministic Replay**: Same data + same config + same strategy = EXACTLY same result. No randomness in execution.
5. **Audit Trail**: Every order fill records the bar index and timestamp that triggered it, for post-hoc leakage verification.

---

## 14. Agent Implementation Boundaries

| Agent | Crate/Module | Must NOT Touch |
|-------|-------------|--------------|
| Data_Pipeline | `data_pipeline/` | `engine/`, `orderbook/` logic |
| Backtest_Engine | `engine/`, `orderbook/`, `indicators/` | `data_pipeline/` storage internals |
| Strategy | `api/`, `python_strategies/` | `engine/` internals, `orderbook/` direct access |
| Frontend | `frontend/` | Any backend logic |
| DevOps | `.github/`, `docs/`, `docker-compose.yml` | Application code |

All agents communicate ONLY through the interfaces defined in Section 4.
