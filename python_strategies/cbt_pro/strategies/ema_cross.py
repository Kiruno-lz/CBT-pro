"""EMA Crossover Trend Following Strategy."""
from decimal import Decimal
from typing import List, Optional, Dict, Any

from cbt_pro.interfaces.strategy import BaseStrategy, Signal, SignalAction, StrategyContext, StandardBar
from cbt_pro.indicators.wrapper import compute_ema

class EmaCrossStrategy(BaseStrategy):
    """
    Trend-following strategy based on EMA(9) and EMA(21) crossovers.

    Logic:
    - Open: EMA(9) crosses above EMA(21) -> OPEN_LONG; below -> OPEN_SHORT
    - Add: Pullback to EMA(9) without breaking EMA(21), add 50%
    - Reduce: EMA(9) slope < threshold, reduce 30%
    - Close: EMA(9) crosses EMA(21) in opposite direction
    """

    def __init__(self, fast_period: int = 9, slow_period: int = 21, 
                 add_threshold: float = 0.001, reduce_slope: float = 0.0005):
        self.fast_period = fast_period
        self.slow_period = slow_period
        self.add_threshold = Decimal(str(add_threshold))
        self.reduce_slope = Decimal(str(reduce_slope))
        self._prev_fast: Optional[Decimal] = None
        self._prev_slow: Optional[Decimal] = None
        self._has_position = False
        self._direction: Optional[str] = None

    @property
    def name(self) -> str:
        return "EMA_Cross"

    @property
    def version(self) -> str:
        return "1.0.0"

    @property
    def required_indicators(self) -> List[str]:
        return ["ema_9", "ema_21"]

    def get_parameters(self) -> Dict[str, Any]:
        return {
            "fast_period": self.fast_period,
            "slow_period": self.slow_period,
            "add_threshold": float(self.add_threshold),
            "reduce_slope": float(self.reduce_slope),
        }

    def on_bar(self, bar: StandardBar, context: StrategyContext) -> Optional[Signal]:
        history = context.bar_history
        if len(history) < self.slow_period + 2:
            return None

        closes = [b.close for b in history] + [bar.close]
        ema_fast = compute_ema(closes, self.fast_period)
        ema_slow = compute_ema(closes, self.slow_period)

        if len(ema_fast) < 2 or ema_fast[-1] is None or ema_slow[-1] is None:
            return None

        curr_fast = ema_fast[-1]
        curr_slow = ema_slow[-1]
        prev_fast = ema_fast[-2]
        prev_slow = ema_slow[-2]

        if prev_fast is None or prev_slow is None:
            return None

        # Cross detection
        crossed_above = prev_fast <= prev_slow and curr_fast > curr_slow
        crossed_below = prev_fast >= prev_slow and curr_fast < curr_slow

        volume_factor = float(bar.volume) / 100.0 if bar.volume > 0 else 1.0
        cross_angle = abs(float(curr_fast - curr_slow)) / max(float(curr_slow), 1e-9)
        strength = min(0.3 + cross_angle * 10.0 + min(volume_factor * 0.1, 0.3), 1.0)

        if crossed_above:
            self._has_position = True
            self._direction = "Long"
            return Signal(
                action=SignalAction.OPEN_LONG,
                symbol=bar.symbol,
                quantity=context.equity * Decimal("0.1") / bar.close,
                strength=strength,
                reason=f"EMA({self.fast_period}) crossed above EMA({self.slow_period}), volume_factor={volume_factor:.2f}",
            )

        if crossed_below:
            self._has_position = True
            self._direction = "Short"
            return Signal(
                action=SignalAction.OPEN_SHORT,
                symbol=bar.symbol,
                quantity=context.equity * Decimal("0.1") / bar.close,
                strength=strength,
                reason=f"EMA({self.fast_period}) crossed below EMA({self.slow_period}), volume_factor={volume_factor:.2f}",
            )

        # Add logic: pullback to EMA(9) without breaking EMA(21)
        if self._has_position and self._direction == "Long":
            diff = abs(float(bar.close - curr_fast)) / max(float(curr_fast), 1e-9)
            if diff < float(self.add_threshold) and bar.close > curr_slow:
                return Signal(
                    action=SignalAction.ADD_LONG,
                    symbol=bar.symbol,
                    quantity=context.equity * Decimal("0.05") / bar.close,
                    strength=strength * 0.8,
                    reason="Pullback to EMA(9) within threshold, trend intact",
                )

        # Reduce logic: flattening slope
        if self._has_position and len(ema_fast) >= 3:
            prev2_fast = ema_fast[-3]
            if prev2_fast is not None:
                slope1 = float(curr_fast - prev_fast)
                slope2 = float(prev_fast - prev2_fast)
                if abs(slope1) < float(self.reduce_slope) and abs(slope1) < abs(slope2):
                    return Signal(
                        action=SignalAction.REDUCE_LONG if self._direction == "Long" else SignalAction.REDUCE_SHORT,
                        symbol=bar.symbol,
                        quantity=context.equity * Decimal("0.03") / bar.close,
                        strength=0.5,
                        reason=f"EMA(9) slope flattening: |slope|={abs(slope1):.6f}",
                    )

        return None

    def on_position_update(self, position, context) -> Optional[Signal]:
        if position.current_size == 0:
            self._has_position = False
            self._direction = None
        return None
