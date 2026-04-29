# CBT-Pro Strategy Developer Guide

This guide is a complete reference for developers who want to create, register, and test new trading strategies in the CBT-Pro backtesting system.

---

## 1. Overview

The strategy module lives in `rust_core/crates/strategy/` and defines a trait-based plugin architecture. Every strategy is a Rust struct that implements the `Strategy` trait. On each bar of a backtest, the engine calls `on_bar`, passing a `StrategyContext` that contains all current market data, open positions, and account state. The strategy returns zero or more `Signal` values, which the engine translates into orders.

There is no dynamic loading, configuration-file magic, or runtime reflection. Strategies are plain Rust code that are instantiated directly in the API layer.

---

## 2. Core Concepts

### The `Strategy` Trait

Every strategy must implement `strategy::Strategy`:

```rust
use strategy::{Strategy, StrategyContext, Signal, SignalAction};

pub trait Strategy: Send + Sync {
    /// Called on each bar to generate trading signals.
    /// Returns a vector of signals (empty if no action).
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal>;

    /// Save internal state for persistence (optional).
    fn save_state(&self) -> Option<Vec<u8>> { None }

    /// Load internal state from persisted data (optional).
    fn load_state(&mut self, _state: &[u8]) -> Result<(), StrategyError> { Ok(()) }
}
```

- **`on_bar`** — The only required method. It receives read-only context and returns a list of signals.
- **`save_state` / `load_state`** — Optional hooks for persisting internal counters, learned parameters, or machine-learning weights between runs.

### `StrategyContext`

The engine provides a single `StrategyContext` on every tick. It is a snapshot of the world at the current bar:

```rust
pub struct StrategyContext<'a> {
    pub current_bar: &'a StandardBar,
    pub historical_bars: &'a [StandardBar],
    pub current_idx: usize,
    pub positions: &'a [Position],
    pub equity: Decimal,
    pub available_balance: Decimal,
}
```

All fields are documented in detail in Section 4.

### `Signal` and `SignalAction`

A `Signal` is an instruction emitted by a strategy:

```rust
pub struct Signal {
    pub action: SignalAction,
    pub symbol: String,
    pub quantity: Option<Decimal>,
    pub strength: f64,
    pub reason: String,
    pub stop_loss: Option<Decimal>,
    pub take_profit: Option<Decimal>,
}
```

- **`action`** — What to do (open, close, reduce, etc.).
- **`symbol`** — The trading pair (e.g. `"BTC-USDT"`).
- **`quantity`** — Override the default order size. `None` lets the engine decide.
- **`strength`** — Normalised confidence in the range `0.0` to `1.0`.
- **`reason`** — Human-readable tag for logs and debugging.
- **`stop_loss` / `take_profit`** — Optional price levels.

### State Persistence

If your strategy accumulates state (e.g. a custom volatility estimate), implement `save_state` and `load_state`.

---

## 3. Creating a New Strategy

### Step 1 — Create a new source file

Add a file inside `rust_core/crates/strategy/src/`, for example `my_strategy.rs`.

### Step 2 — Define your struct

```rust
use crate::base::*;
use rust_decimal::Decimal;

pub struct MyStrategy {
    pub symbol: String,
    pub quantity: Decimal,
    pub lookback: usize,
}
```

### Step 3 — Implement the `Strategy` trait

```rust
impl Strategy for MyStrategy {
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal> {
        // Wait until enough history is available
        if ctx.current_idx < self.lookback {
            return vec![];
        }

        // Access the current bar
        let bar = ctx.current_bar;
        let _price = bar.close;

        // Access lookback history safely
        let history = &ctx.historical_bars[..=ctx.current_idx];
        let _prev_close = history[history.len() - 2].close;

        // Access positions
        let has_long = ctx.positions.iter().any(|p| p.direction == Direction::Long);

        // Return signals (empty vec means "do nothing")
        vec![]
    }
}
```

### Step 4 — Export from `lib.rs`

