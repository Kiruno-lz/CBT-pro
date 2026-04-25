import { useEffect, useRef, useMemo } from 'react';
import { type IChartApi, type ISeriesApi, type Time, type SeriesMarker } from 'lightweight-charts';
import type { OrderFill } from '../types';

interface TradeMarkersProps {
  chart: IChartApi | null;
  series: ISeriesApi<'Candlestick'> | null;
  trades: OrderFill[];
}

function getMarkerShape(direction: 'Long' | 'Short', _side: 'Buy' | 'Sell', realizedPnl?: string): SeriesMarker<Time>['shape'] {
  const isClose = !!realizedPnl;
  if (isClose) return 'square';
  return direction === 'Long' ? 'arrowUp' : 'arrowDown';
}

function getMarkerColor(_direction: 'Long' | 'Short', side: 'Buy' | 'Sell', realizedPnl?: string): string {
  if (realizedPnl) {
    const pnl = parseFloat(realizedPnl);
    return pnl >= 0 ? '#22c55e' : '#ef4444';
  }
  return side === 'Buy' ? '#22c55e' : '#ef4444';
}

function getMarkerPosition(direction: 'Long' | 'Short', _side: 'Buy' | 'Sell', realizedPnl?: string): SeriesMarker<Time>['position'] {
  if (realizedPnl) return 'inBar';
  return direction === 'Long' ? 'belowBar' : 'aboveBar';
}

function getMarkerText(fill: OrderFill): string {
  const qty = parseFloat(fill.filled_quantity).toFixed(3);
  if (fill.realized_pnl) {
    const pnl = parseFloat(fill.realized_pnl);
    const sign = pnl >= 0 ? '+' : '';
    return `${sign}${pnl.toFixed(2)}`;
  }
  return `${fill.side} ${qty}`;
}

export default function TradeMarkers({ chart, series, trades }: TradeMarkersProps) {
  const markersRef = useRef<SeriesMarker<Time>[]>([]);

  const markers = useMemo<SeriesMarker<Time>[]>(() => {
    return trades.map((fill) => ({
      time: Math.floor(fill.timestamp / 1000) as Time,
      position: getMarkerPosition(fill.direction, fill.side, fill.realized_pnl),
      color: getMarkerColor(fill.direction, fill.side, fill.realized_pnl),
      shape: getMarkerShape(fill.direction, fill.side, fill.realized_pnl),
      text: getMarkerText(fill),
      size: fill.realized_pnl ? 1 : 2,
    }));
  }, [trades]);

  useEffect(() => {
    if (!series || !chart) return;

    if (JSON.stringify(markers) !== JSON.stringify(markersRef.current)) {
      series.setMarkers(markers);
      markersRef.current = markers;
    }
  }, [markers, series, chart]);

  return null;
}
