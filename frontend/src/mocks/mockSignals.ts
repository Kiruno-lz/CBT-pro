import type { Signal } from '../types';

export const MOCK_SIGNALS: Signal[] = [
  {
    action: 'open_long',
    symbol: 'BTC-USDT',
    quantity: '0.50',
    strength: 0.82,
    reason: 'EMA9 crossed above EMA21 with RSI confirmation',
    timestamp: Date.now() - 86400000,
    take_profit: '45000.00',
    stop_loss: '40000.00',
  },
  {
    action: 'open_short',
    symbol: 'ETH-USDT',
    quantity: '5.00',
    strength: 0.65,
    reason: 'Price rejected at upper Bollinger Band, MACD bearish crossover',
    timestamp: Date.now() - 43200000,
    take_profit: '2400.00',
    stop_loss: '2700.00',
  },
  {
    action: 'add_long',
    symbol: 'BTC-USDT',
    quantity: '0.25',
    strength: 0.71,
    reason: 'Pullback to EMA9 support on increased volume',
    timestamp: Date.now() - 21600000,
  },
  {
    action: 'reduce_long',
    symbol: 'BTC-USDT',
    quantity: '0.25',
    strength: 0.45,
    reason: 'Partial profit taking at resistance level',
    timestamp: Date.now() - 10800000,
  },
  {
    action: 'close_long',
    symbol: 'BTC-USDT',
    quantity: '0.50',
    strength: 0.91,
    reason: 'Target reached, full position closed',
    timestamp: Date.now() - 3600000,
  },
];

export const MOCK_ACTIVE_SIGNALS: Signal[] = MOCK_SIGNALS.filter(
  (s) =>
    s.action === 'open_long' ||
    s.action === 'open_short' ||
    s.action === 'add_long' ||
    s.action === 'add_short'
);
