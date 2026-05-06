import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { KLineChart } from '../components/Chart/KLineChart';
import { useAppStore } from '../stores/useAppStore';
import type { StandardBar, IndicatorConfig } from '../types';

// Mock lightweight-charts
vi.mock('lightweight-charts', () => ({
  createChart: vi.fn(() => ({
    addCandlestickSeries: vi.fn(() => ({
      setData: vi.fn(),
      update: vi.fn(),
    })),
    addHistogramSeries: vi.fn(() => ({
      setData: vi.fn(),
      update: vi.fn(),
    })),
    addLineSeries: vi.fn(() => ({
      setData: vi.fn(),
      update: vi.fn(),
    })),
    removeSeries: vi.fn(),
    priceScale: vi.fn(() => ({
      applyOptions: vi.fn(),
    })),
    applyOptions: vi.fn(),
    remove: vi.fn(),
  })),
  ColorType: { Solid: 'solid' },
  CrosshairMode: { Normal: 'normal' },
}));

describe('KLineChart', () => {
  const createMockBar = (index: number): StandardBar => ({
    timestamp: Date.now() + index * 3600000,
    open: '100',
    high: '110',
    low: '90',
    close: '105',
    volume: '1000',
    symbol: 'BTCUSDT',
    exchange: 'binance',
    confirmed: true,
  });

  beforeEach(() => {
    // Reset store to initial state with some bars
    useAppStore.setState({
      wsConnected: false,
      engineOnline: false,
      playback: { status: 'idle', currentBarIndex: 0, totalBars: 0, speed: 1, currentTime: 0 },
      bars: [createMockBar(0), createMockBar(1), createMockBar(2)],
      snapshot: null,
      signals: [],
      activeSignals: [],
      tradeHistory: [],
      indicators: [
        { name: 'ema_9', params: { period: 9 }, visible: true, panel: 'main' },
        { name: 'ema_21', params: { period: 21 }, visible: false, panel: 'main' },
        { name: 'rsi_14', params: { period: 14 }, visible: true, panel: 'sub' },
      ],
      chartTimeframe: 'H1',
      visibleRange: { from: 0, to: 0 },
      markerVisibility: true,
      backtestResult: null,
      backtestId: 'test-backtest-123',
      currentStrategy: null,
    });
  });

  it('renders with KLINE CHART title', () => {
    render(<KLineChart />);
    
    expect(screen.getByText('KLINE CHART')).toBeInTheDocument();
  });

  it('indicator toggle buttons render when indicators are in store', () => {
    render(<KLineChart />);
    
    expect(screen.getByText('EMA 9')).toBeInTheDocument();
    expect(screen.getByText('EMA 21')).toBeInTheDocument();
    expect(screen.getByText('RSI 14')).toBeInTheDocument();
  });

  it('clicking toggle button calls updateIndicator', async () => {
    const updateIndicatorSpy = vi.spyOn(useAppStore.getState(), 'updateIndicator');
    
    render(<KLineChart />);
    
    const ema9Button = screen.getByText('EMA 9');
    fireEvent.click(ema9Button);
    
    await waitFor(() => {
      expect(updateIndicatorSpy).toHaveBeenCalledWith('ema_9', { visible: false });
    });
  });

  it('bars count display shows correct number', () => {
    render(<KLineChart />);
    
    expect(screen.getByText(/3 BARS/)).toBeInTheDocument();
  });

  it('chart container renders', () => {
    render(<KLineChart />);
    
    const chartContainer = screen.getByText('KLINE CHART').closest('.panel')?.querySelector('div[class*="flex-1"]') || 
      document.querySelector('[style*="height: calc(100% - 40px)"]');
    expect(chartContainer).toBeTruthy();
  });

  it('updates bars count when bars change', async () => {
    render(<KLineChart />);
    
    expect(screen.getByText(/3 BARS/)).toBeInTheDocument();
    
    // Add more bars
    await act(async () => {
      useAppStore.setState({
        bars: [
          ...useAppStore.getState().bars,
          createMockBar(3),
          createMockBar(4),
        ],
      });
    });
    
    expect(screen.getByText(/5 BARS/)).toBeInTheDocument();
  });

  it('toggle button reflects indicator visibility state', () => {
    render(<KLineChart />);
    
    const ema9Button = screen.getByText('EMA 9');
    const ema21Button = screen.getByText('EMA 21');
    
    // EMA 9 is visible, so it should have different styling class
    expect(ema9Button.className).toContain('bg-surface-elevated');
    
    // EMA 21 is not visible
    expect(ema21Button.className).toContain('bg-surface-base');
  });
});