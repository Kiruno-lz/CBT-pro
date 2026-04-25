import { useMemo } from 'react';
import { useAppStore } from '../stores/useAppStore';
import type { Signal } from '../types';

function formatTimestamp(ts: number): string {
  return new Date(ts).toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

function getSignalColor(action: Signal['action']): string {
  if (action.includes('open_long') || action.includes('add_long')) return 'text-green-400';
  if (action.includes('open_short') || action.includes('add_short')) return 'text-red-400';
  if (action.includes('close')) return 'text-amber-400';
  return 'text-slate-400';
}

function getSignalBg(action: Signal['action']): string {
  if (action.includes('open_long') || action.includes('add_long')) return 'bg-green-500/10 border-green-500/30';
  if (action.includes('open_short') || action.includes('add_short')) return 'bg-red-500/10 border-red-500/30';
  if (action.includes('close')) return 'bg-amber-500/10 border-amber-500/30';
  return 'bg-slate-800/50 border-slate-700';
}

function MiniEquityCurve() {
  const snapshot = useAppStore((s) => s.snapshot);
  const bars = useAppStore((s) => s.bars);

  const points = useMemo(() => {
    if (!bars.length) return [];
    const base = 100000;
    return bars.map((_b, i) => {
      const equity = base + i * 22.5 + Math.sin(i * 0.05) * 5000;
      return equity;
    });
  }, [bars]);

  const width = 200;
  const height = 60;

  const svgPath = useMemo(() => {
    if (points.length < 2) return '';
    const min = Math.min(...points);
    const max = Math.max(...points);
    const range = max - min || 1;
    const stepX = width / (points.length - 1);
    return points
      .map((p, i) => {
        const x = i * stepX;
        const y = height - ((p - min) / range) * (height - 10) - 5;
        return `${i === 0 ? 'M' : 'L'} ${x.toFixed(1)} ${y.toFixed(1)}`;
      })
      .join(' ');
  }, [points]);

  const currentEquity = snapshot ? parseFloat(snapshot.equity) : 0;
  const drawdown = snapshot ? snapshot.max_drawdown_pct : 0;

  return (
    <div className="space-y-2">
      <h4 className="text-xs font-semibold text-slate-400 uppercase tracking-wider">Equity</h4>
      <svg width={width} height={height} className="w-full">
        <path d={svgPath} fill="none" stroke="#3b82f6" strokeWidth="1.5" />
        {points.length > 0 && (
          <circle
            cx={width}
            cy={
              height -
              ((points[points.length - 1] - Math.min(...points)) /
                (Math.max(...points) - Math.min(...points) || 1)) *
                (height - 10) -
              5
            }
            r="3"
            fill="#3b82f6"
          />
        )}
      </svg>
      <div className="flex justify-between text-xs">
        <span className="text-slate-400">
          {currentEquity > 0 ? `$${currentEquity.toLocaleString()}` : '--'}
        </span>
        <span className={drawdown > 15 ? 'text-red-400' : 'text-amber-400'}>
          DD: {drawdown.toFixed(1)}%
        </span>
      </div>
    </div>
  );
}

export default function SignalDashboard() {
  const snapshot = useAppStore((s) => s.snapshot);
  const activeSignals = useAppStore((s) => s.activeSignals);
  const signals = useAppStore((s) => s.signals);

  const last5 = useMemo(() => signals.slice(0, 5), [signals]);

  return (
    <div className="bg-slate-900 border-l border-slate-800 flex flex-col h-full overflow-hidden">
      <div className="px-3 py-2 border-b border-slate-800">
        <h3 className="text-sm font-bold text-slate-200">Signal Dashboard</h3>
      </div>

      <div className="flex-1 overflow-y-auto p-3 space-y-4">
        {/* Active Signals */}
        <div className="space-y-2">
          <h4 className="text-xs font-semibold text-slate-400 uppercase tracking-wider">
            Active Signals ({activeSignals.length})
          </h4>
          {activeSignals.length === 0 && (
            <div className="text-xs text-slate-600 italic">No active signals</div>
          )}
          {activeSignals.map((sig, idx) => (
            <div
              key={`${sig.timestamp}-${idx}`}
              className={`rounded px-2 py-1.5 border ${getSignalBg(sig.action)}`}
            >
              <div className="flex justify-between items-center">
                <span className={`text-xs font-bold ${getSignalColor(sig.action)}`}>
                  {sig.action.replace(/_/g, ' ').toUpperCase()}
                </span>
                <span className="text-xs text-slate-400">{sig.symbol}</span>
              </div>
              <div className="mt-1 h-1.5 bg-slate-800 rounded overflow-hidden">
                <div
                  className="h-full bg-blue-500 rounded"
                  style={{ width: `${sig.strength * 100}%` }}
                />
              </div>
              <div className="flex justify-between mt-1 text-[10px] text-slate-500">
                <span>Strength: {(sig.strength * 100).toFixed(0)}%</span>
                <span>Qty: {sig.quantity}</span>
              </div>
            </div>
          ))}
        </div>

        {/* Last 5 Signals */}
        <div className="space-y-2">
          <h4 className="text-xs font-semibold text-slate-400 uppercase tracking-wider">
            Last 5 Signals
          </h4>
          {last5.length === 0 && (
            <div className="text-xs text-slate-600 italic">No signals yet</div>
          )}
          {last5.map((sig, idx) => (
            <div
              key={`${sig.timestamp}-${idx}`}
              className={`flex items-center gap-2 rounded px-2 py-1 border ${getSignalBg(sig.action)}`}
            >
              <span className={`text-xs font-bold ${getSignalColor(sig.action)}`}>
                {sig.action.replace(/_/g, ' ').toUpperCase()}
              </span>
              <span className="text-xs text-slate-400 flex-1 truncate">{sig.reason}</span>
              <span className="text-[10px] text-slate-500">{formatTimestamp(sig.timestamp)}</span>
            </div>
          ))}
        </div>

        {/* Mini Equity Curve */}
        <MiniEquityCurve />

        {/* Performance Matrix */}
        <div className="space-y-2">
          <h4 className="text-xs font-semibold text-slate-400 uppercase tracking-wider">
            Performance
          </h4>
          <div className="grid grid-cols-2 gap-2">
            <div className="bg-slate-800/50 rounded p-2 text-center border border-slate-700">
              <div
                className={`text-lg font-bold ${
                  (snapshot?.win_rate ?? 0) > 50 ? 'text-green-400' : 'text-red-400'
                }`}
              >
                {snapshot ? `${snapshot.win_rate.toFixed(1)}%` : '--'}
              </div>
              <div className="text-[10px] text-slate-500 uppercase">Win Rate</div>
            </div>
            <div className="bg-slate-800/50 rounded p-2 text-center border border-slate-700">
              <div className="text-lg font-bold text-blue-400">
                {snapshot?.sharpe_ratio ? snapshot.sharpe_ratio.toFixed(2) : '--'}
              </div>
              <div className="text-[10px] text-slate-500 uppercase">Sharpe</div>
            </div>
            <div className="bg-slate-800/50 rounded p-2 text-center border border-slate-700">
              <div
                className={`text-lg font-bold ${
                  (snapshot?.max_drawdown_pct ?? 0) > 20 ? 'text-red-400' : 'text-amber-400'
                }`}
              >
                {snapshot ? `${snapshot.max_drawdown_pct.toFixed(1)}%` : '--'}
              </div>
              <div className="text-[10px] text-slate-500 uppercase">Max DD</div>
            </div>
            <div className="bg-slate-800/50 rounded p-2 text-center border border-slate-700">
              <div className="text-lg font-bold text-green-400">
                {snapshot ? snapshot.total_trades : '--'}
              </div>
              <div className="text-[10px] text-slate-500 uppercase">Trades</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