Open `rust_core/crates/strategy/src/lib.rs` and add:

```rust
pub mod my_strategy;
pub use my_strategy::MyStrategy;
```

### Step 5 — Register in the strategy registry

Add your strategy to the `available_strategies()` function in `rust_core/crates/strategy/src/config.rs` (see Section 8).

---

## 4. StrategyContext Fields

| Field | Type | Description |
|---|---|---|
| `current_bar` | `&StandardBar` | The bar currently being processed. Contains OHLCV, timestamp, symbol, exchange, and `confirmed` flag. |
| `historical_bars` | `&[StandardBar]` | **All** bars from the start of the backtest up to and including the current bar. Use `historical_bars[..=current_idx]` for a safe lookback slice. |
| `current_idx` | `usize` | Zero-based index of the current bar inside `historical_bars`. Useful for guarding against insufficient history. |
| `positions` | `&[Position]` | Slice of currently **open** positions. Empty when flat. |
| `equity` | `Decimal` | Current total account equity (balance + unrealised PnL). |
| `available_balance` | `Decimal` | Free cash that can be used to open new positions. |

### `StandardBar` fields

```rust
pub struct StandardBar {
    pub timestamp: i64,      // Unix timestamp in seconds
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub symbol: String,
    pub exchange: String,
    pub confirmed: bool,
}
```

### `Position` fields

```rust
pub struct Position {
    pub id: PositionId,                 // UUID
    pub symbol: String,
    pub direction: Direction,           // Long | Short
    pub status: PositionStatus,         // Open | PartiallyClosed | Closed
    pub entries: Vec<PositionLeg>,
    pub current_size: Decimal,
    pub average_entry_price: Decimal,
    pub realized_pnl: Decimal,
    pub unrealized_pnl: Decimal,
    pub opened_at: i64,
    pub updated_at: i64,
}
```

---

## 5. SignalAction Variants

```rust
pub enum SignalAction {
    OpenLong,
    OpenShort,
    CloseLong,
    CloseShort,
    CloseAll,
    ReduceLong(Decimal),
    ReduceShort(Decimal),
}
```

| Variant | Meaning |
|---|---|
| `OpenLong` | Open a new long position. |
| `OpenShort` | Open a new short position. |
| `CloseLong` | Close an existing long position entirely. |
| `CloseShort` | Close an existing short position entirely. |
| `CloseAll` | Close every open position (both long and short). |
| `ReduceLong(Decimal)` | Partially close a long position by the given `Decimal` quantity. |
| `ReduceShort(Decimal)` | Partially close a short position by the given `Decimal` quantity. |

---

## 6. Indicator Usage Rule

All indicator-based strategies **MUST** use the `indicators` crate instead of computing indicators internally.

### Why?

- **Consistency** — All strategies use the same indicator implementations, ensuring identical calculations across the system.
- **Reusability** — Indicators are maintained in one place. Bug fixes and optimizations benefit every strategy automatically.
- **Testing** — The `indicators` crate has its own comprehensive test suite. You don't need to re-test EMA or RSI math in your strategy tests.
- **Performance** — Indicator implementations may be optimized (e.g., incremental updates, SIMD) transparently.

### Available Indicator Functions

```rust
use indicators;

// Exponential Moving Average
indicators::ema(period: usize, prices: &[Decimal]) -> Result<Vec<IndicatorValue>, String>

// Relative Strength Index
indicators::rsi(period: usize, prices: &[Decimal]) -> Result<Vec<IndicatorValue>, String>

// Bollinger Bands
indicators::bollinger(period: usize, std_dev: Decimal, prices: &[Decimal]) -> Result<Vec<BollingerBands>, String>

// MACD
indicators::macd(fast: usize, slow: usize, signal: usize, prices: &[Decimal]) -> Result<Vec<MacdValue>, String>

// Average True Range
indicators::atr(period: usize, highs: &[Decimal], lows: &[Decimal], closes: &[Decimal]) -> Result<Vec<IndicatorValue>, String>

// Volume-Weighted Average Price
indicators::vwap(prices: &[Decimal], volumes: &[Decimal]) -> Result<Vec<IndicatorValue>, String>
```

