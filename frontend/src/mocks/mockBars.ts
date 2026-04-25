import type { StandardBar } from '../types';

function generateSyntheticBars(
  count = 1000,
  basePrice = 42000,
  volatility = 0.015,
  startTime = Date.now() - count * 3600000
): StandardBar[] {
  const bars: StandardBar[] = [];
  let price = basePrice;

  for (let i = 0; i < count; i++) {
    const change = (Math.random() - 0.5) * volatility;
    const open = price;
    const close = price * (1 + change);
    const high = Math.max(open, close) * (1 + Math.random() * volatility * 0.5);
    const low = Math.min(open, close) * (1 - Math.random() * volatility * 0.5);
    const volume = 100 + Math.random() * 900;
    price = close;

    bars.push({
      timestamp: startTime + i * 3600000,
      open: open.toFixed(2),
      high: high.toFixed(2),
      low: low.toFixed(2),
      close: close.toFixed(2),
      volume: volume.toFixed(4),
      symbol: 'BTC-USDT',
      exchange: 'binance',
      confirmed: true,
    });
  }

  return bars;
}

export const MOCK_BARS = generateSyntheticBars(2000, 42500, 0.012);

export function getMockBars(count?: number): StandardBar[] {
  if (count) {
    return MOCK_BARS.slice(0, count);
  }
  return [...MOCK_BARS];
}
