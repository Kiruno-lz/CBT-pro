import { useEffect, useRef, useCallback } from 'react';
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
import type { StandardBar } from '../../types';

export function KLineChart() {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const candlestickSeriesRef = useRef<ISeriesApi<'Candlestick'> | null>(null);
  const volumeSeriesRef = useRef<ISeriesApi<'Histogram'> | null>(null);

  const { bars, snapshot } = useAppStore();

  const formatBarToCandle = useCallback((bar: StandardBar): CandlestickData<Time> => {
    return {
      time: (bar.timestamp / 1000) as Time,
      open: parseFloat(bar.open),
      high: parseFloat(bar.high),
      low: parseFloat(bar.low),
      close: parseFloat(bar.close),
    };
  }, []);

  const formatBarToVolume = useCallback((bar: StandardBar) => {
    const isUp = parseFloat(bar.close) >= parseFloat(bar.open);
    return {
      time: (bar.timestamp / 1000) as Time,
      value: parseFloat(bar.volume),
      color: isUp ? 'rgba(52, 211, 153, 0.4)' : 'rgba(248, 113, 113, 0.4)',
    };
  }, []);

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
        scaleMargins: { top: 0.05, bottom: 0.1 },
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
      scaleMargins: { top: 0.85, bottom: 0 },
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
      chart.remove();
      chartRef.current = null;
    };
  }, []);

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

  return (
    <div className="panel h-full">
      <div className="panel-header">
        <div className="flex items-center gap-4">
          <span className="panel-title">KLINE CHART</span>
          {snapshot?.current_bar && (
            <div className="flex items-center gap-3 font-mono text-2xs">
              <span className="text-text-secondary">{snapshot.current_bar.symbol}</span>
              <span className="text-text-muted">
                {new Date(snapshot.current_bar.timestamp).toLocaleString()}
              </span>
            </div>
          )}
        </div>
        <div className="flex items-center gap-2">
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