### Usage in a Strategy

```rust
impl Strategy for MyStrategy {
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal> {
        if ctx.current_idx < self.period {
            return vec![];
        }

        let closes: Vec<Decimal> = ctx.historical_bars[..=ctx.current_idx]
            .iter().map(|b| b.close).collect();

        let rsi_values = match indicators::rsi(self.period, &closes) {
            Ok(v) => v,
            Err(_) => return vec![],
        };

        let current_rsi = rsi_values.last().unwrap().value;
        // ... signal logic based on RSI ...
        vec![]
    }
}
```

---

## 7. Strategy Configuration Interface

Each strategy must expose its configurable parameters through the configuration interface. This enables the REST API to discover strategy metadata and allows users to customize parameters at runtime.

### Types

```rust
// strategy/src/config.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParamType {
    Integer { min: i64, max: i64, default: i64 },
    Decimal { min: String, max: String, default: String },
    String { default: String, options: Vec<String> },
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
```

### Adding Configurable Parameters to a Strategy

```rust
use crate::base::*;
use rust_decimal::Decimal;
use serde::{Serialize, Deserialize};

pub struct MovingAverageCrossover {
    pub symbol: String,
    pub quantity: Decimal,
    pub fast_period: usize,
    pub slow_period: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovingAverageCrossoverParams {
    pub fast_period: i64,
    pub slow_period: i64,
}

impl MovingAverageCrossover {
    pub fn new(symbol: String, quantity: Decimal, params: serde_json::Value) -> Result<Self, String> {
        let p: MovingAverageCrossoverParams = serde_json::from_value(params)
            .map_err(|e| format!("Invalid params: {}", e))?;
        
        if p.fast_period >= p.slow_period {
            return Err("fast_period must be less than slow_period".to_string());
        }
        
        Ok(Self {
            symbol,
            quantity,
            fast_period: p.fast_period as usize,
            slow_period: p.slow_period as usize,
        })
    }
}

impl Strategy for MovingAverageCrossover {
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal> {
        if ctx.current_idx < self.slow_period + 1 {
            return vec![];
        }

        let closes: Vec<Decimal> = ctx.historical_bars[..=ctx.current_idx]
            .iter().map(|b| b.close).collect();

        let fast_results = match indicators::ema(self.fast_period, &closes) {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        let slow_results = match indicators::ema(self.slow_period, &closes) {
            Ok(v) => v,
            Err(_) => return vec![],
        };

        let fast_ema = fast_results.last().unwrap().value;
        let slow_ema = slow_results.last().unwrap().value;
        let prev_fast = fast_results[fast_results.len() - 2].value;
        let prev_slow = slow_results[slow_results.len() - 2].value;

        if prev_fast <= prev_slow && fast_ema > slow_ema {
            vec![Signal {
                action: SignalAction::OpenLong,
                symbol: self.symbol.clone(),
                quantity: Some(self.quantity),
                strength: 1.0,
                reason: "MA crossover bullish".to_string(),
                stop_loss: None,
                take_profit: None,
            }]
        } else if prev_fast >= prev_slow && fast_ema < slow_ema {
            vec![Signal {
                action: SignalAction::CloseLong,
                symbol: self.symbol.clone(),
                quantity: Some(self.quantity),
                strength: 1.0,
                reason: "MA crossover bearish".to_string(),
                stop_loss: None,
                take_profit: None,
            }]
        } else {
            vec![]
        }
    }
}
```

### Registering in `available_strategies()`

