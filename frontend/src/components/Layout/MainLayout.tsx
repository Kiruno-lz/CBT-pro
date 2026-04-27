import { type RefObject } from 'react';
import { EngineWebSocket } from '../../stores/websocket';
import { useAppStore } from '../../stores/useAppStore';
import { KLineChart } from '../Chart/KLineChart';
import { PlaybackPanel } from '../Playback/PlaybackPanel';
import { AccountDashboard } from '../Dashboard/AccountDashboard';
import { SignalsPanel } from '../Signals/SignalsPanel';
import { BacktestConfig } from '../Backtest/BacktestConfig';
import { BacktestResult } from '../Backtest/BacktestResult';

interface MainLayoutProps {
  wsRef: RefObject<EngineWebSocket | null>;
}

export function MainLayout({ wsRef }: MainLayoutProps) {
  const { playback, backtestResult } = useAppStore();

  return (
    <div className="h-screen flex flex-col overflow-hidden">
      <header className="h-10 bg-surface-base border-b border-border-subtle flex items-center justify-between px-4">
        <div className="flex items-center gap-3">
          <h1 className="text-sm font-semibold text-text-primary tracking-tight">CBT-Pro</h1>
          <span className="text-2xs text-text-muted font-mono">QUANTITATIVE BACKTEST ENGINE</span>
        </div>
        <StatusBar wsRef={wsRef} />
      </header>

      <main className="flex-1 flex gap-0 p-3 overflow-hidden">
        <div className="flex-1 flex flex-col gap-3 min-w-0">
          <div className="flex-1 min-h-0">
            <KLineChart />
          </div>
        </div>

        <div className="w-72 flex-shrink-0 flex flex-col gap-3 ml-3 overflow-hidden">
          {playback.status === 'idle' && !backtestResult ? (
            <>
              <BacktestConfig wsRef={wsRef} />
              <AccountDashboard />
            </>
          ) : (
            <>
              <PlaybackPanel wsRef={wsRef} />
              <AccountDashboard />
              <SignalsPanel />
            </>
          )}
          {backtestResult && <BacktestResult />}
        </div>
      </main>
    </div>
  );
}

function StatusBar({ wsRef }: { wsRef: RefObject<EngineWebSocket | null> }) {
  const { wsConnected, engineOnline, playback, reset } = useAppStore();

  const handleReset = () => {
    if (confirm('Reset all data and start fresh?')) {
      reset();
      wsRef.current?.disconnect();
      setTimeout(() => wsRef.current?.connect(), 100);
    }
  };

  return (
    <div className="flex items-center gap-4">
      <StatusIndicator label="WS" active={wsConnected} activeColor="bg-accent-cyan" />
      <StatusIndicator label="Engine" active={engineOnline} activeColor="bg-accent-green" />
      <StatusIndicator
        label={playback.status.toUpperCase()}
        active={playback.status !== 'idle'}
        activeColor="bg-accent-amber"
      />
      <button onClick={handleReset} className="btn-ghost text-2xs">RESET</button>
    </div>
  );
}

function StatusIndicator({
  label,
  active,
  activeColor,
}: {
  label: string;
  active: boolean;
  activeColor: string;
}) {
  return (
    <div className="flex items-center gap-1.5">
      <div className={`w-1.5 h-1.5 rounded-full transition-colors ${active ? activeColor : 'bg-surface-overlay'}`} />
      <span className="text-2xs font-mono text-text-muted">{label}</span>
    </div>
  );
}