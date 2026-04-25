import type { WsMessage, StandardBar, EngineSnapshot, Signal, OrderFill } from '../types';
import { MOCK_BARS } from './mockBars';
import { MOCK_SNAPSHOT } from './mockSnapshot';
import { MOCK_SIGNALS } from './mockSignals';

interface MockEngineOptions {
  intervalMs?: number;
  speed?: number;
}

export class MockEngineWebSocket {
  private listeners: Array<(msg: WsMessage) => void> = [];
  private timer: ReturnType<typeof setInterval> | null = null;
  private running = false;
  private barIndex = 0;
  private intervalMs: number;
  private speed: number;

  constructor(options: MockEngineOptions = {}) {
    this.intervalMs = options.intervalMs ?? 1000;
    this.speed = options.speed ?? 1;
  }

  addEventListener(_type: 'message', handler: (event: { data: string }) => void): void;
  addEventListener(_type: 'open', handler: () => void): void;
  addEventListener(_type: 'close', handler: () => void): void;
  addEventListener(
    _type: string,
    handler: (() => void) | ((event: { data: string }) => void)
  ): void {
    if (_type === 'message') {
      this.listeners.push((msg) => {
        (handler as (event: { data: string }) => void)({ data: JSON.stringify(msg) });
      });
    }
    if (_type === 'open') {
      (handler as () => void)();
    }
  }

  removeEventListener(_type: string, _handler: unknown): void {
    // Simplified for mock
  }

  send(data: string): void {
    try {
      const parsed = JSON.parse(data) as Record<string, unknown>;
      if (parsed.type === 'subscribe') {
        this.start();
      }
      if (parsed.type === 'control') {
        const action = parsed.action as string;
        if (action === 'pause') this.pause();
        if (action === 'play') this.start();
        if (action === 'step_forward') this.stepForward();
        if (action === 'step_backward') this.stepBackward();
        if (action === 'set_speed') {
          this.speed = (parsed.speed as number) ?? 1;
        }
      }
    } catch {
      // ignore
    }
  }

  close(): void {
    this.running = false;
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  private start(): void {
    if (this.running) return;
    this.running = true;
    this.timer = setInterval(() => {
      this.tick();
    }, this.intervalMs / this.speed);
  }

  private pause(): void {
    this.running = false;
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  private stepForward(): void {
    this.tick();
  }

  private stepBackward(): void {
    if (this.barIndex > 0) {
      this.barIndex--;
      this.emitBar(this.barIndex);
    }
  }

  private tick(): void {
    if (this.barIndex >= MOCK_BARS.length) {
      this.emitComplete();
      this.pause();
      return;
    }
    this.emitBar(this.barIndex);
    this.barIndex++;
  }

  private emitBar(index: number): void {
    const bar = MOCK_BARS[index];
    const snapshot = this.buildSnapshot(bar, index);
    this.emit({ type: 'snapshot', data: snapshot });
    this.emit({ type: 'bar_update', bar });

    if (index % 50 === 0 && index < MOCK_SIGNALS.length * 50) {
      const sigIdx = Math.floor(index / 50);
      if (sigIdx < MOCK_SIGNALS.length) {
        const sig = { ...MOCK_SIGNALS[sigIdx], timestamp: bar.timestamp };
        this.emit({ type: 'signal', signal: sig });
        this.emit({ type: 'trade', fill: this.buildFill(sig, bar) });
      }
    }
  }

  private buildSnapshot(bar: StandardBar, index: number): EngineSnapshot {
    const equity = 100000 + index * 22.5 + Math.sin(index * 0.05) * 5000;
    return {
      ...MOCK_SNAPSHOT,
      timestamp: bar.timestamp,
      current_bar: bar,
      equity: equity.toFixed(2),
      available_balance: (equity * 0.8).toFixed(2),
      unrealized_pnl: (Math.sin(index * 0.1) * 2000).toFixed(2),
      realized_pnl_today: (index * 5).toFixed(2),
      total_trades: Math.floor(index / 5),
      win_rate: 50 + Math.sin(index * 0.01) * 10,
      max_drawdown_pct: Math.max(0, 15 - index * 0.005),
      positions: MOCK_SNAPSHOT.positions.map((p) => ({
        ...p,
        unrealized_pnl: (parseFloat(p.unrealized_pnl) + Math.sin(index * 0.1) * 100).toFixed(2),
        updated_at: bar.timestamp,
      })),
    };
  }

  private buildFill(signal: Signal, bar: StandardBar): OrderFill {
    return {
      order_id: `ord-${signal.timestamp}`,
      symbol: signal.symbol,
      side: signal.action.includes('long') ? 'Buy' : 'Sell',
      direction: signal.action.includes('long') ? 'Long' : 'Short',
      filled_price: bar.close,
      filled_quantity: signal.quantity,
      fee: (parseFloat(bar.close) * parseFloat(signal.quantity) * 0.0005).toFixed(4),
      timestamp: signal.timestamp,
      realized_pnl: signal.action.startsWith('close') || signal.action.startsWith('reduce')
        ? (Math.random() * 500).toFixed(2)
        : undefined,
    };
  }

  private emitComplete(): void {
    this.emit({
      type: 'complete',
      result: {
        backtest_id: 'bt_mock_001',
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
      },
    });
  }

  private emit(msg: WsMessage): void {
    this.listeners.forEach((fn) => fn(msg));
  }
}