```rust
// rust_core/crates/strategy/src/config.rs
pub fn available_strategies() -> Vec<StrategyInfo> {
    vec![
        StrategyInfo {
            id: "always_long",
            name: "Always Long",
            description: "Opens a long position on the first bar and holds.",
            default_params: serde_json::json!({}),
            param_definitions: vec![],
            create: |symbol, quantity, _params| {
                Ok(Box::new(AlwaysLong::new(symbol, quantity)))
            },
        },
        StrategyInfo {
            id: "moving_average_crossover",
            name: "Moving Average Crossover",
            description: "Generates signals based on EMA crossovers.",
            default_params: serde_json::json!({
                "fast_period": 9,
                "slow_period": 21
            }),
            param_definitions: vec![
                ParamDefinition {
                    name: "fast_period".to_string(),
                    description: "Fast EMA period".to_string(),
                    param_type: ParamType::Integer {
                        min: 2,
                        max: 100,
                        default: 9,
                    },
                },
                ParamDefinition {
                    name: "slow_period".to_string(),
                    description: "Slow EMA period".to_string(),
                    param_type: ParamType::Integer {
                        min: 5,
                        max: 200,
                        default: 21,
                    },
                },
            ],
            create: |symbol, quantity, params| {
                MovingAverageCrossover::new(symbol, quantity, params)
                    .map(|s| Box::new(s) as Box<dyn Strategy>)
            },
        },
    ]
}
```

---

## 8. Code Examples

### Example 1 — Simple Strategy (AlwaysLong)

A minimal strategy that opens a single long position on the very first bar and then stays flat.

```rust
// rust_core/crates/strategy/src/always_long.rs
use crate::base::*;
use rust_decimal::Decimal;

pub struct AlwaysLong {
    pub symbol: String,
    pub quantity: Decimal,
    has_position: bool,
}

impl AlwaysLong {
    pub fn new(symbol: String, quantity: Decimal) -> Self {
        Self {
            symbol,
            quantity,
            has_position: false,
        }
    }
}

impl Strategy for AlwaysLong {
    fn on_bar(&mut self, _ctx: &StrategyContext) -> Vec<Signal> {
        if self.has_position {
            return vec![];
        }
        self.has_position = true;
        vec![Signal {
            action: SignalAction::OpenLong,
            symbol: self.symbol.clone(),
            quantity: Some(self.quantity),
            strength: 1.0,
            reason: "AlwaysLong: first bar entry".to_string(),
            stop_loss: None,
            take_profit: None,
        }]
    }
}
```

### Example 2 — Indicator-Based Strategy (EMA Crossover)

A strategy that computes fast and slow EMAs using the `indicators` crate and generates signals on crossovers.

```rust
// rust_core/crates/strategy/src/ema_crossover.rs
use crate::base::*;
use rust_decimal::Decimal;

pub struct EmaCrossover {
    pub symbol: String,
    pub quantity: Decimal,
    pub fast_period: usize,
    pub slow_period: usize,
}

impl Strategy for EmaCrossover {
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal> {
        if ctx.current_idx < self.slow_period + 1 {
            return vec![];
        }
        let closes: Vec<Decimal> = ctx.historical_bars[..=ctx.current_idx]
            .iter().map(|b| b.close).collect();
        
        let fast_results = match indicators::ema(self.fast_period, &closes) {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        let slow_results = match indicators::ema(self.slow_period, &closes) {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        
        let fast_ema = fast_results.last().unwrap().value;
        let slow_ema = slow_results.last().unwrap().value;
        let prev_fast = fast_results[fast_results.len() - 2].value;
        let prev_slow = slow_results[slow_results.len() - 2].value;
        
        if prev_fast <= prev_slow && fast_ema > slow_ema {
            vec![Signal {
                action: SignalAction::OpenLong,
                symbol: self.symbol.clone(),
                quantity: Some(self.quantity),
                strength: 1.0,
                reason: "EMA crossover bullish".to_string(),
                stop_loss: None,
                take_profit: None,
            }]
        } else if prev_fast >= prev_slow && fast_ema < slow_ema {
            vec![Signal {
                action: SignalAction::CloseLong,
                symbol: self.symbol.clone(),
                quantity: Some(self.quantity),
                strength: 1.0,
                reason: "EMA crossover bearish".to_string(),
                stop_loss: None,
                take_profit: None,
            }]
        } else {
            vec![]
        }
    }
}
```

