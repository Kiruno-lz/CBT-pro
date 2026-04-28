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

### Step 5 — Register in the API layer

Open `rust_core/crates/api/src/server.rs` and add a new arm to the `match strategy_id` block (see Section 8).

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

## 6. Code Examples

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

A strategy that computes fast and slow EMAs and generates signals on crossovers.

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

impl EmaCrossover {
    fn ema(values: &[Decimal], period: usize) -> Option<Decimal> {
        if values.len() < period {
            return None;
        }
        let slice = &values[values.len() - period..];
        let multiplier = Decimal::from(2) / Decimal::from(period + 1);
        let mut ema = slice[0];
        for val in &slice[1..] {
            ema = (*val - ema) * multiplier + ema;
        }
        Some(ema)
    }
}

impl Strategy for EmaCrossover {
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal> {
        // Guard: need at least slow_period + 1 bars
        if ctx.current_idx < self.slow_period + 1 {
            return vec![];
        }

        let available = &ctx.historical_bars[..=ctx.current_idx];
        let closes: Vec<Decimal> = available.iter().map(|b| b.close).collect();

        let fast_ema = Self::ema(&closes, self.fast_period)?;
        let slow_ema = Self::ema(&closes, self.slow_period)?;
        let prev_fast = Self::ema(&closes[..closes.len() - 1], self.fast_period)?;
        let prev_slow = Self::ema(&closes[..closes.len() - 1], self.slow_period)?;

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

## 7. Best Practices

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

---

## 8. Registration

CBT-Pro uses **pure-code registration**. There are no configuration files, YAML manifests, or dynamic libraries. To make your strategy available to the REST API, edit the match statement in `rust_core/crates/api/src/server.rs`.

### 1. Import your strategy

```rust
use strategy::{AlwaysLong, EmaCrossover, MyStrategy};
```

### 2. Add a match arm in `start_backtest`

Locate the block that looks like this:

```rust
let strategy: Option<Box<dyn strategy::Strategy>> = match strategy_id {
    "always_long" => Some(Box::new(strategy::AlwaysLong::new(
        config.symbol.clone(),
        Decimal::from_str("0.1").unwrap(),
    ))),
    "ema_crossover" => Some(Box::new(strategy::EmaCrossover {
        symbol: config.symbol.clone(),
        quantity: Decimal::from_str("0.1").unwrap(),
        fast_period: 9,
        slow_period: 21,
    })),
    _ => None,
};
```

Append your variant:

```rust
    "my_strategy" => Some(Box::new(strategy::MyStrategy {
        symbol: config.symbol.clone(),
        quantity: Decimal::from_str("0.1").unwrap(),
        lookback: 20,
    })),
```

### 3. Rebuild

```bash
cd rust_core
cargo build --release
```

Clients can now start a backtest with `"strategy_id": "my_strategy"`.

---

## 9. Testing Your Strategy

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

---

## Summary Checklist

- [ ] Create `.rs` file in `rust_core/crates/strategy/src/`
- [ ] Implement `Strategy` trait (`on_bar`, optional `save_state`/`load_state`)
- [ ] Export module and type from `rust_core/crates/strategy/src/lib.rs`
- [ ] Register match arm in `rust_core/crates/api/src/server.rs`
- [ ] Add unit tests with synthetic bars
- [ ] Run `cargo test` and `cargo clippy` before committing

Happy strategising!
