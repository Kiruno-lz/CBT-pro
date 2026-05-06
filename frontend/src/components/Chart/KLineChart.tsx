import { useEffect, useRef, useCallback, useState } from 'react';
import {
  createChart,
  ColorType,
  CrosshairMode,
  type IChartApi,
  type ISeriesApi,
  type CandlestickData,
  type Time,
  type SeriesMarkerPosition,
} from 'lightweight-charts';
import { useAppStore } from '../../stores/useAppStore';
import { fetchWithTimeout } from '../../utils/fetch';
import type { StandardBar, IndicatorValue } from '../../types';

interface IndicatorData {
  values: IndicatorValue[];
  panel: 'main' | 'sub';
}

const INDICATOR_DISPLAY_NAMES: Record<string, string> = {
  ema_9: 'EMA 9',
  ema_21: 'EMA 21',
  rsi_14: 'RSI 14',
  macd_12_26_9: 'MACD',
  bollinger_20_2: 'Bollinger',
  atr_14: 'ATR 14',
  vwap: 'VWAP',
};

function getIndicatorDisplayName(name: string): string {
  if (INDICATOR_DISPLAY_NAMES[name]) {
    return INDICATOR_DISPLAY_NAMES[name];
  }

  const parts = name.split('_');

  if (parts[0] === 'macd') {
    return `MACD ${parts.slice(1).join('/')}`;
  }

  if (parts[0] === 'bollinger') {
    return `Bollinger ${parts.slice(1).join('/')}`;
  }

  if (parts[0] === 'ema' || parts[0] === 'rsi' || parts[0] === 'atr') {
    return `${parts[0].toUpperCase()} ${parts[1]}`;
  }

  return name.toUpperCase();
}

const PREDEFINED_COLORS: Record<string, string> = {
  ema_9: '#f59e0b',
  ema_21: '#8b5cf6',
  rsi_14: '#10b981',
  atr_14: '#ef4444',
  vwap: '#06b6d4',
};

const COLOR_PALETTE = [
  '#f59e0b', '#8b5cf6', '#10b981', '#ef4444', '#06b6d4',
  '#ec4899', '#f97316', '#84cc16', '#14b8a6', '#6366f1',
  '#a855f7', '#d946ef', '#f43f5e', '#eab308', '#22c55e',
];

function getIndicatorColor(name: string): string {
  if (PREDEFINED_COLORS[name]) {
    return PREDEFINED_COLORS[name];
  }

  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }

  const index = Math.abs(hash) % COLOR_PALETTE.length;
  return COLOR_PALETTE[index];
}