### Example 3 — Advanced Strategy (Balance-Aware Sizer)

A strategy that dynamically scales position size based on available balance, respects existing positions, and persists a simple counter via `save_state`/`load_state`.

```rust
// rust_core/crates/strategy/src/balance_aware.rs
use crate::base::*;
use rust_decimal::Decimal;
use serde::{Serialize, Deserialize};

pub struct BalanceAware {
    pub symbol: String,
    pub base_quantity: Decimal,
    pub max_risk_pct: Decimal, // e.g. 0.05 for 5%
    pub lookback: usize,
    bars_since_entry: usize,
}

#[derive(Serialize, Deserialize)]
struct BalanceAwareState {
    bars_since_entry: usize,
}

impl BalanceAware {
    pub fn new(symbol: String, base_quantity: Decimal, max_risk_pct: Decimal, lookback: usize) -> Self {
        Self {
            symbol,
            base_quantity,
            max_risk_pct,
            lookback,
            bars_since_entry: 0,
        }
    }

    fn sma(bars: &[StandardBar], period: usize) -> Option<Decimal> {
        if bars.len() < period {
            return None;
        }
        let sum: Decimal = bars.iter().rev().take(period).map(|b| b.close).sum();
        Some(sum / Decimal::from(period))
    }
}

impl Strategy for BalanceAware {
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal> {
        if ctx.current_idx < self.lookback {
            return vec![];
        }

        let history = &ctx.historical_bars[..=ctx.current_idx];
        let sma = match Self::sma(history, self.lookback) {
            Some(v) => v,
            None => return vec![],
        };

        let price = ctx.current_bar.close;
        let has_long = ctx.positions.iter().any(|p| p.direction == Direction::Long);

        // Dynamic sizing: risk a percentage of available balance
        let risk_amount = ctx.available_balance * self.max_risk_pct;
        let sized_qty = if price > Decimal::ZERO {
            (risk_amount / price).max(self.base_quantity)
        } else {
            self.base_quantity
        };

        // Entry: price above SMA and no existing long
        if price > sma && !has_long {
            self.bars_since_entry = 0;
            return vec![Signal {
                action: SignalAction::OpenLong,
                symbol: self.symbol.clone(),
                quantity: Some(sized_qty),
                strength: 0.85,
                reason: format!("Price {} > SMA {}", price, sma),
                stop_loss: Some(price * Decimal::from_str("0.95").unwrap()),
                take_profit: Some(price * Decimal::from_str("1.10").unwrap()),
            }];
        }

        // Exit: price below SMA and we are long
        if price < sma && has_long {
            return vec![Signal {
                action: SignalAction::CloseLong,
                symbol: self.symbol.clone(),
                quantity: None,
                strength: 0.9,
                reason: format!("Price {} < SMA {}", price, sma),
                stop_loss: None,
                take_profit: None,
            }];
        }

        self.bars_since_entry += 1;
        vec![]
    }

    fn save_state(&self) -> Option<Vec<u8>> {
        let state = BalanceAwareState {
            bars_since_entry: self.bars_since_entry,
        };
        serde_json::to_vec(&state).ok()
    }

    fn load_state(&mut self, state: &[u8]) -> Result<(), StrategyError> {
        let s: BalanceAwareState = serde_json::from_slice(state)
            .map_err(|e| StrategyError::StateError(e.to_string()))?;
        self.bars_since_entry = s.bars_since_entry;
        Ok(())
    }
}
```

---

## 9. Best Practices

1. **Guard for minimum data**  
   Always check `ctx.current_idx` before accessing history. If your strategy needs a 50-period lookback, return `vec![]` until `current_idx >= 50`.

