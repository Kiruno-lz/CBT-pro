"""RSI Momentum Reversal Strategy."""
from decimal import Decimal
from typing import List, Optional, Dict, Any

from cbt_pro.interfaces.strategy import BaseStrategy, Signal, SignalAction, StrategyContext, StandardBar
from cbt_pro.indicators.wrapper import compute_rsi

class RsiMomentumStrategy(BaseStrategy):
    """
    Mean-reversion strategy based on RSI divergences.

    Logic:
    - Open: RSI < 30 with bullish divergence -> OPEN_LONG; RSI > 70 with bearish -> OPEN_SHORT
    - Add: RSI returns to 50 then moves toward extreme again
    - Reduce: RSI enters 40-60 neutral zone, reduce 50%
    - Close: RSI crosses 50 in opposite direction
    """

    def __init__(self, period: int = 14, oversold: float = 30.0, overbought: float = 70.0):
        self.period = period
        self.oversold = oversold
        self.overbought = overbought
        self._prev_rsi: Optional[float] = None
        self._has_position = False
        self._direction: Optional[str] = None
        self._last_extreme: Optional[float] = None

    @property
    def name(self) -> str:
        return "RSI_Momentum"

    @property
    def version(self) -> str:
        return "1.0.0"

    @property
    def required_indicators(self) -> List[str]:
        return ["rsi_14"]

    def get_parameters(self) -> Dict[str, Any]:
        return {"period": self.period, "oversold": self.oversold, "overbought": self.overbought}

    def on_bar(self, bar: StandardBar, context: StrategyContext) -> Optional[Signal]:
        history = context.bar_history
        if len(history) < self.period + 5:
            return None

        closes = [b.close for b in history] + [bar.close]
        rsi_values = compute_rsi(closes, self.period)

        if len(rsi_values) < 3 or rsi_values[-1] is None:
            return None

        curr_rsi = rsi_values[-1]
        prev_rsi = rsi_values[-2]
        if prev_rsi is None:
            return None

        # Divergence detection: price makes new low but RSI doesn't
        def detect_divergence(window_size: int = 5) -> tuple[bool, float]:
            if len(closes) < window_size + 2 or len(rsi_values) < window_size + 2:
                return False, 0.0
            price_lows = [float(c) for c in closes[-window_size:]]
            rsi_vals = [r for r in rsi_values[-window_size:] if r is not None]
            if len(rsi_vals) < window_size:
                return False, 0.0
            # Bullish: price lower low, RSI higher low
            price_ll = min(price_lows)
            price_ll_idx = price_lows.index(price_ll)
            if price_ll_idx > 0 and price_ll_idx < len(rsi_vals):
                rsi_at_ll = rsi_vals[price_ll_idx]
                prev_rsi_ll = rsi_vals[price_ll_idx - 1]
                if price_ll < float(closes[-window_size-1]) and rsi_at_ll > prev_rsi_ll:
                    return True, abs(rsi_at_ll - prev_rsi_ll)
            return False, 0.0

        # Bullish setup
        if curr_rsi < self.oversold:
            div, div_strength = detect_divergence()
            strength = min(0.5 + (self.oversold - curr_rsi) / self.oversold * 0.3 + div_strength * 0.02, 1.0)
            reason = f"RSI oversold ({curr_rsi:.1f})"
            if div:
                reason += " with bullish divergence"
            self._has_position = True
            self._direction = "Long"
            self._last_extreme = curr_rsi
            return Signal(
                action=SignalAction.OPEN_LONG,
                symbol=bar.symbol,
                quantity=context.equity * Decimal("0.08") / bar.close,
                strength=strength,
                reason=reason,
            )

        # Bearish setup
        if curr_rsi > self.overbought:
            div, div_strength = detect_divergence()
            strength = min(0.5 + (curr_rsi - self.overbought) / (100.0 - self.overbought) * 0.3 + div_strength * 0.02, 1.0)
            reason = f"RSI overbought ({curr_rsi:.1f})"
            if div:
                reason += " with bearish divergence"
            self._has_position = True
            self._direction = "Short"
            self._last_extreme = curr_rsi
            return Signal(
                action=SignalAction.OPEN_SHORT,
                symbol=bar.symbol,
                quantity=context.equity * Decimal("0.08") / bar.close,
                strength=strength,
                reason=reason,
            )

        # Add logic: RSI returns to 50 then moves back
        if self._has_position and self._last_extreme is not None:
            if (self._direction == "Long" and curr_rsi > 45 and prev_rsi <= 45 and curr_rsi < self.oversold + 10) or \
               (self._direction == "Short" and curr_rsi < 55 and prev_rsi >= 55 and curr_rsi > self.overbought - 10):
                return Signal(
                    action=SignalAction.ADD_LONG if self._direction == "Long" else SignalAction.ADD_SHORT,
                    symbol=bar.symbol,
                    quantity=context.equity * Decimal("0.04") / bar.close,
                    strength=0.6,
                    reason=f"RSI returned to 50 then moved toward extreme ({curr_rsi:.1f})",
                )

        # Reduce logic: neutral zone
        if self._has_position and 40.0 <= curr_rsi <= 60.0:
            return Signal(
                action=SignalAction.REDUCE_LONG if self._direction == "Long" else SignalAction.REDUCE_SHORT,
                symbol=bar.symbol,
                quantity=context.equity * Decimal("0.05") / bar.close,
                strength=0.4,
                reason=f"RSI in neutral zone ({curr_rsi:.1f}), taking partial profits",
            )

        # Close logic: cross 50 opposite
        if self._has_position:
            if (self._direction == "Long" and prev_rsi > 50.0 and curr_rsi <= 50.0) or \
               (self._direction == "Short" and prev_rsi < 50.0 and curr_rsi >= 50.0):
                self._has_position = False
                self._direction = None
                return Signal(
                    action=SignalAction.CLOSE_ALL,
                    symbol=bar.symbol,
                    quantity=context.equity * Decimal("0.1") / bar.close,
                    strength=0.7,
                    reason=f"RSI crossed 50 opposite direction ({curr_rsi:.1f})",
                )

        return None

    def on_position_update(self, position, context) -> Optional[Signal]:
        if position.current_size == 0:
            self._has_position = False
            self._direction = None
        return None