export function KLineChart() {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const candlestickSeriesRef = useRef<ISeriesApi<'Candlestick'> | null>(null);
  const volumeSeriesRef = useRef<ISeriesApi<'Histogram'> | null>(null);
  const indicatorSeriesRef = useRef<Map<string, ISeriesApi<'Line'>>>(new Map());
  const indicatorHistogramRef = useRef<Map<string, ISeriesApi<'Histogram'>>>(new Map());
  const prevBarsLengthRef = useRef<number>(0);

  const [indicatorData, setIndicatorData] = useState<Map<string, IndicatorData>>(new Map());

  const { bars, snapshot, indicators, backtestId, chartTimeframe, updateIndicator } = useAppStore();

  const formatBarToCandle = useCallback((bar: StandardBar): CandlestickData<Time> => {
    return {
      time: bar.timestamp as Time,
      open: parseFloat(bar.open),
      high: parseFloat(bar.high),
      low: parseFloat(bar.low),
      close: parseFloat(bar.close),
    };
  }, []);

  const formatBarToVolume = useCallback((bar: StandardBar) => {
    const isUp = parseFloat(bar.close) >= parseFloat(bar.open);
    return {
      time: bar.timestamp as Time,
      value: parseFloat(bar.volume),
      color: isUp ? 'rgba(52, 211, 153, 0.4)' : 'rgba(248, 113, 113, 0.4)',
    };
  }, []);

  const removeAllIndicators = useCallback(() => {
    if (!chartRef.current) return;

    indicatorSeriesRef.current.forEach((series) => {
      chartRef.current?.removeSeries(series);
    });
    indicatorSeriesRef.current.clear();

    indicatorHistogramRef.current.forEach((series) => {
      chartRef.current?.removeSeries(series);
    });
    indicatorHistogramRef.current.clear();
  }, []);

  const fetchIndicators = useCallback(async () => {
    if (!backtestId || bars.length === 0) return;

    const visibleIndicators = indicators.filter((ind) => ind.visible);
    console.log('fetchIndicators - visibleIndicators:', visibleIndicators.map(i => ({ name: i.name, params: i.params })));
    if (visibleIndicators.length === 0) {
      setIndicatorData(new Map());
      return;
    }

    const indicatorNames = visibleIndicators.map((ind) => ind.name).join(',');
    console.log('fetchIndicators - requesting:', indicatorNames);
    const symbol = snapshot?.current_bar?.symbol || bars[0]?.symbol || '';

    try {
      const response = await fetchWithTimeout(
        `/api/indicators?symbol=${encodeURIComponent(symbol)}&timeframe=${encodeURIComponent(chartTimeframe)}&indicators=${encodeURIComponent(indicatorNames)}&backtest_id=${encodeURIComponent(backtestId)}&full=true`,
        {},
        10000
      );

      if (!response.ok) return;

      const data = await response.json();
      const newData = new Map<string, IndicatorData>();

      Object.entries(data).forEach(([name, values]) => {
        const config = indicators.find((ind) => ind.name === name);
        if (config) {
          newData.set(name, {
            values: values as IndicatorValue[],
            panel: config.panel,
          });
        }
      });

      setIndicatorData(newData);
    } catch (error) {
      if (error instanceof Error && error.name === 'AbortError') {
        console.error('Failed to fetch indicators: Request timed out');
      } else {
        console.error('Failed to fetch indicators:', error);
      }
    }
  }, [backtestId, bars, indicators, chartTimeframe, snapshot]);

  useEffect(() => {
    if (!containerRef.current) return;

    const chart = createChart(containerRef.current, {
      width: containerRef.current.clientWidth,
      height: containerRef.current.clientHeight,
      layout: {
        background: { type: ColorType.Solid, color: 'transparent' },
        textColor: '#9ca3af',
        fontFamily: 'JetBrains Mono, monospace',
        fontSize: 10,
      },
      grid: {
        vertLines: { color: 'rgba(99, 122, 241, 0.08)' },
        horzLines: { color: 'rgba(99, 122, 241, 0.08)' },
      },
      crosshair: {
        mode: CrosshairMode.Normal,
        vertLine: {
          color: 'rgba(110, 177, 232, 0.4)',
          labelBackgroundColor: '#1e293b',
        },
        horzLine: {
          color: 'rgba(110, 177, 232, 0.4)',
          labelBackgroundColor: '#1e293b',
        },
      },
      timeScale: {
        borderColor: 'rgba(99, 122, 241, 0.15)',
        timeVisible: true,
        secondsVisible: false,
      },
      rightPriceScale: {
        borderColor: 'rgba(99, 122, 241, 0.15)',
        scaleMargins: { top: 0.05, bottom: 0.3 },
      },
      handleScroll: { vertTouchDrag: false },
    });

    chartRef.current = chart;

    const candlestickSeries = chart.addCandlestickSeries({
      upColor: '#34d399',
      downColor: '#f87171',
      borderUpColor: '#34d399',
      borderDownColor: '#f87171',
      wickUpColor: '#6b7280',
      wickDownColor: '#6b7280',
    });
    candlestickSeriesRef.current = candlestickSeries;

    const volumeSeries = chart.addHistogramSeries({
      color: 'rgba(99, 122, 241, 0.3)',
      priceFormat: { type: 'volume' },
      priceScaleId: 'volume',
    });
    volumeSeriesRef.current = volumeSeries;

    chart.priceScale('volume').applyOptions({
      scaleMargins: { top: 0.75, bottom: 0 },
    });

    const handleResize = () => {
      if (chartRef.current && containerRef.current) {
        chartRef.current.applyOptions({
          width: containerRef.current.clientWidth,
          height: containerRef.current.clientHeight,
        });
      }
    };

    const resizeObserver = new ResizeObserver(handleResize);
    resizeObserver.observe(containerRef.current);

    return () => {
      resizeObserver.disconnect();
      removeAllIndicators();
      chart.remove();
      chartRef.current = null;
      candlestickSeriesRef.current = null;
      volumeSeriesRef.current = null;
    };
  }, [removeAllIndicators]);

  useEffect(() => {
    if (!candlestickSeriesRef.current || bars.length === 0) return;

    const candleData = bars.map(formatBarToCandle);
    candlestickSeriesRef.current.setData(candleData);
  }, [bars, formatBarToCandle]);

  useEffect(() => {
    if (!volumeSeriesRef.current || bars.length === 0) return;

    const volumeData = bars.map(formatBarToVolume);
    volumeSeriesRef.current.setData(volumeData);
  }, [bars, formatBarToVolume]);

  useEffect(() => {
    if (!candlestickSeriesRef.current || !snapshot?.current_bar || bars.length === 0) {
      return;
    }

    const lastBar = bars[bars.length - 1];
    if (lastBar) {
      candlestickSeriesRef.current.update(formatBarToCandle(lastBar));
    }
  }, [snapshot, bars, formatBarToCandle]);

  useEffect(() => {
    if (!backtestId) return;
    fetchIndicators();
  }, [backtestId, fetchIndicators]);

  useEffect(() => {
    if (!backtestId || bars.length === 0) return;

    if (bars.length % 10 === 0 && bars.length !== prevBarsLengthRef.current) {
      prevBarsLengthRef.current = bars.length;
      fetchIndicators();
    }
  }, [bars.length, backtestId, fetchIndicators]);

  useEffect(() => {
    if (!chartRef.current) return;

    removeAllIndicators();

    indicatorData.forEach((data, name) => {
      if (!chartRef.current) return;

      if (name.startsWith('bollinger_')) {
        const upperSeries = chartRef.current.addLineSeries({
          color: 'rgba(236, 72, 153, 0.6)',
          lineStyle: 2,
          priceScaleId: 'right',
          lastValueVisible: false,
        });
        upperSeries.setData(
          data.values.map((v) => ({
            time: v.timestamp as Time,
            value: parseFloat(v.upper || '0'),
          }))
        );
        indicatorSeriesRef.current.set('bollinger_20_2_upper', upperSeries);

        const middleSeries = chartRef.current.addLineSeries({
          color: 'rgba(236, 72, 153, 0.9)',
          lineWidth: 2,
          priceScaleId: 'right',
          lastValueVisible: false,
        });
        middleSeries.setData(
          data.values.map((v) => ({
            time: v.timestamp as Time,
            value: parseFloat(v.middle || '0'),
          }))
        );
        indicatorSeriesRef.current.set('bollinger_20_2_middle', middleSeries);

        const lowerSeries = chartRef.current.addLineSeries({
          color: 'rgba(236, 72, 153, 0.6)',
          lineStyle: 2,
          priceScaleId: 'right',
          lastValueVisible: false,
        });
        lowerSeries.setData(
          data.values.map((v) => ({
            time: v.timestamp as Time,
            value: parseFloat(v.lower || '0'),
          }))
        );
        indicatorSeriesRef.current.set('bollinger_20_2_lower', lowerSeries);
      } else if (name.startsWith('macd_')) {
        const macdSeries = chartRef.current.addLineSeries({
          color: '#3b82f6',
          priceScaleId: 'sub',
          lastValueVisible: false,
        });
        macdSeries.setData(
          data.values.map((v) => ({
            time: v.timestamp as Time,
            value: parseFloat(v.macd || '0'),
          }))
        );
        indicatorSeriesRef.current.set('macd_12_26_9_macd', macdSeries);

        const signalSeries = chartRef.current.addLineSeries({
          color: '#f59e0b',
          priceScaleId: 'sub',
          lastValueVisible: false,
        });
        signalSeries.setData(
          data.values.map((v) => ({
            time: v.timestamp as Time,
            value: parseFloat(v.signal || '0'),
          }))
        );
        indicatorSeriesRef.current.set('macd_12_26_9_signal', signalSeries);

        const histogramSeries = chartRef.current.addHistogramSeries({
          color: 'rgba(99, 122, 241, 0.5)',
          priceScaleId: 'sub',
        });
        histogramSeries.setData(
          data.values.map((v) => ({
            time: v.timestamp as Time,
            value: parseFloat(v.histogram || '0'),
          }))
        );
        indicatorHistogramRef.current.set('macd_12_26_9_histogram', histogramSeries);
      } else {
        const color = getIndicatorColor(name);
        const priceScaleId = data.panel === 'main' ? 'right' : 'sub';

        const series = chartRef.current.addLineSeries({
          color,
          priceScaleId,
          lastValueVisible: false,
        });
        series.setData(
          data.values.map((v) => ({
            time: v.timestamp as Time,
            value: parseFloat(v.value || '0'),
          }))
        );
        indicatorSeriesRef.current.set(name, series);
      }
    });
  }, [indicatorData, removeAllIndicators]);

  const handleToggleIndicator = useCallback(
    (name: string) => {
      const indicator = indicators.find((ind) => ind.name === name);
      if (indicator) {
        updateIndicator(name, { visible: !indicator.visible });
      }
    },
    [indicators, updateIndicator]
  );

  return (
    <div className="panel h-full">
      <div className="panel-header">
        <div className="flex items-center gap-4">
          <span className="panel-title">KLINE CHART</span>
          {snapshot?.current_bar && (
            <div className="flex items-center gap-3 font-mono text-2xs">
              <span className="text-text-secondary">{snapshot.current_bar.symbol}</span>
              <span className="text-text-muted">
                {new Date(snapshot.current_bar.timestamp * 1000).toLocaleString()}
              </span>
            </div>
          )}
        </div>
        <div className="flex items-center gap-2">
          <div className="flex items-center gap-1">
            {indicators.map((indicator) => (
              <button
                key={indicator.name}
                onClick={() => handleToggleIndicator(indicator.name)}
                className={`text-2xs font-mono rounded px-2 py-0.5 cursor-pointer transition-colors ${
                  indicator.visible
                    ? 'bg-surface-elevated text-text-primary border-l-2'
                    : 'bg-surface-base text-text-muted'
                }`}
                style={
                  indicator.visible
                    ? { borderLeftColor: getIndicatorColor(indicator.name) }
                    : undefined
                }
              >
                {getIndicatorDisplayName(indicator.name)}
              </button>
            ))}
          </div>
          <span className="text-2xs text-text-muted font-mono">{bars.length} BARS</span>
        </div>
      </div>

      <div
        ref={containerRef}
        className="flex-1 min-h-0"
        style={{ height: 'calc(100% - 40px)' }}
      />
    </div>
  );
}