2. **Use inclusive slicing for lookback**  
   ```rust
   let history = &ctx.historical_bars[..=ctx.current_idx];
   ```
   This gives you every bar from the start up to and including the current bar, avoiding out-of-bounds access.

3. **Return an empty vector when idle**  
   Returning `vec![]` is the idiomatic way to say "no action this bar". Do not return a `Signal` with a no-op action.

4. **Set meaningful `strength` values**  
   Use the `0.0` to `1.0` range to express confidence. This field is consumed by risk-management layers and logging pipelines.

5. **Write a descriptive `reason`**  
   The `reason` string appears in trade logs and backtest reports. Something like `"EMA(9) crossed above EMA(21)"` is far more useful than `"signal"`.

6. **Avoid floating-point math on prices**  
   All financial values in CBT-Pro are `rust_decimal::Decimal`. Do not cast to `f64` for price calculations unless you are computing indicators that explicitly require it.

7. **Keep state minimal**  
   Strategies are recreated on every backtest request. If you need persistence, implement `save_state`/`load_state`, but prefer pure, deterministic logic derived from `StrategyContext`.

8. **Always use the `indicators` crate**  
   Never compute EMA, RSI, Bollinger Bands, MACD, ATR, or VWAP manually. Import from `indicators` for consistency, correctness, and maintainability.

9. **Expose configurable parameters**  
   Define `ParamDefinition`s for every tunable value in your strategy. This makes your strategy usable via the REST API and allows parameter optimization without code changes.

10. **Validate parameters in the factory function**  
    The `create` function should validate `strategy_params` and return a descriptive `Err(String)` on invalid input. The API will forward this message to the client.

---

## 10. Registration

CBT-Pro uses a **strategy registry** in `rust_core/crates/strategy/src/config.rs`. The API queries this registry dynamically, so no API code changes are required when adding a new strategy.

### How it works

1. Create your strategy struct and implement the `Strategy` trait.
2. Define a factory function that accepts `(String, Decimal, serde_json::Value)` and returns `Result<Box<dyn Strategy>, String>`.
3. Add a `StrategyInfo` entry to `available_strategies()`.

The API will automatically:
- List your strategy in `GET /api/v1/strategies`
- Return its default parameters in `GET /api/v1/strategies/:id/defaults`
- Instantiate it via the factory when `POST /api/v1/backtest/start` is called with your strategy ID

### Example: Registering a Strategy

```rust
// rust_core/crates/strategy/src/config.rs

use serde_json;

pub fn available_strategies() -> Vec<StrategyInfo> {
    vec![
        // ... existing strategies ...
        
        StrategyInfo {
            id: "my_strategy",
            name: "My Strategy",
            description: "A custom strategy with configurable lookback.",
            default_params: serde_json::json!({
                "lookback": 20
            }),
            param_definitions: vec![
                ParamDefinition {
                    name: "lookback".to_string(),
                    description: "Number of bars to look back".to_string(),
                    param_type: ParamType::Integer {
                        min: 5,
                        max: 200,
                        default: 20,
                    },
                },
            ],
            create: |symbol, quantity, params| {
                let lookback = params.get("lookback")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(20) as usize;
                
                Ok(Box::new(MyStrategy {
                    symbol,
                    quantity,
                    lookback,
                }))
            },
        },
    ]
}
```

### Parameter passing flow

When a client sends a backtest request:

```json
{
  "strategy_id": "my_strategy",
  "strategy_params": {
    "lookback": 30
  }
}
```

The API:
1. Looks up `"my_strategy"` in the registry returned by `available_strategies()`
2. Merges client-provided `strategy_params` with `default_params`
3. Calls the `create` factory with `(symbol, quantity, merged_params)`
4. The factory validates parameters and returns `Box<dyn Strategy>`

If `strategy_params` is omitted, the API uses `default_params` as-is.

### Rebuild

```bash
cd rust_core
cargo build --release
```

Clients can now start a backtest with `"strategy_id": "my_strategy"` and optionally override parameters via `strategy_params`.

