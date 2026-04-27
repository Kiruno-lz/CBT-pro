import { useState, type RefObject } from 'react';
import { useAppStore } from '../../stores/useAppStore';
import { EngineWebSocket } from '../../stores/websocket';
import type { TimeFrame } from '../../types';

interface BacktestConfigForm {
  symbol: string;
  timeframe: TimeFrame;
  startDate: string;
  endDate: string;
  initialBalance: string;
  leverage: string;
  strategy: string;
}

const DEFAULT_FORM: BacktestConfigForm = {
  symbol: 'BTCUSDT',
  timeframe: 'H1',
  startDate: '2024-01-01',
  endDate: '2024-12-31',
  initialBalance: '10000',
  leverage: '10',
  strategy: 'ema_cross',
};

const TIMEFRAMES: TimeFrame[] = ['M1', 'M5', 'M15', 'M30', 'H1', 'H4', 'D1', 'W1'];

const TIMEFAME_MAP: Record<string, string> = {
  M1: '1m',
  M5: '5m',
  M15: '15m',
  M30: '30m',
  H1: '1h',
  H4: '4h',
  D1: '1d',
  W1: '1w',
};

const STRATEGY_ID_MAP: Record<string, string> = {
  always_long: 'always_long',
  ema_cross: 'ema_crossover',
  rsi_macd: 'rsi_macd',
  bollinger: 'bollinger_bands',
  breakout: 'breakout',
};

function dateToTimestamp(dateStr: string): number {
  return new Date(dateStr).getTime();
}

function formatSymbol(symbol: string): string {
  if (symbol.includes('-')) return symbol;
  const base = symbol.replace(/USDT$/, '');
  return `${base}-USDT`;
}

const API_BASE = import.meta.env.DEV ? '' : (import.meta.env.VITE_API_BASE || '');

interface BacktestConfigProps {
  wsRef: RefObject<EngineWebSocket | null>;
}

export function BacktestConfig({ wsRef }: BacktestConfigProps) {
  const { setPlayback, setWsConnected } = useAppStore();
  const [form, setForm] = useState<BacktestConfigForm>(DEFAULT_FORM);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleChange = (field: keyof BacktestConfigForm, value: string) => {
    setForm((prev) => ({ ...prev, [field]: value }));
    setError(null);
  };

  const handleStart = async () => {
    setLoading(true);
    setError(null);

    try {
      const response = await fetch(`${API_BASE}/api/backtest/start`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          config: {
            symbol: formatSymbol(form.symbol),
            initial_balance: form.initialBalance,
            margin_mode: 'Isolated',
            default_leverage: form.leverage,
          },
          strategy_id: STRATEGY_ID_MAP[form.strategy] || form.strategy,
          timeframe: TIMEFAME_MAP[form.timeframe] || form.timeframe,
          start_time: dateToTimestamp(form.startDate),
          end_time: dateToTimestamp(form.endDate),
        }),
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const data = await response.json();
      wsRef.current?.subscribe(data.backtest_id);

      setPlayback({
        status: 'paused',
        totalBars: data.total_bars || 0,
        currentBarIndex: 0,
        currentTime: 0,
      });

      setWsConnected(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to start backtest');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="panel flex-shrink-0">
      <div className="panel-header">
        <span className="panel-title">Backtest Config</span>
      </div>

      <div className="panel-body space-y-4">
        <div className="space-y-1.5">
          <label className="label">Symbol</label>
          <select
            value={form.symbol}
            onChange={(e) => handleChange('symbol', e.target.value)}
            className="input-field"
          >
            <option value="BTCUSDT">BTC/USDT</option>
            <option value="ETHUSDT">ETH/USDT</option>
            <option value="BNBUSDT">BNB/USDT</option>
          </select>
        </div>

        <div className="space-y-1.5">
          <label className="label">Timeframe</label>
          <div className="flex gap-1">
            {TIMEFRAMES.map((tf) => (
              <button
                key={tf}
                onClick={() => handleChange('timeframe', tf)}
                className={`flex-1 py-1 text-2xs font-mono rounded transition-colors ${
                  form.timeframe === tf
                    ? 'bg-accent-cyan text-surface-base'
                    : 'bg-surface-raised text-text-secondary hover:bg-surface-elevated'
                }`}
              >
                {tf}
              </button>
            ))}
          </div>
        </div>

        <div className="grid grid-cols-2 gap-2">
          <div className="space-y-1.5">
            <label className="label">Start Date</label>
            <input
              type="date"
              value={form.startDate}
              onChange={(e) => handleChange('startDate', e.target.value)}
              className="input-field"
            />
          </div>
          <div className="space-y-1.5">
            <label className="label">End Date</label>
            <input
              type="date"
              value={form.endDate}
              onChange={(e) => handleChange('endDate', e.target.value)}
              className="input-field"
            />
          </div>
        </div>

        <div className="grid grid-cols-2 gap-2">
          <div className="space-y-1.5">
            <label className="label">Initial Balance</label>
            <input
              type="number"
              value={form.initialBalance}
              onChange={(e) => handleChange('initialBalance', e.target.value)}
              className="input-field font-mono"
              min="100"
              step="100"
            />
          </div>
          <div className="space-y-1.5">
            <label className="label">Leverage</label>
            <input
              type="number"
              value={form.leverage}
              onChange={(e) => handleChange('leverage', e.target.value)}
              className="input-field font-mono"
              min="1"
              max="100"
            />
          </div>
        </div>

        <div className="space-y-1.5">
          <label className="label">Strategy</label>
          <select
            value={form.strategy}
            onChange={(e) => handleChange('strategy', e.target.value)}
            className="input-field"
          >
            <option value="always_long">Always Long</option>
            <option value="ema_cross">EMA Crossover</option>
            <option value="rsi_macd">RSI + MACD</option>
            <option value="bollinger">Bollinger Bands</option>
            <option value="breakout">Breakout</option>
          </select>
        </div>

        {error && (
          <div className="bg-accent-red/10 border border-accent-red/30 rounded p-2">
            <span className="text-xs text-accent-red">{error}</span>
          </div>
        )}

        <button onClick={handleStart} disabled={loading} className="btn-primary w-full">
          {loading ? (
            <span className="flex items-center justify-center gap-2">
              <svg className="animate-spin w-4 h-4" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>
              Starting...
            </span>
          ) : (
            'START BACKTEST'
          )}
        </button>
      </div>
    </div>
  );
}