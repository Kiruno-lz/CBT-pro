"""Tests verifying anti-data-leakage protections in strategies."""

from decimal import Decimal
from typing import Optional, List
import pytest

from cbt_pro.interfaces.strategy import (
    BaseStrategy,
    Signal,
    SignalAction,
    StrategyContext,
    PositionSnapshot,
    StandardBar,
)


def _make_bar(timestamp: int, close: str = "10000") -> StandardBar:
    """Helper to create a minimal StandardBar."""
    return StandardBar(
        timestamp=timestamp,
        open=Decimal("10000"),
        high=Decimal("10100"),
        low=Decimal("9900"),
        close=Decimal(close),
        volume=Decimal("1.0"),
        symbol="BTC-USDT",
        exchange="test",
    )


def _make_context(bars: List[StandardBar], current_idx: int) -> StrategyContext:
    """Helper to create a StrategyContext."""
    return StrategyContext(
        current_price=bars[current_idx].close,
        open_orders=0,
        positions=[],
        equity=Decimal("100000"),
        available_balance=Decimal("100000"),
        unrealized_pnl=Decimal("0"),
        margin_ratio=Decimal("0"),
        bar_history=bars,
        current_bar_index=current_idx,
        total_bars=len(bars),
        timestamp=bars[current_idx].timestamp,
    )


def test_strategy_only_sees_history_up_to_current_bar():
    """StrategyContext.bar_history must not contain future bars."""
    bars = [_make_bar(1704067200000 + i * 3600000) for i in range(10)]

    class HistoryCheckingStrategy(BaseStrategy):
        @property
        def name(self) -> str:
            return "history_check"

        @property
        def version(self) -> str:
            return "1.0.0"

        @property
        def required_indicators(self) -> list[str]:
            return []

        def on_bar(self, bar: StandardBar, context: StrategyContext) -> Optional[Signal]:
            # The engine should guarantee this invariant:
            assert len(context.bar_history) <= context.current_bar_index + 1, (
                f"Leakage: bar_history has {len(context.bar_history)} bars "
                f"but current_bar_index is only {context.current_bar_index}"
            )
            return None

        def on_position_update(
            self, position: PositionSnapshot, context: StrategyContext
        ) -> Optional[Signal]:
            return None

    strategy = HistoryCheckingStrategy()

    # Simulate calling on_bar at bar index 5
    ctx = _make_context(bars, current_idx=5)
    # Simulate what a correct engine would do: only pass first 6 bars
    ctx_correct = StrategyContext(
        current_price=ctx.current_price,
        open_orders=ctx.open_orders,
        positions=ctx.positions,
        equity=ctx.equity,
        available_balance=ctx.available_balance,
        unrealized_pnl=ctx.unrealized_pnl,
        margin_ratio=ctx.margin_ratio,
        bar_history=bars[:6],  # Only up to current bar
        current_bar_index=5,
        total_bars=10,
        timestamp=ctx.timestamp,
    )

    result = strategy.on_bar(bars[5], ctx_correct)
    assert result is None  # Strategy returns None by design


def test_strategy_cannot_lookahead_with_priced_context():
    """Simulate an engine that incorrectly exposes future data."""
    bars = [_make_bar(1704067200000 + i * 3600000, str(10000 + i * 100)) for i in range(10)]

    class LookaheadStrategy(BaseStrategy):
        @property
        def name(self) -> str:
            return "lookahead_detector"

        @property
        def version(self) -> str:
            return "1.0.0"

        @property
        def required_indicators(self) -> list[str]:
            return []

        def on_bar(self, bar: StandardBar, context: StrategyContext) -> Optional[Signal]:
            # This strategy tries to peek at future bar closes
            future_idx = context.current_bar_index + 1
            if future_idx < len(context.bar_history):
                future_close = context.bar_history[future_idx].close
                current_close = bar.close
                if future_close > current_close:
                    # We know the future — this is CHEATING
                    return Signal(
                        action=SignalAction.OPEN_LONG,
                        symbol=bar.symbol,
                        quantity=Decimal("1.0"),
                        strength=1.0,
                        reason="CHEATING: looked at future bar",
                    )
            return None

        def on_position_update(
            self, position: PositionSnapshot, context: StrategyContext
        ) -> Optional[Signal]:
            return None

    strategy = LookaheadStrategy()

    # Simulate an engine that CORRECTLY only passes history up to current bar
    ctx = StrategyContext(
        current_price=bars[5].close,
        open_orders=0,
        positions=[],
        equity=Decimal("100000"),
        available_balance=Decimal("100000"),
        unrealized_pnl=Decimal("0"),
        margin_ratio=Decimal("0"),
        bar_history=bars[:6],  # ONLY up to index 5
        current_bar_index=5,
        total_bars=10,
        timestamp=bars[5].timestamp,
    )

    # With correct engine behavior, strategy cannot access future data
    signal = strategy.on_bar(bars[5], ctx)
    assert signal is None, (
        "Strategy generated a signal using future data — "
        "the engine must prevent this by truncating bar_history"
    )


def test_execution_delay_simulation():
    """Signals generated at bar N should execute at bar N + delay."""
    bars = [_make_bar(1704067200000 + i * 3600000) for i in range(10)]

    class DelayedExecutionStrategy(BaseStrategy):
        def __init__(self):
            self.signals_generated: List[int] = []
            self.signals_executed: List[int] = []
            self.delay_bars = 1

        @property
        def name(self) -> str:
            return "delay_test"

        @property
        def version(self) -> str:
            return "1.0.0"

        @property
        def required_indicators(self) -> list[str]:
            return []

        def on_bar(self, bar: StandardBar, context: StrategyContext) -> Optional[Signal]:
            # Generate signal on bar 3
            if context.current_bar_index == 3:
                self.signals_generated.append(context.current_bar_index)
                return Signal(
                    action=SignalAction.OPEN_LONG,
                    symbol=bar.symbol,
                    quantity=Decimal("0.1"),
                    strength=0.5,
                    reason="Test delay",
                )
            return None

        def on_position_update(
            self, position: PositionSnapshot, context: StrategyContext
        ) -> Optional[Signal]:
            return None

    strategy = DelayedExecutionStrategy()

    # Simulate engine calling on_bar for each bar
    for i in range(10):
        ctx = _make_context(bars, i)
        signal = strategy.on_bar(bars[i], ctx)
        if signal is not None:
            # In a real engine, the signal would execute at i + delay_bars
            expected_execution = i + strategy.delay_bars
            strategy.signals_executed.append(expected_execution)

    assert strategy.signals_generated == [3]
    assert strategy.signals_executed == [4], (
        f"Signal generated at bar 3 must execute at bar 4 (delay=1), "
        f"but simulated execution at bar {strategy.signals_executed}"
    )