---

## 11. API Integration

### Endpoints

#### `GET /api/v1/strategies`

List all available strategies with their metadata.

**Response:**

```json
{
  "strategies": [
    {
      "id": "always_long",
      "name": "Always Long",
      "description": "Opens a long position on the first bar and holds."
    },
    {
      "id": "moving_average_crossover",
      "name": "Moving Average Crossover",
      "description": "Generates signals based on EMA crossovers."
    }
  ]
}
```

#### `GET /api/v1/strategies/:id/defaults`

Get default parameters and parameter definitions for a specific strategy.

**Request:**

```bash
curl http://localhost:8080/api/v1/strategies/moving_average_crossover/defaults
```

**Response:**

```json
{
  "id": "moving_average_crossover",
  "name": "Moving Average Crossover",
  "description": "Generates signals based on EMA crossovers.",
  "default_params": {
    "fast_period": 9,
    "slow_period": 21
  },
  "param_definitions": [
    {
      "name": "fast_period",
      "description": "Fast EMA period",
      "param_type": {
        "Integer": {
          "min": 2,
          "max": 100,
          "default": 9
        }
      }
    },
    {
      "name": "slow_period",
      "description": "Slow EMA period",
      "param_type": {
        "Integer": {
          "min": 5,
          "max": 200,
          "default": 21
        }
      }
    }
  ]
}
```

#### `POST /api/v1/backtest/start`

Start a new backtest. Now accepts an optional `strategy_params` field to customize strategy parameters.

**Request:**

```bash
curl -X POST http://localhost:8080/api/v1/backtest/start \
  -H "Content-Type: application/json" \
  -d '{
    "strategy_id": "moving_average_crossover",
    "strategy_params": {
      "fast_period": 12,
      "slow_period": 26
    },
    "symbol": "BTC-USDT",
    "timeframe": "1h",
    "start_time": "2024-01-01T00:00:00Z",
    "end_time": "2024-06-01T00:00:00Z",
    "initial_balance": "100000",
    "quantity": "0.1"
  }'
```

**Key fields:**

| Field | Type | Required | Description |
|---|---|---|---|
| `strategy_id` | string | yes | Strategy identifier from `GET /api/v1/strategies` |
| `strategy_params` | object | no | Parameter overrides. Must match the strategy's `ParamDefinition`s. |
| `symbol` | string | yes | Trading pair (e.g. `"BTC-USDT"`) |
| `timeframe` | string | yes | Bar timeframe (e.g. `"1h"`, `"4h"`, `"1d"`) |
| `start_time` | string | yes | ISO 8601 start timestamp |
| `end_time` | string | yes | ISO 8601 end timestamp |
| `initial_balance` | string | yes | Starting account balance |
| `quantity` | string | yes | Default order quantity |

**Response:**

```json
{
  "backtest_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "running",
  "strategy_id": "moving_average_crossover",
  "strategy_params": {
    "fast_period": 12,
    "slow_period": 26
  }
}
```

### Error Responses

If parameters are invalid, the API returns a `400 Bad Request`:

```json
{
  "error": "Invalid strategy parameters",
  "details": "fast_period must be less than slow_period"
}
```

If the strategy ID is unknown:

```json
{
  "error": "Unknown strategy",
  "details": "Strategy 'unknown_strategy' not found in registry"
}
```

---

## 12. Testing Your Strategy

### Unit tests inside the strategy crate

