import { useState, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import Layout from '../components/Layout';

interface BacktestConfigForm {
  symbol: string;
  timeframe: string;
  startDate: string;
  endDate: string;
  strategy: string;
  initialBalance: string;
  leverage: string;
  makerFee: string;
  takerFee: string;
  marginMode: string;
  executionDelay: string;
}

const SYMBOLS = ['BTC-USDT', 'ETH-USDT', 'SOL-USDT', 'BNB-USDT', 'XRP-USDT'];
const TIMEFRAMES = ['M1', 'M5', 'M15', 'M30', 'H1', 'H4', 'D1', 'W1'];
const STRATEGIES = [
  { id: 'ema_cross_v1', name: 'EMA Cross v1', params: { fast: 9, slow: 21 } },
  { id: 'rsi_reversal', name: 'RSI Reversal', params: { period: 14, overbought: 70, oversold: 30 } },
  { id: 'bollinger_breakout', name: 'Bollinger Breakout', params: { period: 20, stdDev: 2 } },
  { id: 'macd_momentum', name: 'MACD Momentum', params: { fast: 12, slow: 26, signal: 9 } },
];

export default function BacktestConfig() {
  const navigate = useNavigate();
  const [form, setForm] = useState<BacktestConfigForm>({
    symbol: 'BTC-USDT',
    timeframe: 'H1',
    startDate: '2024-01-01',
    endDate: '2024-06-01',
    strategy: 'ema_cross_v1',
    initialBalance: '100000',
    leverage: '10',
    makerFee: '0.0002',
    takerFee: '0.0005',
    marginMode: 'cross',
    executionDelay: '1',
  });
  const [loading, setLoading] = useState(false);

  const handleChange = useCallback(
    (field: keyof BacktestConfigForm, value: string) => {
      setForm((prev) => ({ ...prev, [field]: value }));
    },
    []
  );

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setLoading(true);
      try {
        const res = await fetch('/api/backtest/start', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            config: {
              symbol: form.symbol,
              initial_balance: form.initialBalance,
              margin_mode: form.marginMode,
              default_leverage: form.leverage,
              maker_fee_rate: form.makerFee,
              taker_fee_rate: form.takerFee,
              execution_delay_bars: parseInt(form.executionDelay),
            },
            strategy_id: form.strategy,
            timeframe: form.timeframe.toLowerCase(),
            start_time: new Date(form.startDate).getTime(),
            end_time: new Date(form.endDate).getTime(),
          }),
        });
        if (res.ok) {
          const data = (await res.json()) as { backtest_id: string };
          navigate(`/?backtest=${data.backtest_id}`);
        } else {
          // Fallback: just navigate to dashboard for mock mode
          navigate('/');
        }
      } catch {
        navigate('/');
      } finally {
        setLoading(false);
      }
    },
    [form, navigate]
  );

  const selectedStrategy = STRATEGIES.find((s) => s.id === form.strategy);

  return (
    <Layout>
      <div className="flex-1 overflow-y-auto p-6">
        <div className="max-w-3xl mx-auto">
          <h1 className="text-2xl font-bold text-slate-100 mb-6">Backtest Configuration</h1>

          <form onSubmit={handleSubmit} className="space-y-6">
            {/* Symbol & Timeframe */}
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <label className="text-xs font-semibold text-slate-400 uppercase">Symbol</label>
                <select
                  value={form.symbol}
                  onChange={(e) => handleChange('symbol', e.target.value)}
                  className="w-full bg-slate-800 border border-slate-700 rounded px-3 py-2 text-sm text-slate-200 focus:outline-none focus:border-blue-500"
                >
                  {SYMBOLS.map((s) => (
                    <option key={s} value={s}>
                      {s}
                    </option>
                  ))}
                </select>
              </div>
              <div className="space-y-2">
                <label className="text-xs font-semibold text-slate-400 uppercase">Timeframe</label>
                <select
                  value={form.timeframe}
                  onChange={(e) => handleChange('timeframe', e.target.value)}
                  className="w-full bg-slate-800 border border-slate-700 rounded px-3 py-2 text-sm text-slate-200 focus:outline-none focus:border-blue-500"
                >
                  {TIMEFRAMES.map((tf) => (
                    <option key={tf} value={tf}>
                      {tf}
                    </option>
                  ))}
                </select>
              </div>
            </div>

            {/* Date Range */}
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <label className="text-xs font-semibold text-slate-400 uppercase">Start Date</label>
                <input
                  type="date"
                  value={form.startDate}
                  onChange={(e) => handleChange('startDate', e.target.value)}
                  className="w-full bg-slate-800 border border-slate-700 rounded px-3 py-2 text-sm text-slate-200 focus:outline-none focus:border-blue-500"
                />
              </div>
              <div className="space-y-2">
                <label className="text-xs font-semibold text-slate-400 uppercase">End Date</label>
                <input
                  type="date"
                  value={form.endDate}
                  onChange={(e) => handleChange('endDate', e.target.value)}
                  className="w-full bg-slate-800 border border-slate-700 rounded px-3 py-2 text-sm text-slate-200 focus:outline-none focus:border-blue-500"
                />
              </div>
            </div>

            {/* Strategy */}
            <div className="space-y-2">
              <label className="text-xs font-semibold text-slate-400 uppercase">Strategy</label>
              <select
                value={form.strategy}
                onChange={(e) => handleChange('strategy', e.target.value)}
                className="w-full bg-slate-800 border border-slate-700 rounded px-3 py-2 text-sm text-slate-200 focus:outline-none focus:border-blue-500"
              >
                {STRATEGIES.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name}
                  </option>
                ))}
              </select>
              {selectedStrategy && (
                <div className="bg-slate-800/50 rounded p-2 border border-slate-700">
                  <div className="text-[10px] text-slate-500 uppercase mb-1">Parameters</div>
                  <div className="flex flex-wrap gap-2">
                    {Object.entries(selectedStrategy.params).map(([k, v]) => (
                      <span key={k} className="text-xs text-slate-300 bg-slate-800 px-2 py-0.5 rounded border border-slate-700">
                        {k}: {v}
                      </span>
                    ))}
                  </div>
                </div>
              )}
            </div>

            {/* Account Settings */}
            <div className="bg-slate-900 rounded border border-slate-800 p-4 space-y-4">
              <h3 className="text-sm font-bold text-slate-200">Account Settings</h3>
              <div className="grid grid-cols-3 gap-4">
                <div className="space-y-2">
                  <label className="text-xs text-slate-500">Initial Balance</label>
                  <input
                    type="number"
                    value={form.initialBalance}
                    onChange={(e) => handleChange('initialBalance', e.target.value)}
                    className="w-full bg-slate-800 border border-slate-700 rounded px-3 py-2 text-sm text-slate-200 focus:outline-none focus:border-blue-500"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-xs text-slate-500">Leverage</label>
                  <input
                    type="number"
                    value={form.leverage}
                    onChange={(e) => handleChange('leverage', e.target.value)}
                    className="w-full bg-slate-800 border border-slate-700 rounded px-3 py-2 text-sm text-slate-200 focus:outline-none focus:border-blue-500"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-xs text-slate-500">Margin Mode</label>
                  <select
                    value={form.marginMode}
                    onChange={(e) => handleChange('marginMode', e.target.value)}
                    className="w-full bg-slate-800 border border-slate-700 rounded px-3 py-2 text-sm text-slate-200 focus:outline-none focus:border-blue-500"
                  >
                    <option value="cross">Cross</option>
                    <option value="isolated">Isolated</option>
                  </select>
                </div>
              </div>
              <div className="grid grid-cols-3 gap-4">
                <div className="space-y-2">
                  <label className="text-xs text-slate-500">Maker Fee</label>
                  <input
                    type="number"
                    step="0.0001"
                    value={form.makerFee}
                    onChange={(e) => handleChange('makerFee', e.target.value)}
                    className="w-full bg-slate-800 border border-slate-700 rounded px-3 py-2 text-sm text-slate-200 focus:outline-none focus:border-blue-500"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-xs text-slate-500">Taker Fee</label>
                  <input
                    type="number"
                    step="0.0001"
                    value={form.takerFee}
                    onChange={(e) => handleChange('takerFee', e.target.value)}
                    className="w-full bg-slate-800 border border-slate-700 rounded px-3 py-2 text-sm text-slate-200 focus:outline-none focus:border-blue-500"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-xs text-slate-500">Execution Delay (bars)</label>
                  <input
                    type="number"
                    value={form.executionDelay}
                    onChange={(e) => handleChange('executionDelay', e.target.value)}
                    className="w-full bg-slate-800 border border-slate-700 rounded px-3 py-2 text-sm text-slate-200 focus:outline-none focus:border-blue-500"
                  />
                </div>
              </div>
            </div>

            <div className="flex gap-3">
              <button
                type="submit"
                disabled={loading}
                className="px-6 py-2.5 bg-blue-600 hover:bg-blue-500 disabled:bg-blue-800 text-white text-sm font-bold rounded transition-colors"
              >
                {loading ? 'Starting...' : 'Start Backtest'}
              </button>
              <button
                type="button"
                onClick={() => navigate('/')}
                className="px-6 py-2.5 bg-slate-800 hover:bg-slate-700 text-slate-300 text-sm font-bold rounded transition-colors"
              >
                Cancel
              </button>
            </div>
          </form>
        </div>
      </div>
    </Layout>
  );
}
