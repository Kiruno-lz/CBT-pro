import { useEffect, useState, useRef, useCallback } from 'react';
import Layout from '../components/Layout';
import ChartWrapper from '../charting/ChartWrapper';
import PlaybackControls from '../components/PlaybackControls';
import SignalDashboard from '../components/SignalDashboard';
import PositionPanel from '../components/PositionPanel';
import OrderHistoryPanel from '../components/OrderHistoryPanel';
import IndicatorConfigSidebar from '../components/IndicatorConfigSidebar';
import { useAppStore } from '../stores/useAppStore';
import { EngineWebSocket } from '../stores/websocket';
import { MockEngineWebSocket } from '../mocks/mockEngine';
import { MOCK_BARS } from '../mocks/mockBars';
import { MOCK_SNAPSHOT } from '../mocks/mockSnapshot';
import { MOCK_SIGNALS } from '../mocks/mockSignals';
import type { StoreApi } from '../stores/websocket';

export default function Dashboard() {
  const bars = useAppStore((s) => s.bars);
  const setBars = useAppStore((s) => s.setBars);
  const setSnapshot = useAppStore((s) => s.setSnapshot);
  const setPlayback = useAppStore((s) => s.setPlayback);
  const addTrade = useAppStore((s) => s.addTrade);
  const addSignal = useAppStore((s) => s.addSignal);
  const setWsConnected = useAppStore((s) => s.setWsConnected);
  const setEngineOnline = useAppStore((s) => s.setEngineOnline);
  const playback = useAppStore((s) => s.playback);
  const wsRef = useRef<EngineWebSocket | null>(null);
  const [useMock, setUseMock] = useState(true);

  // Initialize with mock data on mount
  useEffect(() => {
    if (bars.length === 0) {
      setBars(MOCK_BARS.slice(0, 100));
      setSnapshot(MOCK_SNAPSHOT);
      setPlayback({ totalBars: MOCK_BARS.length, currentBarIndex: 100, status: 'paused' });
      MOCK_SIGNALS.forEach((s) => addSignal(s));
    }
  }, []);

  // Initialize WebSocket
  useEffect(() => {
    const storeApi: StoreApi = {
      setWsConnected,
      setEngineOnline,
      setSnapshot,
      appendBar: useAppStore.getState().appendBar,
      addTrade,
      addSignal,
      setPlayback,
      setTradeHistory: useAppStore.getState().setTradeHistory,
      setBacktestResult: useAppStore.getState().setBacktestResult,
      playback: useAppStore.getState().playback,
    };

    if (useMock) {
      const mock = new MockEngineWebSocket({ intervalMs: 500, speed: playback.speed });
      const handler = (event: { data: string }) => {
        try {
          const msg = JSON.parse(event.data);
          switch (msg.type) {
            case 'snapshot':
              setSnapshot(msg.data);
              setEngineOnline(true);
              break;
            case 'bar_update':
              useAppStore.getState().appendBar(msg.bar);
              break;
            case 'trade':
              addTrade(msg.fill);
              break;
            case 'signal':
              addSignal(msg.signal);
              break;
            case 'complete':
              setPlayback({ status: 'complete' });
              useAppStore.getState().setBacktestResult(msg.result);
              break;
          }
        } catch {
          // ignore
        }
      };
      mock.addEventListener('message', handler);
      mock.addEventListener('open', () => setWsConnected(true));
      mock.addEventListener('close', () => setWsConnected(false));
      wsRef.current = mock as unknown as EngineWebSocket;
      setWsConnected(true);
    } else {
      const ws = new EngineWebSocket('ws://localhost:8081/ws', storeApi);
      ws.connect();
      wsRef.current = ws;
    }

    return () => {
      wsRef.current?.disconnect?.();
      wsRef.current?.close?.();
    };
  }, [useMock]);

  const handleControl = useCallback((action: 'play' | 'pause' | 'step_forward' | 'step_backward') => {
    wsRef.current?.sendControl?.(action);
    wsRef.current?.send?.(JSON.stringify({ type: 'control', action }));
  }, []);

  const handleSpeedChange = useCallback((speed: number) => {
    wsRef.current?.setSpeed?.(speed);
    wsRef.current?.send?.(JSON.stringify({ type: 'control', action: 'set_speed', speed }));
  }, []);

  const handleSeek = useCallback((index: number) => {
    // Not directly supported by mock, but can be implemented
    setPlayback({ currentBarIndex: index });
  }, [setPlayback]);

  return (
    <Layout
      sidebar={<IndicatorConfigSidebar />}
      rightPanel={<SignalDashboard />}
    >
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Equity Bar */}
        <div className="h-10 bg-slate-900 border-b border-slate-800 flex items-center px-4 gap-6 shrink-0">
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-slate-500 uppercase">Equity</span>
            <span className="text-sm font-bold text-slate-200">
              {useAppStore.getState().snapshot?.equity ? `$${parseFloat(useAppStore.getState().snapshot!.equity).toLocaleString()}` : '--'}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-slate-500 uppercase">Unreal PnL</span>
            <span className={`text-sm font-bold ${parseFloat(useAppStore.getState().snapshot?.unrealized_pnl ?? '0') >= 0 ? 'text-green-400' : 'text-red-400'}`}>
              {useAppStore.getState().snapshot?.unrealized_pnl ?? '--'}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-slate-500 uppercase">Realized Today</span>
            <span className="text-sm font-bold text-blue-400">
              {useAppStore.getState().snapshot?.realized_pnl_today ?? '--'}
            </span>
          </div>
          <div className="ml-auto flex items-center gap-2">
            <label className="flex items-center gap-1.5 text-xs text-slate-400 cursor-pointer">
              <input
                type="checkbox"
                checked={useMock}
                onChange={() => setUseMock(!useMock)}
                className="rounded border-slate-600 bg-slate-800 text-blue-500"
              />
              Mock Engine
            </label>
          </div>
        </div>

        {/* Chart Area */}
        <div className="flex-1 min-h-0">
          <ChartWrapper bars={bars} />
        </div>

        {/* Bottom Panels */}
        <div className="h-48 grid grid-cols-2 gap-0 border-t border-slate-800 shrink-0">
          <div className="overflow-hidden">
            <PositionPanel />
          </div>
          <div className="overflow-hidden border-l border-slate-800">
            <OrderHistoryPanel />
          </div>
        </div>
      </div>

      {/* Playback Controls */}
      <PlaybackControls
        onControl={handleControl}
        onSpeedChange={handleSpeedChange}
        onSeek={handleSeek}
      />
    </Layout>
  );
}