Add a `#[cfg(test)]` module at the bottom of your strategy file. Use synthetic bars so tests are deterministic.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use data_pipeline::StandardBar;
    use rust_decimal::Decimal;

    fn make_bars(count: usize) -> Vec<StandardBar> {
        let mut bars = Vec::with_capacity(count);
        let mut price = Decimal::from(40000);
        for i in 0..count {
            bars.push(StandardBar {
                timestamp: i as i64 * 60,
                open: price,
                high: price + Decimal::from(10),
                low: price - Decimal::from(10),
                close: price + Decimal::from(5),
                volume: Decimal::from(100),
                symbol: "BTC-USDT".to_string(),
                exchange: "binance".to_string(),
                confirmed: true,
            });
            price += Decimal::from(5);
        }
        bars
    }

    #[test]
    fn test_always_long_emits_once() {
        let bars = make_bars(5);
        let mut strategy = AlwaysLong::new("BTC-USDT".to_string(), Decimal::from(1));

        let mut signals = 0;
        for (idx, bar) in bars.iter().enumerate() {
            let ctx = StrategyContext {
                current_bar: bar,
                historical_bars: &bars[..=idx],
                current_idx: idx,
                positions: &[],
                equity: Decimal::from(100000),
                available_balance: Decimal::from(100000),
            };
            let sigs = strategy.on_bar(&ctx);
            signals += sigs.len();
        }
        assert_eq!(signals, 1, "AlwaysLong should only emit one signal");
    }
}
```

### Integration tests with `BacktestEngine`

For end-to-end validation, instantiate the engine directly:

```rust
use engine::{BacktestEngine, EngineConfig};
use orderbook::MarginMode;
use data_pipeline::StandardBar;
use strategy::MyStrategy;
use rust_decimal::Decimal;

#[test]
fn test_my_strategy_backtest() {
    let bars: Vec<StandardBar> = make_bars(200);
    let config = EngineConfig {
        symbol: "BTC-USDT".to_string(),
        initial_balance: Decimal::from(100000),
        margin_mode: MarginMode::Cross,
        default_leverage: Decimal::from(10),
        default_quantity: Decimal::from_str("0.1").unwrap(),
        maker_fee_rate: Decimal::from_str("0.0002").unwrap(),
        taker_fee_rate: Decimal::from_str("0.0005").unwrap(),
        maintenance_margin_rate: Decimal::from_str("0.004").unwrap(),
        funding_interval_hours: 8,
        cost_basis_method: orderbook::CostBasisMethod::FIFO,
        execution_delay_bars: 1,
        allow_future_data: false,
        risk_free_rate: 0.02,
    };

    let strategy = Some(Box::new(MyStrategy::new(
        "BTC-USDT".to_string(),
        Decimal::from_str("0.1").unwrap(),
        Decimal::from_str("0.05").unwrap(),
        20,
    )) as Box<dyn strategy::Strategy>);

    let mut engine = BacktestEngine::new(config, bars, strategy);
    let result = engine.run().expect("backtest should complete");

    assert!(result.total_trades > 0, "strategy should have traded");
}
```

### Key testing tips

- **Determinism** — Always use the same seed or hard-coded bar sequences so tests do not flake.
- **Edge cases** — Test with fewer bars than your lookback period to verify graceful no-ops.
- **Position overlap** — Verify that your strategy does not emit `OpenLong` when a long is already open, unless that is intentional (e.g. pyramiding).
- **State round-trip** — If you implement `save_state`/`load_state`, write a test that serialises, deserialises, and asserts identical behaviour.
- **Parameter validation** — Test your factory function with invalid parameters to ensure it returns clear error messages.

---

## Summary Checklist

- [ ] Create `.rs` file in `rust_core/crates/strategy/src/`
- [ ] Implement `Strategy` trait (`on_bar`, optional `save_state`/`load_state`)
- [ ] Use the `indicators` crate for all indicator calculations (EMA, RSI, Bollinger, MACD, ATR, VWAP)
- [ ] Define configurable parameters with `ParamDefinition`s
- [ ] Add parameter validation in the factory function
- [ ] Export module and type from `rust_core/crates/strategy/src/lib.rs`
- [ ] Register `StrategyInfo` in `available_strategies()` in `rust_core/crates/strategy/src/config.rs`
- [ ] Add unit tests with synthetic bars
- [ ] Test parameter validation and edge cases
- [ ] Run `cargo test` and `cargo clippy` before committing

Happy strategising!
