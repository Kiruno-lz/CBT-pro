"""Tests for BaseStrategy ABC and interface contracts."""

import pytest
from decimal import Decimal
from typing import Optional

from cbt_pro.interfaces.strategy import (
    BaseStrategy,
    Signal,
    SignalAction,
    StrategyContext,
    PositionSnapshot,
    StandardBar,
)


def test_base_strategy_cannot_be_instantiated():
    """BaseStrategy is abstract and cannot be instantiated directly."""
    with pytest.raises(TypeError):
        BaseStrategy()


def test_base_strategy_requires_name():
    """Concrete strategy must implement the name property."""
    class IncompleteStrategy(BaseStrategy):
        @property
        def version(self) -> str:
            return "1.0.0"

        @property
        def required_indicators(self) -> list[str]:
            return []

        def on_bar(self, bar: StandardBar, context: StrategyContext) -> Optional[Signal]:
            return None

        def on_position_update(
            self, position: PositionSnapshot, context: StrategyContext
        ) -> Optional[Signal]:
            return None

    with pytest.raises(TypeError):
        IncompleteStrategy()


def test_base_strategy_requires_version():
    """Concrete strategy must implement the version property."""
    class IncompleteStrategy(BaseStrategy):
        @property
        def name(self) -> str:
            return "incomplete"

        @property
        def required_indicators(self) -> list[str]:
            return []

        def on_bar(self, bar: StandardBar, context: StrategyContext) -> Optional[Signal]:
            return None

        def on_position_update(
            self, position: PositionSnapshot, context: StrategyContext
        ) -> Optional[Signal]:
            return None

    with pytest.raises(TypeError):
        IncompleteStrategy()


def test_base_strategy_requires_on_bar():
    """Concrete strategy must implement on_bar method."""
    class IncompleteStrategy(BaseStrategy):
        @property
        def name(self) -> str:
            return "incomplete"

        @property
        def version(self) -> str:
            return "1.0.0"

        @property
        def required_indicators(self) -> list[str]:
            return []

        def on_position_update(
            self, position: PositionSnapshot, context: StrategyContext
        ) -> Optional[Signal]:
            return None

    with pytest.raises(TypeError):
        IncompleteStrategy()


def test_valid_strategy_can_be_instantiated():
    """A fully implemented strategy can be instantiated."""
    class ValidStrategy(BaseStrategy):
        @property
        def name(self) -> str:
            return "valid_test"

        @property
        def version(self) -> str:
            return "1.0.0"

        @property
        def required_indicators(self) -> list[str]:
            return ["ema_9", "ema_21"]

        def on_bar(self, bar: StandardBar, context: StrategyContext) -> Optional[Signal]:
            return Signal(
                action=SignalAction.OPEN_LONG,
                symbol=bar.symbol,
                quantity=Decimal("0.1"),
                strength=0.8,
                reason="Test signal",
            )

        def on_position_update(
            self, position: PositionSnapshot, context: StrategyContext
        ) -> Optional[Signal]:
            return None

        def get_parameters(self) -> dict:
            return {"ema_fast": 9, "ema_slow": 21}

    strategy = ValidStrategy()
    assert strategy.name == "valid_test"
    assert strategy.version == "1.0.0"
    assert strategy.required_indicators == ["ema_9", "ema_21"]
    assert strategy.get_parameters() == {"ema_fast": 9, "ema_slow": 21}


def test_strategy_on_bar_returns_signal_or_none():
    """on_bar must return either a Signal or None."""
    class SignalStrategy(BaseStrategy):
        @property
        def name(self) -> str:
            return "signal_test"

        @property
        def version(self) -> str:
            return "1.0.0"

        @property
        def required_indicators(self) -> list[str]:
            return []

        def on_bar(self, bar: StandardBar, context: StrategyContext) -> Optional[Signal]:
            return Signal(
                action=SignalAction.OPEN_LONG,
                symbol=bar.symbol,
                quantity=Decimal("0.1"),
                strength=0.8,
                reason="Test",
            )

        def on_position_update(
            self, position: PositionSnapshot, context: StrategyContext
        ) -> Optional[Signal]:
            return None

    strategy = SignalStrategy()
    bar = StandardBar(
        timestamp=1704067200000,
        open=Decimal("10000"),
        high=Decimal("10100"),
        low=Decimal("9900"),
        close=Decimal("10050"),
        volume=Decimal("1.0"),
        symbol="BTC-USDT",
        exchange="test",
    )
    ctx = StrategyContext(
        current_price=Decimal("10050"),
        open_orders=0,
        positions=[],
        equity=Decimal("100000"),
        available_balance=Decimal("100000"),
        unrealized_pnl=Decimal("0"),
        margin_ratio=Decimal("0"),
        bar_history=[bar],
        current_bar_index=0,
        total_bars=100,
        timestamp=bar.timestamp,
    )

    signal = strategy.on_bar(bar, ctx)
    assert signal is not None
    assert signal.action == SignalAction.OPEN_LONG
    assert signal.symbol == "BTC-USDT"
    assert signal.quantity == Decimal("0.1")
