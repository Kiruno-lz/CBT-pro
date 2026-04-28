export interface StandardBar {
  timestamp: number;
  open: string;
  high: string;
  low: string;
  close: string;
  volume: string;
  symbol: string;
  exchange: string;
  confirmed: boolean;
}

export interface PositionLeg {
  entry_price: string;
  quantity: string;
  timestamp: number;
  order_id: string;
}

export interface Position {
  id: string;
  symbol: string;
  direction: 'Long' | 'Short';
  status: string;
  entries: PositionLeg[];
  current_size: string;
  average_entry_price: string;
  realized_pnl: string;
  unrealized_pnl: string;
  opened_at: number;
  updated_at: number;
}

export interface OrderFill {
  order_id: string;
  position_id?: string;
  symbol: string;
  side: 'Buy' | 'Sell';
  direction: 'Long' | 'Short';
  filled_price: string;
  filled_quantity: string;
  fee: string;
  fee_asset?: string;
  timestamp: number;
  realized_pnl?: string;
}

export type SignalAction =
  | 'open_long'
  | 'open_short'
  | 'add_long'
  | 'add_short'
  | 'reduce_long'
  | 'reduce_short'
  | 'close_long'
  | 'close_short'
  | 'close_all';

export interface Signal {
  action: SignalAction;
  symbol: string;
  quantity: string;
  strength: number;
  reason: string;
  timestamp: number;
  take_profit?: string;
  stop_loss?: string;
}

export interface EngineSnapshot {
  timestamp: number;
  current_bar_index: number;
  current_bar: StandardBar;
  equity: string;
  available_balance: string;
  margin_used: string;
  margin_ratio: string;
  unrealized_pnl: string;
  realized_pnl_today: string;
  positions: Position[];
  total_trades: number;
  win_rate: number;
  max_drawdown_pct: number;
  sharpe_ratio?: number;
}

export interface BacktestResult {
  backtest_id: string;
  final_equity: string;
  total_return_pct: number;
  max_drawdown_pct: number;
  sharpe_ratio: number;
  total_trades: number;
  win_rate: number;
  profit_factor: number;
  avg_trade_return: number;
  daily_pnls: Array<{ date: number; pnl: string }>;
  trades: OrderFill[];
}

export interface PlaybackState {
  status: 'idle' | 'playing' | 'paused' | 'stepping' | 'complete';
  currentBarIndex: number;
  totalBars: number;
  speed: number | 'max';
  currentTime: number;
}

export type TimeFrame = 'M1' | 'M5' | 'M15' | 'M30' | 'H1' | 'H4' | 'D1' | 'W1';

export interface IndicatorConfig {
  name: string;
  params: Record<string, number>;
  visible: boolean;
  panel: 'main' | 'sub';
}

export interface ParamTypeInteger {
  min: number;
  max: number;
  default: number;
}

export interface ParamTypeDecimal {
  min: string;
  max: string;
  default: string;
}

export interface ParamTypeString {
  default: string;
  options: string[];
}

export interface ParamDefinition {
  name: string;
  description: string;
  param_type: {
    Integer?: ParamTypeInteger;
    Decimal?: ParamTypeDecimal;
    String?: ParamTypeString;
  };
}

export interface StrategyDefaults {
  id: string;
  name: string;
  description: string;
  default_params: Record<string, unknown>;
  param_definitions: ParamDefinition[];
}

export interface IndicatorValue {
  value?: string;
  upper?: string;
  middle?: string;
  lower?: string;
  macd?: string;
  signal?: string;
  histogram?: string;
  timestamp: number;
}

export type IndicatorSeries = IndicatorValue[];

export interface IndicatorsResponse {
  [key: string]: IndicatorValue | IndicatorSeries;
}

export interface AppState {
  wsConnected: boolean;
  engineOnline: boolean;
  playback: PlaybackState;
  bars: StandardBar[];
  snapshot: EngineSnapshot | null;
  signals: Signal[];
  activeSignals: Signal[];
  tradeHistory: OrderFill[];
  indicators: IndicatorConfig[];
  chartTimeframe: TimeFrame;
  visibleRange: { from: number; to: number };
  markerVisibility: boolean;
  backtestResult: BacktestResult | null;
  backtestId: string | null;
  currentStrategy: StrategyDefaults | null;
}

export interface AppActions {
  setWsConnected: (connected: boolean) => void;
  setEngineOnline: (online: boolean) => void;
  setPlayback: (playback: Partial<PlaybackState>) => void;
  setBars: (bars: StandardBar[]) => void;
  appendBar: (bar: StandardBar) => void;
  setSnapshot: (snapshot: EngineSnapshot) => void;
  addSignal: (signal: Signal) => void;
  setActiveSignals: (signals: Signal[]) => void;
  addTrade: (trade: OrderFill) => void;
  setTradeHistory: (trades: OrderFill[]) => void;
  setIndicators: (indicators: IndicatorConfig[]) => void;
  updateIndicator: (name: string, patch: Partial<IndicatorConfig>) => void;
  setChartTimeframe: (tf: TimeFrame) => void;
  setVisibleRange: (range: { from: number; to: number }) => void;
  setMarkerVisibility: (visible: boolean) => void;
  setBacktestResult: (result: BacktestResult | null) => void;
  setBacktestId: (id: string | null) => void;
  setCurrentStrategy: (strategy: StrategyDefaults | null) => void;
  reset: () => void;
}

export type WsMessage =
  | { type: 'snapshot'; data: EngineSnapshot }
  | { type: 'bar_update'; bar: StandardBar }
  | { type: 'trade'; fill: OrderFill }
  | { type: 'signal'; signal: Signal }
  | { type: 'complete'; result: BacktestResult }
  | { type: 'error'; message: string };
