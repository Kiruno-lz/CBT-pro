import { useEffect, useState, useRef, type RefObject } from 'react';
import { EngineWebSocket } from './stores/websocket';
import { useAppStore } from './stores/useAppStore';
import { MainLayout } from './components/Layout/MainLayout';

const WS_URL = import.meta.env.VITE_WS_URL || 'ws://localhost:8081/ws';

export default function App() {
  const wsRef = useRef<EngineWebSocket | null>(null);
  const [initialized, setInitialized] = useState(false);

  useEffect(() => {
    const store = useAppStore.getState();
    wsRef.current = new EngineWebSocket(WS_URL, {
      setWsConnected: store.setWsConnected,
      setEngineOnline: store.setEngineOnline,
      setSnapshot: store.setSnapshot,
      appendBar: store.appendBar,
      addTrade: store.addTrade,
      addSignal: store.addSignal,
      setPlayback: store.setPlayback,
      setBacktestResult: store.setBacktestResult,
      setTradeHistory: store.setTradeHistory,
      playback: store.playback,
    });
    wsRef.current.connect();
    setInitialized(true);

    return () => {
      wsRef.current?.disconnect();
    };
  }, []);

  if (!initialized) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="text-text-muted text-sm">Initializing...</div>
      </div>
    );
  }

  return <MainLayout wsRef={wsRef as RefObject<EngineWebSocket | null>} />;
}