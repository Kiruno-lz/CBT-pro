import type { AppState, AppActions, WsMessage } from '../types';

export type StoreApi = Pick<AppActions, 'setWsConnected' | 'setEngineOnline' | 'setSnapshot' | 'appendBar' | 'addTrade' | 'addSignal' | 'setPlayback' | 'setBacktestResult' | 'setTradeHistory'> & { playback: AppState['playback'] };

export class EngineWebSocket {
  private url: string;
  private store: StoreApi;
  private ws: WebSocket | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private backtestId: string | null = null;
  private isManualClose = false;

  constructor(url: string, store: StoreApi) {
    this.url = url;
    this.store = store;
  }

  connect(): void {
    if (this.ws?.readyState === WebSocket.OPEN) return;
    this.isManualClose = false;
    try {
      this.ws = new WebSocket(this.url);
      this.ws.onopen = () => {
        this.reconnectAttempts = 0;
        this.store.setWsConnected(true);
        if (this.backtestId) {
          this.subscribe(this.backtestId);
        }
      };
      this.ws.onmessage = (event) => {
        this.handleMessage(event.data);
      };
      this.ws.onclose = () => {
        this.store.setWsConnected(false);
        this.store.setEngineOnline(false);
        this.attemptReconnect();
      };
      this.ws.onerror = () => {
        this.store.setWsConnected(false);
      };
    } catch {
      this.store.setWsConnected(false);
      this.attemptReconnect();
    }
  }

  disconnect(): void {
    this.isManualClose = true;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.ws?.close();
    this.ws = null;
    this.store.setWsConnected(false);
  }

  subscribe(backtestId: string): void {
    this.backtestId = backtestId;
    this.send({ type: 'subscribe', channel: 'backtest_state', backtest_id: backtestId });
  }

  sendControl(action: 'play' | 'pause' | 'step_forward' | 'step_backward'): void {
    if (!this.backtestId) return;
    this.send({ type: 'control', action, backtest_id: this.backtestId });
  }

  setSpeed(speed: number): void {
    this.send({ type: 'control', action: 'set_speed', speed });
  }

  private send(data: Record<string, unknown>): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(data));
    }
  }

  private attemptReconnect(): void {
    if (this.isManualClose) return;
    if (this.reconnectAttempts >= this.maxReconnectAttempts) return;
    this.reconnectAttempts++;
    this.reconnectTimer = setTimeout(() => {
      this.connect();
    }, Math.min(1000 * this.reconnectAttempts, 10000));
  }

  private handleMessage(data: string): void {
    let msg: WsMessage;
    try {
      msg = JSON.parse(data) as WsMessage;
    } catch {
      return;
    }

    switch (msg.type) {
      case 'snapshot': {
        this.store.setSnapshot(msg.data);
        this.store.setEngineOnline(true);
        this.store.setPlayback({
          currentBarIndex: msg.data.total_trades,
          currentTime: msg.data.timestamp,
        });
        break;
      }
      case 'bar_update': {
        this.store.appendBar(msg.bar);
        break;
      }
      case 'trade': {
        this.store.addTrade(msg.fill);
        break;
      }
      case 'signal': {
        this.store.addSignal(msg.signal);
        break;
      }
      case 'complete': {
        this.store.setPlayback({ status: 'complete' });
        this.store.setBacktestResult(msg.result);
        break;
      }
      case 'error': {
        // eslint-disable-next-line no-console
        console.error('WebSocket error:', msg.message);
        break;
      }
    }
  }
}
