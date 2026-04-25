import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { WsMessage, EngineSnapshot, StandardBar, Signal, OrderFill } from '../types';

// Mock WebSocket for testing
class MockWebSocket {
  url: string;
  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  readyState = 0; // CONNECTING
  sentMessages: string[] = [];

  constructor(url: string) {
    this.url = url;
    // Simulate connection opening
    setTimeout(() => {
      this.readyState = 1; // OPEN
      if (this.onopen) {
        this.onopen(new Event('open'));
      }
    }, 0);
  }

  send(data: string) {
    this.sentMessages.push(data);
  }

  close() {
    this.readyState = 3; // CLOSED
    if (this.onclose) {
      this.onclose(new CloseEvent('close'));
    }
  }

  // Test helper: simulate receiving a message from server
  simulateMessage(data: WsMessage) {
    if (this.onmessage) {
      this.onmessage(new MessageEvent('message', { data: JSON.stringify(data) }));
    }
  }
}

// Replace global WebSocket with mock
global.WebSocket = MockWebSocket as any;

describe('WebSocket Message Handling', () => {
  let ws: MockWebSocket;

  beforeEach(() => {
    ws = new MockWebSocket('ws://localhost:8081/ws');
  });

  afterEach(() => {
    ws.close();
  });

  it('connects successfully', (done) => {
    ws.onopen = (event) => {
      expect(ws.readyState).toBe(1);
      expect(event.type).toBe('open');
      done();
    };
  });

  it('receives snapshot message', (done) => {
    const snapshot: EngineSnapshot = {
      timestamp: 1704067200000,
      current_bar: {
        timestamp: 1704067200000,
        open: '42000',
        high: '42500',
        low: '41800',
        close: '42350',
        volume: '100',
        symbol: 'BTC-USDT',
        exchange: 'binance',
        confirmed: true,
      },
      equity: '100050.50',
      available_balance: '90000',
      margin_used: '10050.50',
      margin_ratio: '0.1005',
      unrealized_pnl: '50.50',
      realized_pnl_today: '0',
      positions: [],
      total_trades: 5,
      win_rate: 60.0,
      max_drawdown_pct: 2.5,
      sharpe_ratio: 1.2,
    };

    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data) as WsMessage;
      expect(msg.type).toBe('snapshot');
      expect(msg.data).toEqual(snapshot);
      done();
    };

    ws.simulateMessage({ type: 'snapshot', data: snapshot });
  });

  it('receives bar_update message', (done) => {
    const bar: StandardBar = {
      timestamp: 1704070800000,
      open: '42350',
      high: '42800',
      low: '42200',
      close: '42600',
      volume: '150',
      symbol: 'BTC-USDT',
      exchange: 'binance',
      confirmed: true,
    };

    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data) as WsMessage;
      expect(msg.type).toBe('bar_update');
      expect(msg.bar).toEqual(bar);
      done();
    };

    ws.simulateMessage({ type: 'bar_update', bar });
  });

  it('receives trade fill message', (done) => {
    const fill: OrderFill = {
      order_id: 'ord_123',
      position_id: 'pos_456',
      symbol: 'BTC-USDT',
      side: 'Buy',
      direction: 'Long',
      filled_price: '42350',
      filled_quantity: '0.1',
      fee: '0.42',
      fee_asset: 'USDT',
      timestamp: 1704067200000,
      realized_pnl: '0',
    };

    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data) as WsMessage;
      expect(msg.type).toBe('trade');
      expect(msg.fill).toEqual(fill);
      done();
    };

    ws.simulateMessage({ type: 'trade', fill });
  });

  it('receives signal message', (done) => {
    const signal: Signal = {
      action: 'open_long',
      symbol: 'BTC-USDT',
      quantity: '0.1',
      strength: 0.85,
      reason: 'EMA cross detected',
      timestamp: 1704067200000,
    };

    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data) as WsMessage;
      expect(msg.type).toBe('signal');
      expect(msg.signal).toEqual(signal);
      done();
    };

    ws.simulateMessage({ type: 'signal', signal });
  });

  it('receives complete message with backtest result', (done) => {
    const result = {
      backtest_id: 'bt_test_123',
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

    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data) as WsMessage;
      expect(msg.type).toBe('complete');
      expect(msg.result).toEqual(result);
      done();
    };

    ws.simulateMessage({ type: 'complete', result });
  });

  it('receives error message', (done) => {
    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data) as WsMessage;
      expect(msg.type).toBe('error');
      expect(msg.message).toBe('Invalid backtest ID');
      done();
    };

    ws.simulateMessage({ type: 'error', message: 'Invalid backtest ID' });
  });

  it('sends subscribe message correctly', () => {
    const subscribeMsg = {
      type: 'subscribe',
      channel: 'backtest_state',
      backtest_id: 'bt_test_123',
    };
    ws.send(JSON.stringify(subscribeMsg));
    expect(ws.sentMessages).toHaveLength(1);
    expect(JSON.parse(ws.sentMessages[0])).toEqual(subscribeMsg);
  });

  it('sends control play message correctly', () => {
    const controlMsg = {
      type: 'control',
      action: 'play',
      backtest_id: 'bt_test_123',
    };
    ws.send(JSON.stringify(controlMsg));
    expect(ws.sentMessages).toHaveLength(1);
    expect(JSON.parse(ws.sentMessages[0])).toEqual(controlMsg);
  });

  it('sends control pause message correctly', () => {
    const controlMsg = {
      type: 'control',
      action: 'pause',
      backtest_id: 'bt_test_123',
    };
    ws.send(JSON.stringify(controlMsg));
    expect(JSON.parse(ws.sentMessages[0])).toEqual(controlMsg);
  });

  it('sends control set_speed message correctly', () => {
    const controlMsg = {
      type: 'control',
      action: 'set_speed',
      speed: 5.0,
    };
    ws.send(JSON.stringify(controlMsg));
    expect(JSON.parse(ws.sentMessages[0])).toEqual(controlMsg);
  });

  it('handles connection close gracefully', (done) => {
    ws.onclose = (event) => {
      expect(ws.readyState).toBe(3);
      expect(event.type).toBe('close');
      done();
    };
    ws.close();
  });
});
