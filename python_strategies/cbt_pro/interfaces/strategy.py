from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from decimal import Decimal
from typing import List, Optional, Dict, Any
from enum import Enum

class SignalAction(Enum):
    OPEN_LONG = "open_long"
    OPEN_SHORT = "open_short"
    ADD_LONG = "add_long"
    ADD_SHORT = "add_short"
    REDUCE_LONG = "reduce_long"
    REDUCE_SHORT = "reduce_short"
    CLOSE_LONG = "close_long"
    CLOSE_SHORT = "close_short"
    CLOSE_ALL = "close_all"

@dataclass
class StandardBar:
    timestamp: int
    open: Decimal
    high: Decimal
    low: Decimal
    close: Decimal
    volume: Decimal
    symbol: str
    exchange: str
    confirmed: bool

@dataclass
class PositionSnapshot:
    id: str
    symbol: str
    direction: str
    current_size: Decimal
    average_entry_price: Decimal
    unrealized_pnl: Decimal
    realized_pnl: Decimal
    leverage: Decimal
    margin_used: Decimal
    opened_at: int

@dataclass
class StrategyContext:
    current_price: Decimal
    open_orders: int
    positions: List[PositionSnapshot]
    equity: Decimal
    available_balance: Decimal
    unrealized_pnl: Decimal
    margin_ratio: Decimal
    bar_history: List[StandardBar]
    current_bar_index: int
    total_bars: int
    timestamp: int

@dataclass
class Signal:
    action: SignalAction
    symbol: str
    quantity: Decimal
    strength: float
    reason: str
    metadata: Dict[str, Any] = field(default_factory=dict)
    take_profit: Optional[Decimal] = None
    stop_loss: Optional[Decimal] = None

class BaseStrategy(ABC):
    @property
    @abstractmethod
    def name(self) -> str: ...

    @property
    @abstractmethod
    def version(self) -> str: ...

    @property
    @abstractmethod
    def required_indicators(self) -> List[str]: ...

    @abstractmethod
    def on_bar(self, bar: StandardBar, context: StrategyContext) -> Optional[Signal]: ...

    @abstractmethod
    def on_position_update(self, position: PositionSnapshot, context: StrategyContext) -> Optional[Signal]: ...

    def get_parameters(self) -> Dict[str, Any]:
        return {}
