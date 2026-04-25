import { create } from 'zustand';
import type {
  StandardBar,
  EngineSnapshot,
  Signal,
  OrderFill,
  IndicatorConfig,
  PlaybackState,
  TimeFrame,
  BacktestResult,
} from '../types';

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
  reset: () => void;
}

const initialPlayback: PlaybackState = {
  status: 'idle',
  currentBarIndex: 0,
  totalBars: 0,
  speed: 1,
  currentTime: 0,
};

const defaultIndicators: IndicatorConfig[] = [
  { name: 'ema_9', params: { period: 9 }, visible: true, panel: 'main' },
  { name: 'ema_21', params: { period: 21 }, visible: true, panel: 'main' },
  { name: 'rsi_14', params: { period: 14 }, visible: true, panel: 'sub' },
  { name: 'macd', params: { fast: 12, slow: 26, signal: 9 }, visible: true, panel: 'sub' },
  { name: 'volume', params: {}, visible: true, panel: 'sub' },
  { name: 'bollinger', params: { period: 20, stdDev: 2 }, visible: false, panel: 'main' },
];

const initialState: AppState = {
  wsConnected: false,
  engineOnline: false,
  playback: initialPlayback,
  bars: [],
  snapshot: null,
  signals: [],
  activeSignals: [],
  tradeHistory: [],
  indicators: defaultIndicators,
  chartTimeframe: 'H1',
  visibleRange: { from: 0, to: 0 },
  markerVisibility: true,
  backtestResult: null,
};

export const useAppStore = create<AppState & AppActions>((set) => ({
  ...initialState,

  setWsConnected: (connected) => set({ wsConnected: connected }),
  setEngineOnline: (online) => set({ engineOnline: online }),
  setPlayback: (playback) =>
    set((state) => ({
      playback: { ...state.playback, ...playback },
    })),
  setBars: (bars) => set({ bars }),
  appendBar: (bar) =>
    set((state) => {
      const last = state.bars[state.bars.length - 1];
      if (last && last.timestamp === bar.timestamp) {
        const next = [...state.bars];
        next[next.length - 1] = bar;
        return { bars: next };
      }
      return { bars: [...state.bars, bar] };
    }),
  setSnapshot: (snapshot) => set({ snapshot }),
  addSignal: (signal) =>
    set((state) => {
      const signals = [signal, ...state.signals].slice(0, 50);
      const isActive =
        signal.action === 'open_long' ||
        signal.action === 'open_short' ||
        signal.action === 'add_long' ||
        signal.action === 'add_short';
      const activeSignals = isActive
        ? [...state.activeSignals, signal]
        : state.activeSignals.filter(
            (s) =>
              !(
                (signal.action === 'close_long' && s.action === 'open_long') ||
                (signal.action === 'close_short' && s.action === 'open_short') ||
                (signal.action === 'close_all')
              )
          );
      return { signals, activeSignals };
    }),
  setActiveSignals: (signals) => set({ activeSignals: signals }),
  addTrade: (trade) =>
    set((state) => ({
      tradeHistory: [trade, ...state.tradeHistory],
    })),
  setTradeHistory: (trades) => set({ tradeHistory: trades }),
  setIndicators: (indicators) => set({ indicators }),
  updateIndicator: (name, patch) =>
    set((state) => ({
      indicators: state.indicators.map((ind) =>
        ind.name === name ? { ...ind, ...patch } : ind
      ),
    })),
  setChartTimeframe: (tf) => set({ chartTimeframe: tf }),
  setVisibleRange: (range) => set({ visibleRange: range }),
  setMarkerVisibility: (visible) => set({ markerVisibility: visible }),
  setBacktestResult: (result) => set({ backtestResult: result }),
  reset: () => set(initialState),
}));
