import { useAppStore } from '../../stores/useAppStore';
import type { Signal } from '../../types';

const ACTION_LABELS: Record<Signal['action'], string> = {
  open_long: 'LONG',
  open_short: 'SHORT',
  add_long: 'ADD LONG',
  add_short: 'ADD SHORT',
  reduce_long: 'RED LONG',
  reduce_short: 'RED SHORT',
  close_long: 'CLOSE LONG',
  close_short: 'CLOSE SHORT',
  close_all: 'CLOSE ALL',
};

const ACTION_COLORS: Record<Signal['action'], string> = {
  open_long: 'text-accent-green border-accent-green/30 bg-accent-green/10',
  open_short: 'text-accent-red border-accent-red/30 bg-accent-red/10',
  add_long: 'text-accent-green border-accent-green/30 bg-accent-green/10',
  add_short: 'text-accent-red border-accent-red/30 bg-accent-red/10',
  reduce_long: 'text-accent-amber border-accent-amber/30 bg-accent-amber/10',
  reduce_short: 'text-accent-amber border-accent-amber/30 bg-accent-amber/10',
  close_long: 'text-accent-cyan border-accent-cyan/30 bg-accent-cyan/10',
  close_short: 'text-accent-cyan border-accent-cyan/30 bg-accent-cyan/10',
  close_all: 'text-text-secondary border-border-strong bg-surface-raised',
};

export function SignalsPanel() {
  const { signals, markerVisibility, setMarkerVisibility } = useAppStore();

  const recentSignals = signals.slice(0, 20);

  return (
    <div className="panel flex-1 flex flex-col min-h-0">
      <div className="panel-header">
        <span className="panel-title">Signals</span>
        <label className="flex items-center gap-1.5 cursor-pointer">
          <input
            type="checkbox"
            checked={markerVisibility}
            onChange={(e) => setMarkerVisibility(e.target.checked)}
            className="w-3 h-3 rounded border-border-default bg-surface-raised
                       checked:bg-accent-cyan checked:border-accent-cyan"
          />
          <span className="text-2xs text-text-muted">SHOW</span>
        </label>
      </div>

      <div className="flex-1 overflow-y-auto panel-body">
        {recentSignals.length === 0 ? (
          <div className="text-center text-text-muted text-xs py-8">
            No signals yet
          </div>
        ) : (
          <div className="space-y-1.5">
            {recentSignals.map((signal, index) => (
              <SignalRow key={`${signal.timestamp}-${index}`} signal={signal} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function SignalRow({ signal }: { signal: Signal }) {
  const actionColorClass = ACTION_COLORS[signal.action];
  const timestamp = new Date(signal.timestamp * 1000);
  const timeStr = timestamp.toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  });

  return (
    <div className={`flex items-center justify-between p-2 rounded border ${actionColorClass}`}>
      <div className="flex items-center gap-2">
        <span className="text-2xs font-semibold">
          {ACTION_LABELS[signal.action]}
        </span>
        <span className="text-2xs text-text-muted font-mono">
          {signal.symbol}
        </span>
      </div>
      <div className="flex items-center gap-2">
        <span className="text-2xs text-text-muted font-mono">{timeStr}</span>
        {signal.strength !== undefined && (
          <span className="text-2xs font-mono tabular-nums">
            {signal.strength.toFixed(2)}
          </span>
        )}
      </div>
    </div>
  );
}