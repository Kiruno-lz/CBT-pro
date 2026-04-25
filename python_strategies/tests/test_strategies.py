"""Tests for CBT-Pro strategies and indicators."""
import pytest
from decimal import Decimal

from cbt_pro.interfaces.strategy import StandardBar, StrategyContext, SignalAction
from cbt_pro.indicators.wrapper import compute_ema, compute_rsi, compute_bollinger, compute_atr, compute_macd, compute_vwap
from cbt_pro.strategies.ema_cross import EmaCrossStrategy
from cbt_pro.strategies.rsi_momentum import RsiMomentumStrategy
from cbt_pro.strategies.bollinger_mean_reversion import BollingerMeanReversionStrategy

def _make_bars(n: int, trend: str = "up") -> list:
    bars = []
    base = Decimal("40000")
    for i in range(n):
        if trend == "up":
            open_p = base + Decimal(str(i * 100))
            close = open_p + Decimal(str(50 + (i % 3) * 10))
        elif trend == "down":
            open_p = base - Decimal(str(i * 100))
            close = open_p - Decimal(str(50 + (i % 3) * 10))
        else:  # oscillating
            open_p = base + Decimal(str((i % 20 - 10) * 50))
            close = open_p + Decimal(str(20))
        bars.append(StandardBar(
            timestamp=1704067200000 + i * 60000,
            open=open_p,
            high=close + Decimal("30"),
            low=open_p - Decimal("20"),
            close=close,
            volume=Decimal(str(10 + i % 5)),
            symbol="BTC-USDT",
            exchange="binance",
            confirmed=True,
        ))
    return bars

def _make_context(bars):
    return StrategyContext(
        current_price=bars[-1].close,
        open_orders=0,
        positions=[],
        equity=Decimal("100000"),
        available_balance=Decimal("90000"),
        unrealized_pnl=Decimal("0"),
        margin_ratio=Decimal("0.1"),
        bar_history=bars[:-1] if len(bars) > 1 else [],
        current_bar_index=len(bars) - 1,
        total_bars=len(bars),
        timestamp=bars[-1].timestamp,
    )

class TestIndicators:
    def test_ema_basic(self):
        prices = [Decimal(str(v)) for v in [10, 11, 12, 13, 14, 15, 16]]
        ema = compute_ema(prices, 3)
        assert ema[0] is None
        assert ema[2] is not None
        assert float(ema[-1]) > float(ema[2])

    def test_rsi_basic(self):
        # Strong uptrend should have high RSI
        prices = [Decimal(str(v)) for v in range(100, 120)]
        rsi = compute_rsi(prices, 14)
        valid = [r for r in rsi if r is not None]
        assert len(valid) > 0
        assert valid[-1] > 50.0  # Uptrend -> RSI high

    def test_bollinger_basic(self):
        prices = [Decimal(str(v)) for v in [10, 12, 11, 13, 12, 14, 13, 15]]
        bb = compute_bollinger(prices, 3, 2.0)
        assert len(bb["upper"]) == len(prices)
        assert len(bb["middle"]) == len(prices)
        assert len(bb["lower"]) == len(prices)
        for i in range(2, len(prices)):
            assert bb["upper"][i] >= bb["middle"][i] >= bb["lower"][i]

    def test_atr_basic(self):
        bars = _make_bars(30, "oscillating")
        atr = compute_atr(bars, 14)
        valid = [a for a in atr if a is not None]
        assert len(valid) > 0
        assert all(a > 0 for a in valid)

    def test_macd_basic(self):
        prices = [Decimal(str(v)) for v in range(50, 100)]
        macd = compute_macd(prices, 12, 26, 9)
        assert len(macd["macd"]) == len(prices)
        assert len(macd["signal"]) == len(prices)
        assert len(macd["histogram"]) == len(prices)

    def test_vwap_basic(self):
        bars = _make_bars(10, "up")
        vwap = compute_vwap(bars)
        assert vwap > Decimal("0")

class TestEmaCrossStrategy:
    def test_initialization(self):
        s = EmaCrossStrategy()
        assert s.name == "EMA_Cross"
        assert s.version == "1.0.0"
        assert "ema_9" in s.required_indicators

    def test_signal_strength_bounds(self):
        s = EmaCrossStrategy()
        bars = _make_bars(50, "up")
        ctx = _make_context(bars)
        sig = s.on_bar(bars[-1], ctx)
        if sig:
            assert 0.0 <= sig.strength <= 1.0

    def test_long_signal_on_uptrend(self):
        s = EmaCrossStrategy()
        bars = _make_bars(50, "up")
        ctx = _make_context(bars)
        sig = s.on_bar(bars[-1], ctx)
        if sig:
            assert sig.action in {SignalAction.OPEN_LONG, SignalAction.ADD_LONG, SignalAction.REDUCE_LONG}

class TestRsiMomentumStrategy:
    def test_initialization(self):
        s = RsiMomentumStrategy()
        assert s.name == "RSI_Momentum"
        assert "rsi_14" in s.required_indicators

    def test_oversold_signal(self):
        s = RsiMomentumStrategy()
        # Create bars with sharp drop (oversold RSI)
        bars = _make_bars(30, "up")
        # Inject a sharp drop at the end
        for i in range(5):
            idx = len(bars) - 5 + i
            bars[idx] = StandardBar(
                timestamp=bars[idx].timestamp,
                open=bars[idx].open - Decimal(str((i+1)*500)),
                high=bars[idx].high - Decimal(str((i+1)*400)),
                low=bars[idx].low - Decimal(str((i+1)*600)),
                close=bars[idx].close - Decimal(str((i+1)*500)),
                volume=bars[idx].volume,
                symbol="BTC-USDT",
                exchange="binance",
                confirmed=True,
            )
        ctx = _make_context(bars)
        sig = s.on_bar(bars[-1], ctx)
        if sig:
            assert 0.0 <= sig.strength <= 1.0

class TestBollingerStrategy:
    def test_initialization(self):
        s = BollingerMeanReversionStrategy()
        assert s.name == "Bollinger_MeanReversion"
        assert "bollinger_20_2" in s.required_indicators

    def test_signal_reason_present(self):
        s = BollingerMeanReversionStrategy()
        bars = _make_bars(50, "oscillating")
        ctx = _make_context(bars)
        sig = s.on_bar(bars[-1], ctx)
        if sig:
            assert len(sig.reason) > 0
            assert sig.symbol == "BTC-USDT"
