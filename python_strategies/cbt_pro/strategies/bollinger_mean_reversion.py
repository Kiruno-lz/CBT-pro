"""Bollinger Band Mean Reversion Strategy."""
from decimal import Decimal
from typing import List, Optional, Dict, Any

from cbt_pro.interfaces.strategy import BaseStrategy, Signal, SignalAction, StrategyContext, StandardBar
from cbt_pro.indicators.wrapper import compute_bollinger, compute_rsi, compute_atr

class BollingerMeanReversionStrategy(BaseStrategy):
    """
    Mean-reversion strategy using Bollinger Bands with RSI confirmation.

    Logic:
    - Open: Price touches lower band + RSI not making new low -> OPEN_LONG
            Price touches upper band + RSI not making new high -> OPEN_SHORT
    - Add: Price extends 1 ATR beyond band, pyramid (decreasing size)
    - Reduce: Price returns to middle band, reduce 50%
    - Close: Price crosses middle band and moves 0.5 ATR opposite
    """

    def __init__(self, bb_period: int = 20, bb_std: float = 2.0, rsi_period: int = 14):
        self.bb_period = bb_period
        self.bb_std = bb_std
        self.rsi_period = rsi_period
        self._has_position = False
        self._direction: Optional[str] = None
        self._entry_price: Optional[Decimal] = None

    @property
    def name(self) -> str:
        return "Bollinger_MeanReversion"

    @property
    def version(self) -> str:
        return "1.0.0"

    @property
    def required_indicators(self) -> List[str]:
        return ["bollinger_20_2", "rsi_14", "atr_14"]

    def get_parameters(self) -> Dict[str, Any]:
        return {"bb_period": self.bb_period, "bb_std": self.bb_std, "rsi_period": self.rsi_period}

    def on_bar(self, bar: StandardBar, context: StrategyContext) -> Optional[Signal]:
        history = context.bar_history
        if len(history) < self.bb_period + 5:
            return None

        closes = [b.close for b in history] + [bar.close]
        bb = compute_bollinger(closes, self.bb_period, self.bb_std)
        rsi_vals = compute_rsi(closes, self.rsi_period)
        atr_vals = compute_atr(history + [bar], self.rsi_period)

        if len(bb["upper"]) < 2 or bb["upper"][-1] is None or bb["lower"][-1] is None:
            return None

        upper = bb["upper"][-1]
        middle = bb["middle"][-1]
        lower = bb["lower"][-1]
        curr_rsi = rsi_vals[-1] if rsi_values and rsi_vals[-1] is not None else 50.0
        curr_atr = atr_vals[-1] if atr_vals and atr_vals[-1] is not None else Decimal("0")

        if upper is None or middle is None or lower is None:
            return None

        # RSI non-confirmation for trend continuation prevention
        def rsi_not_confirms_trend() -> bool:
            if len(rsi_vals) < 5 or len(closes) < 5:
                return True
            recent_closes = [float(c) for c in closes[-5:]]
            recent_rsi = [r for r in rsi_vals[-5:] if r is not None]
            if len(recent_rsi) < 5:
                return True
            # For long: price at lower band, check RSI not making new low
            if bar.close <= lower:
                price_new_low = recent_closes[-1] < min(recent_closes[:-1])
                rsi_new_low = recent_rsi[-1] < min(recent_rsi[:-1])
                return not (price_new_low and rsi_new_low)  # avoid both making new lows
            # For short: price at upper band, check RSI not making new high
            if bar.close >= upper:
                price_new_high = recent_closes[-1] > max(recent_closes[:-1])
                rsi_new_high = recent_rsi[-1] > max(recent_rsi[:-1])
                return not (price_new_high and rsi_new_high)
            return True

        # Calculate bandwidth for signal strength
        bandwidth = float((upper - lower) / middle) if middle > 0 else 0.0
        dist_from_band = float(bar.close - lower) / max(float(upper - lower), 1e-9)

        # Open Long
        if bar.close <= lower and rsi_not_confirms_trend():
            strength = min(0.5 + (1.0 - dist_from_band) * 0.3 + bandwidth * 2.0, 1.0)
            self._has_position = True
            self._direction = "Long"
            self._entry_price = bar.close
            return Signal(
                action=SignalAction.OPEN_LONG,
                symbol=bar.symbol,
                quantity=context.equity * Decimal("0.08") / bar.close,
                strength=strength,
                reason=f"Price touched lower Bollinger band ({float(lower):.2f}), bandwidth={bandwidth:.4f}",
            )

        # Open Short
        if bar.close >= upper and rsi_not_confirms_trend():
            strength = min(0.5 + dist_from_band * 0.3 + bandwidth * 2.0, 1.0)
            self._has_position = True
            self._direction = "Short"
            self._entry_price = bar.close
            return Signal(
                action=SignalAction.OPEN_SHORT,
                symbol=bar.symbol,
                quantity=context.equity * Decimal("0.08") / bar.close,
                strength=strength,
                reason=f"Price touched upper Bollinger band ({float(upper):.2f}), bandwidth={bandwidth:.4f}",
            )

        # Add logic: extends 1 ATR beyond band
        if self._has_position and curr_atr > 0:
            if (self._direction == "Long" and bar.close < lower - curr_atr) or \
               (self._direction == "Short" and bar.close > upper + curr_atr):
                # Pyramid: decreasing size
                add_qty = context.equity * Decimal("0.03") / bar.close
                return Signal(
                    action=SignalAction.ADD_LONG if self._direction == "Long" else SignalAction.ADD_SHORT,
                    symbol=bar.symbol,
                    quantity=add_qty,
                    strength=0.5,
                    reason=f"Price extended 1 ATR beyond Bollinger band, pyramiding",
                )

        # Reduce logic: returns to middle band
        if self._has_position and middle > 0:
            if abs(float(bar.close - middle)) / float(middle) < 0.005:
                return Signal(
                    action=SignalAction.REDUCE_LONG if self._direction == "Long" else SignalAction.REDUCE_SHORT,
                    symbol=bar.symbol,
                    quantity=context.equity * Decimal("0.05") / bar.close,
                    strength=0.6,
                    reason=f"Price returned to Bollinger middle band ({float(middle):.2f})",
                )

        # Close logic: crosses middle and moves 0.5 ATR opposite
        if self._has_position and curr_atr > 0 and middle > 0:
            half_atr = curr_atr / Decimal("2")
            if self._direction == "Long" and bar.close > middle + half_atr:
                self._has_position = False
                self._direction = None
                return Signal(
                    action=SignalAction.CLOSE_ALL,
                    symbol=bar.symbol,
                    quantity=context.equity * Decimal("0.1") / bar.close,
                    strength=0.7,
                    reason=f"Price crossed middle band and moved 0.5 ATR higher",
                )
            if self._direction == "Short" and bar.close < middle - half_atr:
                self._has_position = False
                self._direction = None
                return Signal(
                    action=SignalAction.CLOSE_ALL,
                    symbol=bar.symbol,
                    quantity=context.equity * Decimal("0.1") / bar.close,
                    strength=0.7,
                    reason=f"Price crossed middle band and moved 0.5 ATR lower",
                )

        return None

    def on_position_update(self, position, context) -> Optional[Signal]:
        if position.current_size == 0:
            self._has_position = False
            self._direction = None
            self._entry_price = None
        return None
