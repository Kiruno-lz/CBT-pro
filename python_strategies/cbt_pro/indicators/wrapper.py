"""Pure-Python technical indicators for CBT-Pro strategies."""
import math
from decimal import Decimal
from typing import List, Optional, Dict

from cbt_pro.interfaces.strategy import StandardBar

def _to_floats(values: List[Decimal]) -> List[float]:
    return [float(v) for v in values]

def _from_float(value: Optional[float]) -> Optional[Decimal]:
    return None if value is None or math.isnan(value) else Decimal(str(value))

def compute_ema(prices: List[Decimal], period: int) -> List[Optional[Decimal]]:
    """Compute Exponential Moving Average."""
    if len(prices) < period:
        return [None] * len(prices)
    floats = _to_floats(prices)
    multiplier = 2.0 / (period + 1)
    ema = [None] * (period - 1)
    ema.append(sum(floats[:period]) / period)
    for i in range(period, len(floats)):
        ema.append((floats[i] - ema[-1]) * multiplier + ema[-1])
    return [_from_float(v) for v in ema]

def compute_rsi(prices: List[Decimal], period: int = 14) -> List[Optional[float]]:
    """Compute RSI(period). Returns float values 0-100."""
    if len(prices) < period + 1:
        return [None] * len(prices)
    floats = _to_floats(prices)
    deltas = [floats[i] - floats[i-1] for i in range(1, len(floats))]
    gains = [d if d > 0 else 0.0 for d in deltas]
    losses = [abs(d) if d < 0 else 0.0 for d in deltas]
    avg_gain = sum(gains[:period]) / period
    avg_loss = sum(losses[:period]) / period
    rsi: List[Optional[float]] = [None] * period
    for i in range(period, len(deltas)):
        avg_gain = (avg_gain * (period - 1) + gains[i]) / period
        avg_loss = (avg_loss * (period - 1) + losses[i]) / period
        if avg_loss == 0:
            rsi.append(100.0)
        else:
            rs = avg_gain / avg_loss
            rsi.append(100.0 - (100.0 / (1.0 + rs)))
    return rsi

def compute_bollinger(prices: List[Decimal], period: int = 20, std_dev: float = 2.0) -> Dict[str, List[Optional[Decimal]]]:
    """Compute Bollinger Bands: upper, middle (SMA), lower."""
    floats = _to_floats(prices)
    upper: List[Optional[Decimal]] = [None] * (period - 1)
    middle: List[Optional[Decimal]] = [None] * (period - 1)
    lower: List[Optional[Decimal]] = [None] * (period - 1)
    for i in range(period - 1, len(floats)):
        window = floats[i-period+1:i+1]
        sma = sum(window) / period
        variance = sum((x - sma) ** 2 for x in window) / period
        sd = math.sqrt(variance)
        middle.append(_from_float(sma))
        upper.append(_from_float(sma + std_dev * sd))
        lower.append(_from_float(sma - std_dev * sd))
    return {"upper": upper, "middle": middle, "lower": lower}

def compute_atr(bars: List[StandardBar], period: int = 14) -> List[Optional[Decimal]]:
    """Compute Average True Range."""
    if len(bars) < 2:
        return [None] * len(bars)
    trs = []
    for i in range(len(bars)):
        if i == 0:
            trs.append(float(bars[i].high - bars[i].low))
        else:
            tr1 = float(bars[i].high - bars[i].low)
            tr2 = abs(float(bars[i].high - bars[i-1].close))
            tr3 = abs(float(bars[i].low - bars[i-1].close))
            trs.append(max(tr1, tr2, tr3))
    if len(trs) < period:
        return [None] * len(bars)
    atr = [None] * (period - 1)
    atr.append(sum(trs[:period]) / period)
    for i in range(period, len(trs)):
        atr.append((atr[-1] * (period - 1) + trs[i]) / period)
    return [_from_float(v) for v in atr]

def compute_macd(prices: List[Decimal], fast: int = 12, slow: int = 26, signal: int = 9) -> Dict[str, List[Optional[Decimal]]]:
    """Compute MACD line, Signal line, and Histogram."""
    ema_fast = compute_ema(prices, fast)
    ema_slow = compute_ema(prices, slow)
    macd_line: List[Optional[Decimal]] = []
    for f, s in zip(ema_fast, ema_slow):
        if f is None or s is None:
            macd_line.append(None)
        else:
            macd_line.append(f - s)
    # Remove None padding to compute signal EMA
    clean_macd = [m for m in macd_line if m is not None]
    clean_macd_floats = [float(m) for m in clean_macd]
    if len(clean_macd_floats) < signal:
        return {"macd": macd_line, "signal": [None]*len(macd_line), "histogram": [None]*len(macd_line)}
    sig_ema = compute_ema([Decimal(str(v)) for v in clean_macd_floats], signal)
    # Re-align with original padding
    padding = len(macd_line) - len(sig_ema)
    signal_line = [None] * padding + [_from_float(v) for v in sig_ema]
    histogram = []
    for m, s in zip(macd_line, signal_line):
        if m is None or s is None:
            histogram.append(None)
        else:
            histogram.append(m - s)
    return {"macd": macd_line, "signal": signal_line, "histogram": histogram}

def compute_vwap(bars: List[StandardBar]) -> Decimal:
    """Compute Volume-Weighted Average Price for intraday bars."""
    total_pv = Decimal("0")
    total_vol = Decimal("0")
    for bar in bars:
        typical = (bar.high + bar.low + bar.close) / Decimal("3")
        total_pv += typical * bar.volume
        total_vol += bar.volume
    if total_vol == 0:
        return Decimal("0")
    return total_pv / total_vol
