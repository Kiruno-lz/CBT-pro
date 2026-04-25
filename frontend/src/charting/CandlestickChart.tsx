import { useEffect, useRef, useCallback } from 'react';
import { createChart, type IChartApi, type ISeriesApi, type CandlestickData, type Time, type UTCTimestamp } from 'lightweight-charts';
import type { StandardBar } from '../types';

interface CandlestickChartProps {
  bars: StandardBar[];
  onVisibleRangeChange?: (from: number, to: number) => void;
  onBarClick?: (bar: StandardBar) => void;
  onChartReady?: (chart: IChartApi, series: ISeriesApi<'Candlestick'>) => void;
}

function barToCandlestick(bar: StandardBar): CandlestickData<Time> {
  return {
    time: Math.floor(bar.timestamp / 1000) as UTCTimestamp,
    open: parseFloat(bar.open),
    high: parseFloat(bar.high),
    low: parseFloat(bar.low),
    close: parseFloat(bar.close),
  };
}

export default function CandlestickChart({ bars, onVisibleRangeChange, onChartReady }: CandlestickChartProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<'Candlestick'> | null>(null);
  const barsRef = useRef<StandardBar[]>(bars);
  const rangeCallbackRef = useRef(onVisibleRangeChange);
  const chartReadyRef = useRef(onChartReady);

  barsRef.current = bars;
  rangeCallbackRef.current = onVisibleRangeChange;
  chartReadyRef.current = onChartReady;

  const initChart = useCallback(() => {
    if (!containerRef.current) return;

    const chart = createChart(containerRef.current, {
      layout: {
        background: { color: '#0f172a' },
        textColor: '#94a3b8',
        fontFamily: 'Inter, system-ui, sans-serif',
      },
      grid: {
        vertLines: { color: '#1e293b' },
        horzLines: { color: '#1e293b' },
      },
      crosshair: {
        mode: 1,
        vertLine: { color: '#3b82f6', width: 1, style: 2 },
        horzLine: { color: '#3b82f6', width: 1, style: 2 },
      },
      rightPriceScale: {
        borderColor: '#1e293b',
        scaleMargins: { top: 0.1, bottom: 0.1 },
      },
      timeScale: {
        borderColor: '#1e293b',
        timeVisible: true,
        secondsVisible: false,
      },
      handleScroll: { vertTouchDrag: false },
    });

    const series = chart.addCandlestickSeries({
      upColor: '#22c55e',
      downColor: '#ef4444',
      borderUpColor: '#22c55e',
      borderDownColor: '#ef4444',
      wickUpColor: '#22c55e',
      wickDownColor: '#ef4444',
    });

    chartRef.current = chart;
    seriesRef.current = series;

    if (chartReadyRef.current) {
      chartReadyRef.current(chart, series);
    }

    chart.timeScale().subscribeVisibleLogicalRangeChange(() => {
      const range = chart.timeScale().getVisibleLogicalRange();
      if (range && rangeCallbackRef.current) {
        const from = Math.floor(range.from);
        const to = Math.ceil(range.to);
        rangeCallbackRef.current(from, to);
      }
    });

    const handleResize = () => {
      if (containerRef.current && chartRef.current) {
        const { width, height } = containerRef.current.getBoundingClientRect();
        chartRef.current.applyOptions({ width, height });
      }
    };

    const resizeObserver = new ResizeObserver(handleResize);
    if (containerRef.current) {
      resizeObserver.observe(containerRef.current);
    }

    return () => {
      resizeObserver.disconnect();
      chart.remove();
      chartRef.current = null;
      seriesRef.current = null;
    };
  }, []);

  useEffect(() => {
    const cleanup = initChart();
    return cleanup;
  }, [initChart]);

  useEffect(() => {
    if (!seriesRef.current || bars.length === 0) return;

    const data = bars.map(barToCandlestick);
    seriesRef.current.setData(data);

    if (chartRef.current && bars.length > 0) {
      chartRef.current.timeScale().fitContent();
    }
  }, []);

  useEffect(() => {
    if (!seriesRef.current || bars.length === 0) return;

    const lastBar = bars[bars.length - 1];
    const candle = barToCandlestick(lastBar);
    seriesRef.current.update(candle);
  }, [bars]);

  return (
    <div
      ref={containerRef}
      className="w-full h-full"
      style={{ minHeight: '400px' }}
    />
  );
}
