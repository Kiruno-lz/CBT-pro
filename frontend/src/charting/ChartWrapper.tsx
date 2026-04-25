import { useRef, useCallback } from 'react';
import CandlestickChart from '../charting/CandlestickChart';
import TradeMarkers from '../charting/TradeMarkers';
import IndicatorOverlays from '../charting/IndicatorOverlays';
import type { StandardBar } from '../types';
import { useAppStore } from '../stores/useAppStore';
import type { IChartApi, ISeriesApi } from 'lightweight-charts';

interface ChartWrapperProps {
  bars: StandardBar[];
}

export default function ChartWrapper({ bars }: ChartWrapperProps) {
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<'Candlestick'> | null>(null);
  const tradeHistory = useAppStore((s) => s.tradeHistory);
  const indicators = useAppStore((s) => s.indicators);
  const markerVisibility = useAppStore((s) => s.markerVisibility);
  const setVisibleRange = useAppStore((s) => s.setVisibleRange);

  const handleChartReady = useCallback((chart: IChartApi, series: ISeriesApi<'Candlestick'>) => {
    chartRef.current = chart;
    seriesRef.current = series;
  }, []);

  const handleRangeChange = useCallback((from: number, to: number) => {
    setVisibleRange({ from, to });
  }, [setVisibleRange]);

  return (
    <div className="relative w-full h-full">
      <CandlestickChart bars={bars} onVisibleRangeChange={handleRangeChange} onChartReady={handleChartReady} />
      {chartRef.current && seriesRef.current && markerVisibility && (
        <TradeMarkers chart={chartRef.current} series={seriesRef.current} trades={tradeHistory} />
      )}
      {chartRef.current && (
        <IndicatorOverlays chart={chartRef.current} bars={bars} indicators={indicators} />
      )}
    </div>
  );
}
