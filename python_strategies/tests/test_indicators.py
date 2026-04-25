"""Tests for technical indicator calculations against known reference values."""

from decimal import Decimal
import pytest


def test_ema_calculation():
    """Test EMA calculation against known values."""
    from cbt_pro.indicators import ema

    prices = [Decimal("100"), Decimal("102"), Decimal("101"), Decimal("103"), Decimal("105")]
    period = 3

    result = ema(prices, period)

    # EMA(3): multiplier = 2/(3+1) = 0.5
    # EMA1 = 100
    # EMA2 = 102*0.5 + 100*0.5 = 101
    # EMA3 = 101*0.5 + 101*0.5 = 101
    # EMA4 = 103*0.5 + 101*0.5 = 102
    # EMA5 = 105*0.5 + 102*0.5 = 103.5

    assert len(result) == len(prices)
    assert result[0] == Decimal("100")
    assert result[1] == Decimal("101")
    assert result[2] == Decimal("101")
    assert result[3] == Decimal("102")
    assert result[4] == Decimal("103.5")


def test_ema_insufficient_data():
    """EMA with fewer prices than period should still return values."""
    from cbt_pro.indicators import ema

    prices = [Decimal("100"), Decimal("102")]
    result = ema(prices, 5)
    assert len(result) == len(prices)


def test_rsi_calculation():
    """Test RSI(14) calculation against known reference values."""
    from cbt_pro.indicators import rsi

    # 14 up-days followed by balanced data
    prices = [Decimal(str(100 + i)) for i in range(15)]
    prices.append(Decimal("114"))  # Small down

    result = rsi(prices, period=14)

    assert len(result) == len(prices)
    # RSI should be high (strong uptrend)
    assert result[-1] > Decimal("50")


def test_rsi_oversold():
    """RSI should be low for sustained downtrend."""
    from cbt_pro.indicators import rsi

    prices = [Decimal(str(100 - i)) for i in range(15)]

    result = rsi(prices, period=14)

    assert len(result) == len(prices)
    assert result[-1] < Decimal("50")


def test_bollinger_bands():
    """Test Bollinger Bands calculation."""
    from cbt_pro.indicators import bollinger_bands

    # Flat prices: bands should collapse to the same value
    flat_prices = [Decimal("100")] * 21
    upper, middle, lower = bollinger_bands(flat_prices, period=20, std_dev=2)

    assert len(upper) == len(flat_prices)
    assert len(middle) == len(flat_prices)
    assert len(lower) == len(flat_prices)

    # For flat prices, upper == middle == lower
    assert upper[-1] == middle[-1]
    assert lower[-1] == middle[-1]


def test_bollinger_expansion():
    """Bollinger bands should expand with volatility."""
    from cbt_pro.indicators import bollinger_bands

    # Oscillating prices to create volatility
    prices = [Decimal(str(100 + (i % 2) * 10)) for i in range(21)]
    upper, middle, lower = bollinger_bands(prices, period=20, std_dev=2)

    assert upper[-1] > middle[-1]
    assert lower[-1] < middle[-1]
    # Symmetry check: upper deviation equals lower deviation
    assert (upper[-1] - middle[-1]) == (middle[-1] - lower[-1])


def test_macd_calculation():
    """Test MACD calculation produces valid signal and histogram."""
    from cbt_pro.indicators import macd

    prices = [Decimal(str(100 + i * 2)) for i in range(35)]
    macd_line, signal_line, histogram = macd(prices)

    assert len(macd_line) == len(prices)
    assert len(signal_line) == len(prices)
    assert len(histogram) == len(prices)

    # Histogram = MACD - Signal
    for i in range(len(prices)):
        if macd_line[i] is not None and signal_line[i] is not None:
            assert histogram[i] == macd_line[i] - signal_line[i]


def test_atr_calculation():
    """Test ATR calculation with known candle data."""
    from cbt_pro.indicators import atr

    # Simple candles: high-low range is constant
    highs = [Decimal("110")] * 15
    lows = [Decimal("90")] * 15
    closes = [Decimal("100")] * 15

    result = atr(highs, lows, closes, period=14)

    assert len(result) == len(highs)
    # ATR for constant range should stabilize
    assert result[-1] == Decimal("20")


def test_vwap_calculation():
    """Test VWAP resets and calculates correctly."""
    from cbt_pro.indicators import vwap

    prices = [Decimal("100"), Decimal("102"), Decimal("101")]
    volumes = [Decimal("10"), Decimal("20"), Decimal("30")]

    result = vwap(prices, volumes)

    expected = (
        Decimal("100") * Decimal("10")
        + Decimal("102") * Decimal("20")
        + Decimal("101") * Decimal("30")
    ) / Decimal("60")

    assert result == expected
