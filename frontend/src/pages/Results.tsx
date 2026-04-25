import { useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import Layout from '../components/Layout';
import { useAppStore } from '../stores/useAppStore';

function formatCurrency(val: string | number): string {
  const n = typeof val === 'string' ? parseFloat(val) : val;
  if (isNaN(n)) return '--';
  return `$${n.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

function formatPercent(val: number): string {
  if (isNaN(val)) return '--';
  return `${val >= 0 ? '+' : ''}${val.toFixed(2)}%`;
}

export default function Results() {
  const navigate = useNavigate();
  const result = useAppStore((s) => s.backtestResult);
  const tradeHistory = useAppStore((s) => s.tradeHistory);

  const equityCurvePoints = useMemo(() => {
    if (!result?.daily_pnls?.length) {
      // Generate mock equity curve if no result
      const base = 100000;
      return Array.from({ length: 60 }, (_, i) => base + i * 750 + Math.sin(i * 0.3) * 3000);
    }
    let equity = 100000;
    return result.daily_pnls.map((d) => {
      equity += parseFloat(d.pnl);
      return equity;
    });
  }, [result]);

  const drawdownPoints = useMemo(() => {
    const peak = Math.max(...equityCurvePoints);
    return equityCurvePoints.map((eq) => {
      const dd = peak > 0 ? ((peak - eq) / peak) * 100 : 0;
      return dd;
    });
  }, [equityCurvePoints]);

  const tradeDistribution = useMemo(() => {
    const wins = tradeHistory.filter((t) => t.realized_pnl && parseFloat(t.realized_pnl) > 0).length;
    const losses = tradeHistory.filter((t) => t.realized_pnl && parseFloat(t.realized_pnl) <= 0).length;
    return { wins, losses, total: tradeHistory.length };
  }, [tradeHistory]);

  const svgPath = useMemo(() => {
    if (equityCurvePoints.length < 2) return '';
    const min = Math.min(...equityCurvePoints);
    const max = Math.max(...equityCurvePoints);
    const range = max - min || 1;
    const w = 800;
    const h = 200;
    const stepX = w / (equityCurvePoints.length - 1);
    return equityCurvePoints
      .map((p, i) => {
        const x = i * stepX;
        const y = h - ((p - min) / range) * (h - 20) - 10;
        return `${i === 0 ? 'M' : 'L'} ${x.toFixed(1)} ${y.toFixed(1)}`;
      })
      .join(' ');
  }, [equityCurvePoints]);

  const ddPath = useMemo(() => {
    if (drawdownPoints.length < 2) return '';
    const max = Math.max(...drawdownPoints);
    const min = 0;
    const range = max - min || 1;
    const w = 800;
    const h = 150;
    const stepX = w / (drawdownPoints.length - 1);
    return drawdownPoints
      .map((p, i) => {
        const x = i * stepX;
        const y = h - ((p - min) / range) * (h - 10) - 5;
        return `${i === 0 ? 'M' : 'L'} ${x.toFixed(1)} ${y.toFixed(1)}`;
      })
      .join(' ');
  }, [drawdownPoints]);

  const useResult = result || {
    backtest_id: 'bt_demo',
    final_equity: '145230.50',
    total_return_pct: 45.23,
    max_drawdown_pct: 12.5,
    sharpe_ratio: 1.85,
    total_trades: 156,
    win_rate: 58.3,
    profit_factor: 2.1,
    avg_trade_return: 1.2,
    daily_pnls: [],
    trades: [],
  };

  return (
    <Layout>
      <div className="flex-1 overflow-y-auto p-6 space-y-6">
        <div className="flex justify-between items-center">
          <h1 className="text-2xl font-bold text-slate-100">Backtest Results</h1>
          <div className="flex gap-2">
            <button
              onClick={() => navigate('/')}
              className="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-300 text-sm rounded transition-colors"
            >
              Back to Dashboard
            </button>
            <button
              onClick={() => navigate('/config')}
              className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white text-sm rounded transition-colors"
            >
              New Backtest
            </button>
          </div>
        </div>

        {/* Key Metrics */}
        <div className="grid grid-cols-4 gap-4">
          {[
            { label: 'Total Return', value: formatPercent(useResult.total_return_pct), color: useResult.total_return_pct >= 0 ? 'text-green-400' : 'text-red-400' },
            { label: 'Final Equity', value: formatCurrency(useResult.final_equity), color: 'text-blue-400' },
            { label: 'Sharpe Ratio', value: useResult.sharpe_ratio.toFixed(2), color: 'text-amber-400' },
            { label: 'Max Drawdown', value: formatPercent(-useResult.max_drawdown_pct), color: 'text-red-400' },
            { label: 'Total Trades', value: useResult.total_trades.toString(), color: 'text-slate-200' },
            { label: 'Win Rate', value: `${useResult.win_rate.toFixed(1)}%`, color: useResult.win_rate > 50 ? 'text-green-400' : 'text-red-400' },
            { label: 'Profit Factor', value: useResult.profit_factor.toFixed(2), color: 'text-blue-400' },
            { label: 'Avg Trade', value: `${useResult.avg_trade_return.toFixed(2)}%`, color: useResult.avg_trade_return >= 0 ? 'text-green-400' : 'text-red-400' },
          ].map((metric) => (
            <div key={metric.label} className="bg-slate-900 rounded border border-slate-800 p-4 text-center">
              <div className={`text-xl font-bold ${metric.color}`}>{metric.value}</div>
              <div className="text-[10px] text-slate-500 uppercase mt-1">{metric.label}</div>
            </div>
          ))}
        </div>

        {/* Equity Curve */}
        <div className="bg-slate-900 rounded border border-slate-800 p-4">
          <h3 className="text-sm font-bold text-slate-200 mb-3">Equity Curve</h3>
          <div className="w-full overflow-x-auto">
            <svg viewBox="0 0 800 200" className="w-full" style={{ minHeight: '150px' }}>
              <defs>
                <linearGradient id="equityGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor="#3b82f6" stopOpacity="0.3" />
                  <stop offset="100%" stopColor="#3b82f6" stopOpacity="0" />
                </linearGradient>
              </defs>
              <path d={svgPath} fill="none" stroke="#3b82f6" strokeWidth="2" />
              <path d={`${svgPath} L 800 200 L 0 200 Z`} fill="url(#equityGrad)" />
            </svg>
          </div>
        </div>

        {/* Drawdown Chart */}
        <div className="bg-slate-900 rounded border border-slate-800 p-4">
          <h3 className="text-sm font-bold text-slate-200 mb-3">Drawdown</h3>
          <div className="w-full overflow-x-auto">
            <svg viewBox="0 0 800 150" className="w-full" style={{ minHeight: '120px' }}>
              <path d={ddPath} fill="none" stroke="#ef4444" strokeWidth="2" />
            </svg>
          </div>
        </div>

        {/* Trade Distribution */}
        <div className="grid grid-cols-2 gap-4">
          <div className="bg-slate-900 rounded border border-slate-800 p-4">
            <h3 className="text-sm font-bold text-slate-200 mb-3">Trade Distribution</h3>
            <div className="flex items-center gap-4">
              <div className="relative w-24 h-24">
                <svg viewBox="0 0 100 100" className="w-full h-full -rotate-90">
                  {tradeDistribution.total > 0 && (
                    <>
                      <circle
                        cx="50"
                        cy="50"
                        r="40"
                        fill="none"
                        stroke="#22c55e"
                        strokeWidth="20"
                        strokeDasharray={`${(tradeDistribution.wins / tradeDistribution.total) * 251} 251`}
                      />
                      <circle
                        cx="50"
                        cy="50"
                        r="40"
                        fill="none"
                        stroke="#ef4444"
                        strokeWidth="20"
                        strokeDasharray={`${(tradeDistribution.losses / tradeDistribution.total) * 251} 251`}
                        strokeDashoffset={`${-(tradeDistribution.wins / tradeDistribution.total) * 251}`}
                      />
                    </>
                  )}
                  {tradeDistribution.total === 0 && (
                    <circle cx="50" cy="50" r="40" fill="none" stroke="#334155" strokeWidth="20" />
                  )}
                </svg>
              </div>
              <div className="space-y-2 text-xs">
                <div className="flex items-center gap-2">
                  <div className="w-3 h-3 rounded-full bg-green-500" />
                  <span className="text-slate-300">Wins: {tradeDistribution.wins}</span>
                </div>
                <div className="flex items-center gap-2">
                  <div className="w-3 h-3 rounded-full bg-red-500" />
                  <span className="text-slate-300">Losses: {tradeDistribution.losses}</span>
                </div>
                <div className="flex items-center gap-2">
                  <div className="w-3 h-3 rounded-full bg-slate-600" />
                  <span className="text-slate-300">Total: {tradeDistribution.total}</span>
                </div>
              </div>
            </div>
          </div>

          {/* Monthly Returns Table */}
          <div className="bg-slate-900 rounded border border-slate-800 p-4">
            <h3 className="text-sm font-bold text-slate-200 mb-3">Monthly Returns</h3>
            <div className="overflow-x-auto">
              <table className="w-full text-xs">
                <thead>
                  <tr className="text-slate-500 border-b border-slate-800">
                    <th className="px-2 py-1 text-left">Month</th>
                    <th className="px-2 py-1 text-right">Return</th>
                    <th className="px-2 py-1 text-right">Trades</th>
                  </tr>
                </thead>
                <tbody>
                  {['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun'].map((month, i) => {
                    const ret = [3.2, -1.5, 5.8, 2.1, -0.8, 4.5][i];
                    return (
                      <tr key={month} className="border-b border-slate-800/50">
                        <td className="px-2 py-1 text-slate-300">{month} 2024</td>
                        <td className={`px-2 py-1 text-right font-mono ${ret >= 0 ? 'text-green-400' : 'text-red-400'}`}>
                          {ret >= 0 ? '+' : ''}{ret.toFixed(1)}%
                        </td>
                        <td className="px-2 py-1 text-right text-slate-400">{[24, 18, 32, 28, 20, 34][i]}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </div>
    </Layout>
  );
}
