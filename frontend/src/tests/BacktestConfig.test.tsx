import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { BacktestConfig } from '../components/Backtest/BacktestConfig';
import { useAppStore } from '../stores/useAppStore';

describe('BacktestConfig', () => {
  const mockWsRef = { current: { subscribe: vi.fn() } as any };
  
  // Helper to get the strategy select (second combobox)
  const getStrategySelect = () => screen.getAllByRole('combobox')[1];
  
  beforeEach(() => {
    // Reset store to initial state
    useAppStore.setState({
      wsConnected: false,
      engineOnline: false,
      playback: { status: 'idle', currentBarIndex: 0, totalBars: 0, speed: 1, currentTime: 0 },
      bars: [],
      snapshot: null,
      signals: [],
      activeSignals: [],
      tradeHistory: [],
      indicators: [],
      chartTimeframe: 'H1',
      visibleRange: { from: 0, to: 0 },
      markerVisibility: true,
      backtestResult: null,
      backtestId: null,
      currentStrategy: null,
    });

    // Reset fetch mock - must return a promise to avoid .then() on undefined
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({
        id: 'default',
        name: 'Default Strategy',
        description: 'Default',
        default_params: {},
        param_definitions: [],
      }),
    }));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('renders with all form fields', () => {
    render(<BacktestConfig wsRef={mockWsRef} />);
    
    const comboboxes = screen.getAllByRole('combobox');
    expect(comboboxes).toHaveLength(2); // Symbol and Strategy
    expect(screen.getByText(/timeframe/i)).toBeInTheDocument();
    // Use getByDisplayValue for inputs since labels don't have htmlFor attributes
    expect(screen.getByDisplayValue('2024-01-01')).toBeInTheDocument(); // Start Date
    expect(screen.getByDisplayValue('2024-12-31')).toBeInTheDocument(); // End Date
    expect(screen.getByDisplayValue('10000')).toBeInTheDocument(); // Initial Balance
    expect(screen.getByDisplayValue('10')).toBeInTheDocument(); // Leverage
  });

  it('strategy selector has correct default value', () => {
    render(<BacktestConfig wsRef={mockWsRef} />);
    
    const strategySelect = getStrategySelect() as HTMLSelectElement;
    expect(strategySelect.value).toBe('ema_cross');
  });

  it('changing strategy triggers fetch call to /api/strategies/:id/defaults', async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({
        id: 'rsi_macd',
        name: 'RSI + MACD',
        description: 'RSI and MACD strategy',
        default_params: { period: 14 },
        param_definitions: [],
      }),
    });
    vi.stubGlobal('fetch', mockFetch);

    render(<BacktestConfig wsRef={mockWsRef} />);
    
    const strategySelect = getStrategySelect();
    fireEvent.change(strategySelect, { target: { value: 'rsi_macd' } });
    
    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('/api/strategies/rsi_macd/defaults'),
        expect.any(Object)
      );
    });
  });

  it('accordion appears when strategy has param_definitions', async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({
        id: 'ema_crossover',
        name: 'EMA Crossover',
        description: 'EMA crossover strategy',
        default_params: { fast_period: 9, slow_period: 21 },
        param_definitions: [
          {
            name: 'fast_period',
            description: 'Fast EMA period',
            param_type: { Integer: { min: 1, max: 100, default: 9 } },
          },
          {
            name: 'slow_period',
            description: 'Slow EMA period',
            param_type: { Integer: { min: 1, max: 200, default: 21 } },
          },
        ],
      }),
    });
    vi.stubGlobal('fetch', mockFetch);

    render(<BacktestConfig wsRef={mockWsRef} />);
    
    // Trigger strategy change to load param definitions
    const strategySelect = getStrategySelect();
    fireEvent.change(strategySelect, { target: { value: 'ema_cross' } });
    
    await waitFor(() => {
      expect(screen.getByText(/strategy parameters/i)).toBeInTheDocument();
    });
  });

  it('accordion can be expanded and collapsed', async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({
        id: 'ema_crossover',
        name: 'EMA Crossover',
        description: 'EMA crossover strategy',
        default_params: { fast_period: 9 },
        param_definitions: [
          {
            name: 'fast_period',
            description: 'Fast EMA period',
            param_type: { Integer: { min: 1, max: 100, default: 9 } },
          },
        ],
      }),
    });
    vi.stubGlobal('fetch', mockFetch);

    render(<BacktestConfig wsRef={mockWsRef} />);
    
    const strategySelect = getStrategySelect();
    fireEvent.change(strategySelect, { target: { value: 'ema_cross' } });
    
    await waitFor(() => {
      expect(screen.getByText(/strategy parameters/i)).toBeInTheDocument();
    });
    
    const accordionButton = screen.getByText(/strategy parameters/i);
    
    // Expand
    fireEvent.click(accordionButton);
    await waitFor(() => {
      // fast_period appears in both accordion label and strategy info,
      // so check for the input instead
      expect(screen.getByDisplayValue('9')).toBeInTheDocument();
    });
    
    // Collapse
    fireEvent.click(accordionButton);
    await waitFor(() => {
      expect(screen.queryByDisplayValue('9')).not.toBeInTheDocument();
    });
  });

  it('parameter inputs render correctly for Integer and Decimal types', async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({
        id: 'test_strategy',
        name: 'Test Strategy',
        description: 'Test strategy',
        default_params: { int_param: 10, decimal_param: 1.5 },
        param_definitions: [
          {
            name: 'int_param',
            description: 'Integer parameter',
            param_type: { Integer: { min: 1, max: 100, default: 10 } },
          },
          {
            name: 'decimal_param',
            description: 'Decimal parameter',
            param_type: { Decimal: { min: '0.1', max: '10.0', default: '1.5' } },
          },
        ],
      }),
    });
    vi.stubGlobal('fetch', mockFetch);

    render(<BacktestConfig wsRef={mockWsRef} />);
    
    const strategySelect = getStrategySelect();
    fireEvent.change(strategySelect, { target: { value: 'ema_cross' } });
    
    await waitFor(() => {
      expect(screen.getByText(/strategy parameters/i)).toBeInTheDocument();
    });
    
    const accordionButton = screen.getByText(/strategy parameters/i);
    fireEvent.click(accordionButton);
    
    await waitFor(() => {
      const paramSection = screen.getByText(/strategy parameters/i).closest('div') as HTMLElement;
      expect(within(paramSection).getByText('int_param')).toBeInTheDocument();
      expect(within(paramSection).getByText('decimal_param')).toBeInTheDocument();
    });
  });

  it('changing parameter values updates the input', async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({
        id: 'test_strategy',
        name: 'Test Strategy',
        description: 'Test strategy',
        default_params: { fast_period: 9 },
        param_definitions: [
          {
            name: 'fast_period',
            description: 'Fast EMA period',
            param_type: { Integer: { min: 1, max: 100, default: 9 } },
          },
        ],
      }),
    });
    vi.stubGlobal('fetch', mockFetch);

    render(<BacktestConfig wsRef={mockWsRef} />);
    
    const strategySelect = getStrategySelect();
    fireEvent.change(strategySelect, { target: { value: 'ema_cross' } });
    
    await waitFor(() => {
      expect(screen.getByText(/strategy parameters/i)).toBeInTheDocument();
    });
    
    const accordionButton = screen.getByText(/strategy parameters/i);
    fireEvent.click(accordionButton);
    
    await waitFor(() => {
      const input = screen.getByDisplayValue('9') as HTMLInputElement;
      fireEvent.change(input, { target: { value: '20' } });
      expect(input.value).toBe('20');
    });
  });

  it('strategy info display shows after backtest starts', () => {
    // Set currentStrategy in store to simulate backtest started
    useAppStore.setState({
      currentStrategy: {
        id: 'ema_crossover',
        name: 'EMA Crossover',
        description: 'EMA crossover strategy',
        default_params: { fast_period: 9, slow_period: 21 },
        param_definitions: [],
      },
    });

    render(<BacktestConfig wsRef={mockWsRef} />);
    
    // EMA Crossover appears in select option AND strategy info display
    const emaCrossoverElements = screen.getAllByText('EMA Crossover');
    expect(emaCrossoverElements).toHaveLength(2);
    expect(screen.getByText('fast_period')).toBeInTheDocument();
    expect(screen.getByText('slow_period')).toBeInTheDocument();
    expect(screen.getByText('9')).toBeInTheDocument();
    expect(screen.getByText('21')).toBeInTheDocument();
  });

  it('START BACKTEST button exists', () => {
    render(<BacktestConfig wsRef={mockWsRef} />);
    
    expect(screen.getByRole('button', { name: /start backtest/i })).toBeInTheDocument();
  });
});