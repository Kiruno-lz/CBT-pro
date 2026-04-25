"""HTTP SDK for communicating with the CBT-Pro Rust engine."""
import aiohttp
from decimal import Decimal
from typing import Optional, List, Dict, Any

class BacktestClient:
    """Async HTTP client for the CBT-Pro backtest engine REST API."""

    def __init__(self, base_url: str = "http://localhost:8080/api/v1"):
        self.base_url = base_url.rstrip("/")
        self._session: Optional[aiohttp.ClientSession] = None

    async def __aenter__(self):
        self._session = aiohttp.ClientSession()
        return self

    async def __aexit__(self, *args):
        if self._session:
            await self._session.close()
            self._session = None

    def _session_or_raise(self) -> aiohttp.ClientSession:
        if self._session is None:
            raise RuntimeError("Client not entered. Use 'async with BacktestClient() as client:'.")
        return self._session

    async def start_backtest(self, config: Dict[str, Any], strategy_id: str) -> Dict[str, Any]:
        payload = {"config": config, "strategy_id": strategy_id}
        async with self._session_or_raise().post(f"{self.base_url}/backtest/start", json=payload) as resp:
            resp.raise_for_status()
            return await resp.json()

    async def pause_backtest(self, backtest_id: str) -> Dict[str, Any]:
        async with self._session_or_raise().post(f"{self.base_url}/backtest/{backtest_id}/pause") as resp:
            resp.raise_for_status()
            return await resp.json()

    async def resume_backtest(self, backtest_id: str) -> Dict[str, Any]:
        async with self._session_or_raise().post(f"{self.base_url}/backtest/{backtest_id}/resume") as resp:
            resp.raise_for_status()
            return await resp.json()

    async def get_state(self, backtest_id: str) -> Dict[str, Any]:
        async with self._session_or_raise().get(f"{self.base_url}/backtest/{backtest_id}/state") as resp:
            resp.raise_for_status()
            return await resp.json()

    async def get_result(self, backtest_id: str) -> Dict[str, Any]:
        async with self._session_or_raise().get(f"{self.base_url}/backtest/{backtest_id}/result") as resp:
            resp.raise_for_status()
            return await resp.json()

    async def submit_order(self, order: Dict[str, Any]) -> Dict[str, Any]:
        async with self._session_or_raise().post(f"{self.base_url}/order", json=order) as resp:
            resp.raise_for_status()
            return await resp.json()

    async def get_indicators(self, symbol: str, timeframe: str, indicators: List[str]) -> Dict[str, Any]:
        params = {"symbol": symbol, "timeframe": timeframe, "indicators": ",".join(indicators)}
        async with self._session_or_raise().get(f"{self.base_url}/indicators", params=params) as resp:
            resp.raise_for_status()
            return await resp.json()
