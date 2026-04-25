import { useCallback } from 'react';
import { useAppStore } from '../stores/useAppStore';

export default function IndicatorConfigSidebar() {
  const indicators = useAppStore((s) => s.indicators);
  const updateIndicator = useAppStore((s) => s.updateIndicator);
  const markerVisibility = useAppStore((s) => s.markerVisibility);
  const setMarkerVisibility = useAppStore((s) => s.setMarkerVisibility);

  const handleToggle = useCallback(
    (name: string) => {
      const ind = indicators.find((i) => i.name === name);
      if (ind) {
        updateIndicator(name, { visible: !ind.visible });
      }
    },
    [indicators, updateIndicator]
  );

  return (
    <div className="p-3 space-y-4">
      <h3 className="text-sm font-bold text-slate-200">Indicators</h3>

      <div className="space-y-2">
        {indicators.map((ind) => (
          <div key={ind.name} className="flex items-center justify-between">
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={ind.visible}
                onChange={() => handleToggle(ind.name)}
                className="rounded border-slate-600 bg-slate-800 text-blue-500 focus:ring-blue-500 focus:ring-offset-0"
              />
              <span className="text-xs text-slate-300">{ind.name.replace(/_/g, ' ').toUpperCase()}</span>
            </label>
            <span className="text-[10px] text-slate-500">{ind.panel}</span>
          </div>
        ))}
      </div>

      <div className="border-t border-slate-800 pt-3">
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={markerVisibility}
            onChange={() => setMarkerVisibility(!markerVisibility)}
            className="rounded border-slate-600 bg-slate-800 text-blue-500 focus:ring-blue-500 focus:ring-offset-0"
          />
          <span className="text-xs text-slate-300">Trade Markers</span>
        </label>
      </div>

      <div className="border-t border-slate-800 pt-3">
        <h4 className="text-xs font-semibold text-slate-400 mb-2">Strategy</h4>
        <select className="w-full bg-slate-800 border border-slate-700 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none">
          <option>EMA Cross v1</option>
          <option>RSI Reversal</option>
          <option>Bollinger Breakout</option>
          <option>MACD Momentum</option>
        </select>
      </div>

      <div className="border-t border-slate-800 pt-3">
        <h4 className="text-xs font-semibold text-slate-400 mb-2">Timeframe</h4>
        <div className="grid grid-cols-4 gap-1">
          {['M1', 'M5', 'M15', 'M30', 'H1', 'H4', 'D1', 'W1'].map((tf) => (
            <button
              key={tf}
              className="px-1 py-0.5 rounded bg-slate-800 text-[10px] text-slate-400 hover:bg-slate-700 hover:text-slate-200 transition-colors"
            >
              {tf}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
