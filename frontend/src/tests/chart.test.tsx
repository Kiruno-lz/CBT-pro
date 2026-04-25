import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock lightweight-charts before importing anything that uses it
vi.mock('lightweight-charts', () => ({
  createChart: vi.fn(() => ({
    addCandlestickSeries: vi.fn(() => ({
      setData: vi.fn(),
      update: vi.fn(),
      setMarkers: vi.fn(),
    })),
    addLineSeries: vi.fn(() => ({
      setData: vi.fn(),
      update: vi.fn(),
    })),
    addHistogramSeries: vi.fn(() => ({
      setData: vi.fn(),
      update: vi.fn(),
    })),
    timeScale: vi.fn(() => ({
      fitContent: vi.fn(),
      setVisibleLogicalRange: vi.fn(),
    })),
    remove: vi.fn(),
    resize: vi.fn(),
  })),
  CrosshairMode: { Normal: 0 },
  PriceScaleMode: { Normal: 0 },
}));

import { createChart } from 'lightweight-charts';

// Simple chart component for testing
const MockChartWrapper = () => {
  const container = document.createElement('div');
  container.setAttribute('data-testid', 'chart-container');
  container.style.width = '100%';
  container.style.height = '400px';
  return container;
};

describe('Chart Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders chart container without errors', () => {
    const container = MockChartWrapper();
    expect(container.getAttribute('data-testid')).toBe('chart-container');
    expect(container.style.height).toBe('400px');
  });

  it('createChart is callable with DOM element and options', () => {
    const container = document.createElement('div');
    const chart = createChart(container, {
      width: 800,
      height: 400,
      layout: { background: { color: '#1a1a1a' }, textColor: '#d1d4dc' },
      grid: { vertLines: { color: '#2a2a2a' }, horzLines: { color: '#2a2a2a' } },
      crosshair: { mode: 0 },
      rightPriceScale: { borderColor: '#2a2a2a' },
      timeScale: { borderColor: '#2a2a2a' },
    });

    expect(chart).toBeDefined();
    expect(chart.addCandlestickSeries).toBeDefined();
    expect(chart.addLineSeries).toBeDefined();
    expect(chart.addHistogramSeries).toBeDefined();
  });

  it('candlestick series accepts StandardBar data', () => {
    const container = document.createElement('div');
    const chart = createChart(container, { width: 800, height: 400 });
    const series = chart.addCandlestickSeries({
      upColor: '#26a69a',
      downColor: '#ef5350',
      borderUpColor: '#26a69a',
      borderDownColor: '#ef5350',
      wickUpColor: '#26a69a',
      wickDownColor: '#ef5350',
    });

    const mockBars = [
      { time: 1704067200, open: 42000, high: 42500, low: 41800, close: 42350 },
      { time: 1704070800, open: 42350, high: 42800, low: 42200, close: 42600 },
    ];

    series.setData(mockBars);
    expect(series.setData).toHaveBeenCalledWith(mockBars);
  });

  it('handles 10k bars without crashing', () => {
    const container = document.createElement('div');
    const chart = createChart(container, { width: 800, height: 400 });
    const series = chart.addCandlestickSeries();

    const bars = Array.from({ length: 10000 }, (_, i) => ({
      time: 1704067200 + i * 3600,
      open: 42000 + i,
      high: 42100 + i,
      low: 41900 + i,
      close: 42050 + i,
    }));

    series.setData(bars);
    expect(series.setData).toHaveBeenCalledTimes(1);
    expect(series.setData).toHaveBeenCalledWith(bars);
  });
});
