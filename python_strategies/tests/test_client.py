"""Tests for BacktestClient HTTP SDK with mocked responses."""

import pytest
from decimal import Decimal
from unittest.mock import AsyncMock, patch, MagicMock
import aiohttp

from cbt_pro.backtest_client import BacktestClient


@pytest.fixture
def client():
    return BacktestClient("http://localhost:8080")


@pytest.mark.asyncio
async def test_client_start_backtest(client):
    """Test starting a backtest via mocked HTTP POST."""
    mock_response = MagicMock()
    mock_response.status = 200
    mock_response.json = AsyncMock(return_value={
        "backtest_id": "bt_test_123",
        "status": "running",
        "total_bars": 4320,
    })

    mock_session = MagicMock()
    mock_session.post = AsyncMock(return_value=mock_response)
    mock_session.__aenter__ = AsyncMock(return_value=mock_session)
    mock_session.__aexit__ = AsyncMock(return_value=None)

    with patch("aiohttp.ClientSession", return_value=mock_session):
        result = await client.start_backtest(
            config={
                "symbol": "BTC-USDT",
                "initial_balance": "100000",
                "margin_mode": "Cross",
                "default_leverage": "10",
            },
            strategy_id="ema_cross_v1",
            timeframe="1h",
            start_time=1704067200000,
            end_time=1706745600000,
        )

    assert result["backtest_id"] == "bt_test_123"
    assert result["status"] == "running"
    assert result["total_bars"] == 4320


@pytest.mark.asyncio
async def test_client_get_state(client):
    """Test fetching engine state via mocked HTTP GET."""
    mock_response = MagicMock()
    mock_response.status = 200
    mock_response.json = AsyncMock(return_value={
        "timestamp": 1704067200000,
        "equity": "100050.50",
        "available_balance": "90000",
        "margin_used": "10050.50",
        "total_trades": 5,
        "win_rate": 60.0,
    })

    mock_session = MagicMock()
    mock_session.get = AsyncMock(return_value=mock_response)
    mock_session.__aenter__ = AsyncMock(return_value=mock_session)
    mock_session.__aexit__ = AsyncMock(return_value=None)

    with patch("aiohttp.ClientSession", return_value=mock_session):
        state = await client.get_state("bt_test_123")

    assert state["equity"] == "100050.50"
    assert state["total_trades"] == 5


@pytest.mark.asyncio
async def test_client_pause_backtest(client):
    """Test pausing a backtest."""
    mock_response = MagicMock()
    mock_response.status = 200
    mock_response.json = AsyncMock(return_value={"status": "paused"})

    mock_session = MagicMock()
    mock_session.post = AsyncMock(return_value=mock_response)
    mock_session.__aenter__ = AsyncMock(return_value=mock_session)
    mock_session.__aexit__ = AsyncMock(return_value=None)

    with patch("aiohttp.ClientSession", return_value=mock_session):
        result = await client.pause_backtest("bt_test_123")

    assert result["status"] == "paused"


@pytest.mark.asyncio
async def test_client_resume_backtest(client):
    """Test resuming a paused backtest."""
    mock_response = MagicMock()
    mock_response.status = 200
    mock_response.json = AsyncMock(return_value={"status": "running"})

    mock_session = MagicMock()
    mock_session.post = AsyncMock(return_value=mock_response)
    mock_session.__aenter__ = AsyncMock(return_value=mock_session)
    mock_session.__aexit__ = AsyncMock(return_value=None)

    with patch("aiohttp.ClientSession", return_value=mock_session):
        result = await client.resume_backtest("bt_test_123")

    assert result["status"] == "running"


@pytest.mark.asyncio
async def test_client_get_result(client):
    """Test fetching backtest result."""
    mock_response = MagicMock()
    mock_response.status = 200
    mock_response.json = AsyncMock(return_value={
        "backtest_id": "bt_test_123",
        "final_equity": "145230.50",
        "total_return_pct": 45.23,
        "max_drawdown_pct": 12.5,
        "sharpe_ratio": 1.85,
        "total_trades": 156,
        "win_rate": 58.3,
        "profit_factor": 2.1,
        "avg_trade_return": 1.2,
        "daily_pnls": [],
        "trades": [],
    })

    mock_session = MagicMock()
    mock_session.get = AsyncMock(return_value=mock_response)
    mock_session.__aenter__ = AsyncMock(return_value=mock_session)
    mock_session.__aexit__ = AsyncMock(return_value=None)

    with patch("aiohttp.ClientSession", return_value=mock_session):
        result = await client.get_result("bt_test_123")

    assert result["final_equity"] == "145230.50"
    assert result["total_return_pct"] == 45.23
    assert result["total_trades"] == 156


@pytest.mark.asyncio
async def test_client_http_error_handling(client):
    """Test client handles HTTP errors gracefully."""
    mock_response = MagicMock()
    mock_response.status = 500
    mock_response.text = AsyncMock(return_value="Internal Server Error")

    mock_session = MagicMock()
    mock_session.get = AsyncMock(return_value=mock_response)
    mock_session.__aenter__ = AsyncMock(return_value=mock_session)
    mock_session.__aexit__ = AsyncMock(return_value=None)

    with patch("aiohttp.ClientSession", return_value=mock_session):
        with pytest.raises(Exception):
            await client.get_state("bt_test_123")
