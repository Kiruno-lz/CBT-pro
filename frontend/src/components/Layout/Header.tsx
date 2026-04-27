import { type RefObject } from 'react';
import { EngineWebSocket } from '../../stores/websocket';
import { useAppStore } from '../../stores/useAppStore';

interface HeaderProps {
  wsRef: RefObject<EngineWebSocket | null>;
}

export function Header({ wsRef }: HeaderProps) {
  const { wsConnected, engineOnline, playback, reset } = useAppStore();

  const handleReset = () => {
    if (confirm('Reset all data and start fresh?')) {
      reset();
      wsRef.current?.disconnect();
      setTimeout(() => wsRef.current?.connect(), 100);
    }
  };

  return (
    <header className="h-10 bg-surface-base border-b border-border-subtle flex items-center justify-between px-4">
      <div className="flex items-center gap-3">
        <h1 className="text-sm font-semibold text-text-primary tracking-tight">
          CBT-Pro
        </h1>
        <span className="text-2xs text-text-muted font-mono">
          QUANTITATIVE BACKTEST ENGINE
        </span>
      </div>

      <div className="flex items-center gap-4">
        <StatusIndicator
          label="WS"
          active={wsConnected}
          activeColor="bg-accent-cyan"
        />
        <StatusIndicator
          label="Engine"
          active={engineOnline}
          activeColor="bg-accent-green"
        />
        <StatusIndicator
          label={playback.status.toUpperCase()}
          active={playback.status !== 'idle'}
          activeColor="bg-accent-amber"
        />

        <button
          onClick={handleReset}
          className="btn-ghost text-2xs"
          title="Reset Session"
        >
          RESET
        </button>
      </div>
    </header>
  );
}

interface StatusIndicatorProps {
  label: string;
  active: boolean;
  activeColor: string;
}

function StatusIndicator({ label, active, activeColor }: StatusIndicatorProps) {
  return (
    <div className="flex items-center gap-1.5">
      <div
        className={`w-1.5 h-1.5 rounded-full transition-colors ${
          active ? activeColor : 'bg-surface-overlay'
        }`}
      />
      <span className="text-2xs font-mono text-text-muted">{label}</span>
    </div>
  );
}