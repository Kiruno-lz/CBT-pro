import { useState, useCallback, useMemo } from 'react';
import { useAppStore } from '../stores/useAppStore';

function formatPrice(val: string): string {
  const n = parseFloat(val);
  return isNaN(n) ? '--' : n.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

function formatTimestamp(ts: number): string {
  return new Date(ts).toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}

export default function OrderHistoryPanel() {
  const tradeHistory = useAppStore((s) => s.tradeHistory);
  const [symbolFilter, setSymbolFilter] = useState('');
  const [directionFilter, setDirectionFilter] = useState<'All' | 'Long' | 'Short'>('All');

  const symbols = useMemo(() => {
    const s = new Set(tradeHistory.map((t) => t.symbol));
    return Array.from(s);
  }, [tradeHistory]);

  const filtered = useMemo(() => {
    return tradeHistory.filter((t) => {
      const symbolMatch = !symbolFilter || t.symbol === symbolFilter;
      const dirMatch = directionFilter === 'All' || t.direction === directionFilter;
      return symbolMatch && dirMatch;
    });
  }, [tradeHistory, symbolFilter, directionFilter]);

  const handleExport = useCallback(() => {
    const headers = ['Order ID', 'Symbol', 'Side', 'Direction', 'Price', 'Quantity', 'Fee', 'Timestamp', 'Realized PnL'];
    const rows = filtered.map((t) => [
      t.order_id,
      t.symbol,
      t.side,
      t.direction,
      t.filled_price,
      t.filled_quantity,
      t.fee,
      new Date(t.timestamp).toISOString(),
      t.realized_pnl ?? '',
    ]);
    const csv = [headers.join(','), ...rows.map((r) => r.join(','))].join('\n');
    const blob = new Blob([csv], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `trades_${Date.now()}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }, [filtered]);

  return (
    <div className="bg-slate-900 rounded border border-slate-800 overflow-hidden flex flex-col">
      <div className="px-3 py-2 border-b border-slate-800 flex flex-wrap gap-2 items-center justify-between">
        <h3 className="text-sm font-bold text-slate-200">Order History</h3>
        <div className="flex gap-2">
          <select
            value={symbolFilter}
            onChange={(e) => setSymbolFilter(e.target.value)}
            className="bg-slate-800 border border-slate-700 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none"
          >
            <option value="">All Symbols</option>
            {symbols.map((s) => (
              <option key={s} value={s}>
                {s}
              </option>
            ))}
          </select>
          <select
            value={directionFilter}
            onChange={(e) => setDirectionFilter(e.target.value as 'All' | 'Long' | 'Short')}
            className="bg-slate-800 border border-slate-700 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none"
          >
            <option value="All">All Directions</option>
            <option value="Long">Long</option>
            <option value="Short">Short</option>
          </select>
          <button
            onClick={handleExport}
            className="px-2 py-1 rounded bg-slate-800 hover:bg-slate-700 text-xs text-slate-300 transition-colors"
          >
            Export CSV
          </button>
        </div>
      </div>
      <div className="overflow-x-auto flex-1 max-h-64">
        <table className="w-full text-xs">
          <thead className="sticky top-0 bg-slate-900 z-10">
            <tr className="text-slate-500 border-b border-slate-800">
              <th className="px-2 py-1.5 text-left font-medium">Time</th>
              <th className="px-2 py-1.5 text-left font-medium">Symbol</th>
              <th className="px-2 py-1.5 text-left font-medium">Side</th>
              <th className="px-2 py-1.5 text-right font-medium">Price</th>
              <th className="px-2 py-1.5 text-right font-medium">Qty</th>
              <th className="px-2 py-1.5 text-right font-medium">Fee</th>
              <th className="px-2 py-1.5 text-right font-medium">Realized PnL</th>
            </tr>
          </thead>
          <tbody>
            {filtered.length === 0 && (
              <tr>
                <td colSpan={7} className="px-2 py-4 text-center text-slate-600 italic">
                  No trades recorded
                </td>
              </tr>
            )}
            {filtered.map((trade, idx) => (
              <tr
                key={`${trade.order_id}-${idx}`}
                className="border-b border-slate-800/50 hover:bg-slate-800/50 transition-colors"
              >
                <td className="px-2 py-1.5 text-slate-400 whitespace-nowrap">{formatTimestamp(trade.timestamp)}</td>
                <td className="px-2 py-1.5 text-slate-200 font-medium">{trade.symbol}</td>
                <td className="px-2 py-1.5">
                  <span
                    className={`inline-flex px-1.5 py-0.5 rounded text-[10px] font-bold ${
                      trade.side === 'Buy'
                        ? 'bg-green-500/20 text-green-400'
                        : 'bg-red-500/20 text-red-400'
                    }`}
                  >
                    {trade.side}
                  </span>
                </td>
                <td className="px-2 py-1.5 text-right text-slate-300">{formatPrice(trade.filled_price)}</td>
                <td className="px-2 py-1.5 text-right text-slate-300">{trade.filled_quantity}</td>
                <td className="px-2 py-1.5 text-right text-slate-400">{trade.fee}</td>
                <td className="px-2 py-1.5 text-right font-mono">
                  {trade.realized_pnl ? (
                    <span className={parseFloat(trade.realized_pnl) >= 0 ? 'text-green-400' : 'text-red-400'}>
                      {parseFloat(trade.realized_pnl) >= 0 ? '+' : ''}
                      {formatPrice(trade.realized_pnl)}
                    </span>
                  ) : (
                    <span className="text-slate-600">--</span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
