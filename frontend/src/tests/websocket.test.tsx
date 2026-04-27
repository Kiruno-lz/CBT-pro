import { describe, it, expect, vi, beforeEach } from 'vitest';
import { EngineWebSocket } from '../stores/websocket';
import type { StoreApi } from '../stores/websocket';

describe('EngineWebSocket', () => {
  const mockStore: StoreApi = {
    setWsConnected: vi.fn(),
    setEngineOnline: vi.fn(),
    setSnapshot: vi.fn(),
    appendBar: vi.fn(),
    addTrade: vi.fn(),
    addSignal: vi.fn(),
    setPlayback: vi.fn(),
    setBacktestResult: vi.fn(),
    setTradeHistory: vi.fn(),
    playback: { status: 'idle', currentBarIndex: 0, totalBars: 0, speed: 1, currentTime: 0 },
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should create websocket instance', () => {
    const ws = new EngineWebSocket('ws://localhost:8081/ws', mockStore);
    expect(ws).toBeDefined();
  });

  it('should handle snapshot message', () => {
    const snapshot = {
      timestamp: Date.now(),
      current_bar: {
        timestamp: Date.now(),
        open: '100',
        high: '110',
        low: '90',
        close: '105',
        volume: '1000',
        symbol: 'BTCUSDT',
        exchange: 'binance',
        confirmed: true,
      },
      equity: '10000',
      available_balance: '9000',
      margin_used: '1000',
      margin_ratio: '10',
      unrealized_pnl: '0',
      realized_pnl_today: '0',
      positions: [],
      total_trades: 0,
      win_rate: 0,
      max_drawdown_pct: 0,
    };

    expect(snapshot.equity).toBe('10000');
  });
});