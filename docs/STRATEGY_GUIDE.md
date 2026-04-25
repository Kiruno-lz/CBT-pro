# Strategy Development Guide

## Overview

CBT-Pro strategies are written in Python and communicate with the Rust backtest engine via the `BaseStrategy` abstract class. The engine enforces anti-data-leakage rules to ensure your strategy operates on realistic, historical data.

## Quick Start

### 1. Create Your First Strategy

```python
from decimal import Decimal
from typing import Optional
from cbt_pro import BaseStrategy, Signal, SignalAction, StrategyContext, StandardBar

class EmaCrossStrategy(BaseStrategy):
    @property
    def name(self) -> str:
        return "ema_cross"

    @property
    def version(self) -> str:
        return "1.0.0"

    @property
    def required_indicators(self) -> list[str]:
        return ["ema_9", "ema_21"]

    def on_bar(self, bar: StandardBar, context: StrategyContext) -> Optional[Signal]:
        # Strategy sees ONLY bars up to and including current bar
        # It CANNOT access bars[current_idx + 1]
        history = context.bar_history
        if len(history) < 21:
            return None

        ema9 = self._ema(history, 9)
        ema21 = self._ema(history, 21)

        if ema9[-2] <= ema21[-2] and ema9[-1] > ema21[-1]:
            return Signal(
                action=SignalAction.OPEN_LONG,
                symbol=bar.symbol,
                quantity=Decimal("0.1"),
                strength=0.8,
                reason="EMA9 crossed above EMA21",
            )
        return None

    def on_position_update(
        self, position, context
    ) -> Optional[Signal]:
        return None

    def _ema(self, history, period):
        # Simplified EMA calculation
        prices = [b.close for b in history]
        k = Decimal(2) / Decimal(period + 1)
        ema = [prices[0]]
        for price in prices[1:]:
            ema.append(price * k + ema[-1] * (Decimal(1) - k))
        return ema
```

### 2. Register and Run

```python
import asyncio
from cbt_pro.backtest_client import BacktestClient

async def main():
    client = BacktestClient("http://localhost:8080")

    # Register your strategy with the engine
    await client.register_strategy(EmaCrossStrategy())

    # Start a backtest
    result = await client.start_backtest(
        config={
            "symbol": "BTC-USDT",
            "initial_balance": "100000",
            "margin_mode": "Cross",
            "default_leverage": "10",
            "maker_fee_rate": "0.001",
            "taker_fee_rate": "0.005",
            "maintenance_margin_rate": "0.005",
            "funding_interval_hours": 8,
            "cost_basis_method": "FIFO",
            "execution_delay_bars": 1,
            "allow_future_data": False,
            "risk_free_rate": 0.02,
        },
        strategy_id="ema_cross",
        timeframe="1h",
        start_time=1704067200000,
        end_time=1706745600000,
    )

    print(f"Backtest started: {result['backtest_id']}")
    print(f"Total bars: {result['total_bars']}")

    # Poll for completion
    while True:
        state = await client.get_state(result["backtest_id"])
        if state.get("status") == "complete":
            break
        await asyncio.sleep(1)

    # Fetch results
    final = await client.get_result(result["backtest_id"])
    print(f"Final equity: {final['final_equity']}")
    print(f"Total return: {final['total_return_pct']}%")
    print(f"Max drawdown: {final['max_drawdown_pct']}%")
    print(f"Sharpe ratio: {final['sharpe_ratio']}")

asyncio.run(main())
```

## Interface Contract

### `BaseStrategy` Abstract Methods

| Method | When Called | Return |
|--------|------------|--------|
| `name` | Registration | Strategy identifier |
| `version` | Registration | Version string |
| `required_indicators` | Setup | List of indicator names |
| `on_bar(bar, context)` | Every bar close | `Signal` or `None` |
| `on_position_update(pos, context)` | Position state change | `Signal` or `None` |

### `StrategyContext` Fields (Read-Only)

| Field | Description |
|-------|------------|
| `current_price` | Current bar close price |
| `open_orders` | Number of unfilled orders |
| `positions` | Active position snapshots |
| `equity` | Total account equity |
| `available_balance` | Free margin |
| `unrealized_pnl` | Open P&L |
| `margin_ratio` | Used margin / equity |
| `bar_history` | All bars up to current index |
| `current_bar_index` | Current bar index |
| `total_bars` | Total bars in backtest |
| `timestamp` | Current bar timestamp |

### `Signal` Fields

| Field | Required | Description |
|-------|----------|------------|
| `action` | Yes | SignalAction enum |
| `symbol` | Yes | Trading pair |
| `quantity` | Yes | Order quantity |
| `strength` | No | 0.0-1.0 signal confidence |
| `reason` | No | Human-readable reason |
| `metadata` | No | Additional data |
| `take_profit` | No | TP price |
| `stop_loss` | No | SL price |

## Indicator Usage Examples

### Built-in Indicators

```python
from cbt_pro.indicators import ema, rsi, bollinger_bands, macd, atr, vwap
from decimal import Decimal

prices = [Decimal(str(100 + i * 2)) for i in range(50)]

# EMA
ema_values = ema(prices, period=9)

# RSI
rsi_values = rsi(prices, period=14)

# Bollinger Bands
upper, middle, lower = bollinger_bands(prices, period=20, std_dev=2)

# MACD
macd_line, signal_line, histogram = macd(prices, fast=12, slow=26, signal=9)

# ATR
highs = [Decimal("110")] * 15
lows = [Decimal("90")] * 15
closes = [Decimal("100")] * 15
atr_values = atr(highs, lows, closes, period=14)

# VWAP
vwap_value = vwap(prices, volumes=[Decimal("10")] * len(prices))
```

### Custom Indicators

You can implement custom indicators directly in your strategy:

```python
def custom_momentum(self, prices: List[Decimal], period: int) -> List[Decimal]:
    """Rate of change indicator."""
    result = []
    for i in range(len(prices)):
        if i < period:
            result.append(Decimal("0"))
        else:
            result.append(prices[i] - prices[i - period])
    return result
```

## BacktestClient Usage with Async/Await

### Basic Operations

```python
from cbt_pro.backtest_client import BacktestClient
import asyncio

client = BacktestClient("http://localhost:8080")

async def run_backtest():
    # Start
    result = await client.start_backtest(config=..., strategy_id="...", ...)
    backtest_id = result["backtest_id"]

    # Pause / Resume
    await client.pause_backtest(backtest_id)
    await client.resume_backtest(backtest_id)

    # Get live state
    state = await client.get_state(backtest_id)
    print(f"Current equity: {state['equity']}")

    # Final result
    final = await client.get_result(backtest_id)
    return final

asyncio.run(run_backtest())
```

### Error Handling

```python
import aiohttp

async def safe_start(client, config, strategy_id, timeframe, start_time, end_time):
    try:
        result = await client.start_backtest(
            config=config,
            strategy_id=strategy_id,
            timeframe=timeframe,
            start_time=start_time,
            end_time=end_time,
        )
        return result
    except aiohttp.ClientError as e:
        print(f"Connection error: {e}")
        raise
    except Exception as e:
        print(f"Backtest failed: {e}")
        raise
```

## Debugging Tips

### Using Engine Snapshots

The engine provides snapshots at every bar. You can inspect them via the WebSocket or REST API:

```python
# Get the latest snapshot
snapshot = await client.get_state(backtest_id)

# Key fields to monitor during debugging:
print(f"Bar index: likely inferred from timestamp")
print(f"Equity: {snapshot['equity']}")
print(f"Positions: {len(snapshot['positions'])}")
print(f"Unrealized PnL: {snapshot['unrealized_pnl']}")
print(f"Margin used: {snapshot['margin_used']}")
print(f"Total trades so far: {snapshot['total_trades']}")
print(f"Win rate: {snapshot['win_rate']}%")
```

### Logging Signals

Always include descriptive `reason` strings in your signals:

```python
return Signal(
    action=SignalAction.OPEN_LONG,
    symbol=bar.symbol,
    quantity=Decimal("0.1"),
    strength=0.85,
    reason=f"RSI({rsi_val:.2f}) oversold at bar {context.current_bar_index}",
    metadata={
        "rsi": float(rsi_val),
        "ema9": float(ema9[-1]),
        "ema21": float(ema21[-1]),
    }
)
```

### Strategy Unit Testing

Test your strategy logic independently of the engine:

```python
def test_ema_cross_strategy():
    strategy = EmaCrossStrategy()
    bars = [...]  # Create synthetic bars
    ctx = StrategyContext(
        current_price=bars[-1].close,
        open_orders=0,
        positions=[],
        equity=Decimal("100000"),
        available_balance=Decimal("100000"),
        unrealized_pnl=Decimal("0"),
        margin_ratio=Decimal("0"),
        bar_history=bars,
        current_bar_index=len(bars) - 1,
        total_bars=100,
        timestamp=bars[-1].timestamp,
    )

    signal = strategy.on_bar(bars[-1], ctx)
    assert signal is not None
    assert signal.action == SignalAction.OPEN_LONG
```

## Anti-Data-Leakage Best Practices

### 1. Never Access Future Data

The engine enforces this, but you should also design your strategy defensively:

```python
# GOOD: Only use bar_history up to current bar
def on_bar(self, bar, context):
    history = context.bar_history  # Engine guarantees len <= current_idx + 1
    if len(history) < 20:
        return None

# BAD: Any attempt to access future data
# The engine will reject strategies that try to peek ahead.
```

### 2. Respect Execution Delay

Signals generated at bar `N` execute at bar `N + execution_delay_bars`:

```python
# Default delay is 1 bar. Your signal logic should NOT assume
# the order fills immediately on the same bar.
def on_bar(self, bar, context):
    # This signal will execute on the NEXT bar, not this one
    if some_condition:
        return Signal(action=SignalAction.OPEN_LONG, ...)
```

### 3. No I/O in Callbacks

`on_bar()` and `on_position_update()` must be pure functions:

```python
# GOOD: Pure computation
def on_bar(self, bar, context):
    return self.calculate_signal(bar, context)

# BAD: Do NOT do this
def on_bar(self, bar, context):
    requests.get("https://api.example.com/data")  # NEVER
    with open("log.txt", "a") as f:  # NEVER
        f.write("bar processed")
    return None
```

### 4. Deterministic Replay

Same data + same config + same strategy = EXACTLY same result:

```python
# GOOD: Deterministic logic
def on_bar(self, bar, context):
    if bar.close > self.previous_high:
        return Signal(...)
    return None

# BAD: Non-deterministic
def on_bar(self, bar, context):
    import random
    if random.random() > 0.5:  # NEVER — breaks reproducibility
        return Signal(...)
    return None
```

### 5. Audit Trail

Every order fill records the bar index and timestamp. After a backtest, you can verify:

```python
result = await client.get_result(backtest_id)
for trade in result["trades"]:
    print(f"Trade at timestamp {trade['timestamp']}")
    # Verify the signal was generated at bar N-1 and executed at bar N
```

## Best Practices Summary

1. Use `Decimal` for all price/quantity calculations
2. Keep `on_bar()` execution time < 1ms per bar
3. Validate required indicators exist before accessing
4. Log signal reasons for post-hoc analysis
5. Test strategies on small datasets before full runs
6. Use `execution_delay_bars >= 1` in production
7. Never set `allow_future_data = True` in production backtests
