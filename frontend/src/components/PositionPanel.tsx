import { useAppStore } from '../stores/useAppStore';

function formatPrice(val: string): string {
  const n = parseFloat(val);
  return isNaN(n) ? '--' : n.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

function formatPnl(val: string): string {
  const n = parseFloat(val);
  if (isNaN(n)) return '--';
  const sign = n >= 0 ? '+' : '';
  return `${sign}${n.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

export default function PositionPanel() {
  const positions = useAppStore((s) => s.snapshot?.positions ?? []);

  return (
    <div className="bg-slate-900 rounded border border-slate-800 overflow-hidden">
      <div className="px-3 py-2 border-b border-slate-800 flex justify-between items-center">
        <h3 className="text-sm font-bold text-slate-200">Positions</h3>
        <span className="text-xs text-slate-500">{positions.length} open</span>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-xs">
          <thead>
            <tr className="text-slate-500 border-b border-slate-800">
              <th className="px-2 py-1.5 text-left font-medium">Symbol</th>
              <th className="px-2 py-1.5 text-left font-medium">Dir</th>
              <th className="px-2 py-1.5 text-right font-medium">Size</th>
              <th className="px-2 py-1.5 text-right font-medium">Entry</th>
              <th className="px-2 py-1.5 text-right font-medium">Unreal PnL</th>
            </tr>
          </thead>
          <tbody>
            {positions.length === 0 && (
              <tr>
                <td colSpan={5} className="px-2 py-4 text-center text-slate-600 italic">
                  No open positions
                </td>
              </tr>
            )}
            {positions.map((pos) => (
              <tr key={pos.id} className="border-b border-slate-800/50 hover:bg-slate-800/50 transition-colors">
                <td className="px-2 py-1.5 text-slate-200 font-medium">{pos.symbol}</td>
                <td className="px-2 py-1.5">
                  <span
                    className={`inline-flex px-1.5 py-0.5 rounded text-[10px] font-bold ${
                      pos.direction === 'Long'
                        ? 'bg-green-500/20 text-green-400'
                        : 'bg-red-500/20 text-red-400'
                    }`}
                  >
                    {pos.direction}
                  </span>
                </td>
                <td className="px-2 py-1.5 text-right text-slate-300">{pos.current_size}</td>
                <td className="px-2 py-1.5 text-right text-slate-300">{formatPrice(pos.average_entry_price)}</td>
                <td
                  className={`px-2 py-1.5 text-right font-mono ${
                    parseFloat(pos.unrealized_pnl) >= 0 ? 'text-green-400' : 'text-red-400'
                  }`}
                >
                  {formatPnl(pos.unrealized_pnl)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
