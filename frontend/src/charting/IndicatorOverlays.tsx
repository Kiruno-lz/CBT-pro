import { useEffect, useRef, useMemo, useCallback } from 'react';
import { type IChartApi, type ISeriesApi, type LineData, type HistogramData, type Time, type UTCTimestamp } from 'lightweight-charts';
import type { StandardBar, IndicatorConfig } from '../types';

interface IndicatorOverlaysProps {
  chart: IChartApi | null;
  bars: StandardBar[];
  indicators: IndicatorConfig[];
}

function calculateEMA(data: StandardBar[], period: number): LineData<Time>[] {
  if (data.length < period) return [];
  const k = 2 / (period + 1);
  const result: LineData<Time>[] = [];
  let ema = parseFloat(data[0].close);

  for (let i = 0; i < data.length; i++) {
    const close = parseFloat(data[i].close);
    if (i === 0) {
      ema = close;
    } else {
      ema = close * k + ema * (1 - k);
    }
    result.push({
      time: Math.floor(data[i].timestamp / 1000) as UTCTimestamp,
      value: ema,
    });
  }
  return result;
}

function calculateRSI(data: StandardBar[], period: number): LineData<Time>[] {
  if (data.length < period + 1) return [];
  const result: LineData<Time>[] = [];
  let gains = 0;
  let losses = 0;

  for (let i = 1; i <= period; i++) {
    const change = parseFloat(data[i].close) - parseFloat(data[i - 1].close);
    if (change > 0) gains += change;
    else losses -= change;
  }

  let avgGain = gains / period;
  let avgLoss = losses / period;

  for (let i = period; i < data.length; i++) {
    const change = parseFloat(data[i].close) - parseFloat(data[i - 1].close);
    const gain = change > 0 ? change : 0;
    const loss = change < 0 ? -change : 0;
    avgGain = (avgGain * (period - 1) + gain) / period;
    avgLoss = (avgLoss * (period - 1) + loss) / period;

    const rs = avgLoss === 0 ? 100 : avgGain / avgLoss;
    const rsi = avgLoss === 0 ? 100 : 100 - 100 / (1 + rs);
    result.push({
      time: Math.floor(data[i].timestamp / 1000) as UTCTimestamp,
      value: rsi,
    });
  }
  return result;
}

function calculateMACD(
  data: StandardBar[],
  fast: number,
  slow: number,
  signal: number
): { macd: HistogramData<Time>[]; signal: LineData<Time>[]; histogram: HistogramData<Time>[] } {
  if (data.length < slow + signal) return { macd: [], signal: [], histogram: [] };

  const getEMA = (arr: number[], p: number): number[] => {
    const k = 2 / (p + 1);
    const out: number[] = [];
    let ema = arr[0];
    for (let i = 0; i < arr.length; i++) {
      ema = arr[i] * k + ema * (1 - k);
      out.push(ema);
    }
    return out;
  };

  const closes = data.map((b) => parseFloat(b.close));
  const emaFast = getEMA(closes, fast);
  const emaSlow = getEMA(closes, slow);

  const macdLine: number[] = [];
  for (let i = 0; i < closes.length; i++) {
    macdLine.push(emaFast[i] - emaSlow[i]);
  }

  const sigLine = getEMA(macdLine, signal);

  const macd: HistogramData<Time>[] = [];
  const sig: LineData<Time>[] = [];
  const hist: HistogramData<Time>[] = [];

  for (let i = slow; i < data.length; i++) {
    const time = Math.floor(data[i].timestamp / 1000) as UTCTimestamp;
    const macdVal = macdLine[i];
    const sigVal = sigLine[i];
    macd.push({ time, value: macdVal });
    sig.push({ time, value: sigVal });
    hist.push({ time, value: macdVal - sigVal, color: macdVal >= sigVal ? '#22c55e' : '#ef4444' });
  }

  return { macd, signal: sig, histogram: hist };
}

function calculateBollinger(data: StandardBar[], period: number, stdDev: number): { upper: LineData<Time>[]; middle: LineData<Time>[]; lower: LineData<Time>[] } {
  if (data.length < period) return { upper: [], middle: [], lower: [] };
  const upper: LineData<Time>[] = [];
  const middle: LineData<Time>[] = [];
  const lower: LineData<Time>[] = [];

  for (let i = period - 1; i < data.length; i++) {
    const slice = data.slice(i - period + 1, i + 1).map((b) => parseFloat(b.close));
    const mean = slice.reduce((a, b) => a + b, 0) / period;
    const variance = slice.reduce((a, b) => a + Math.pow(b - mean, 2), 0) / period;
    const sd = Math.sqrt(variance) * stdDev;
    const time = Math.floor(data[i].timestamp / 1000) as UTCTimestamp;
    upper.push({ time, value: mean + sd });
    middle.push({ time, value: mean });
    lower.push({ time, value: mean - sd });
  }
  return { upper, middle, lower };
}

export default function IndicatorOverlays({ chart, bars, indicators }: IndicatorOverlaysProps) {
  const seriesMapRef = useRef<Record<string, ISeriesApi<'Line'> | ISeriesApi<'Histogram'>>>({});
  const paneIdsRef = useRef<Record<string, string>>({});

  const cleanupSeries = useCallback(() => {
    if (!chart) return;
    Object.values(seriesMapRef.current).forEach((s) => {
      chart.removeSeries(s);
    });
    seriesMapRef.current = {};
  }, [chart]);

  const indicatorData = useMemo(() => {
    const result: Record<string, unknown> = {};
    for (const ind of indicators) {
      if (!ind.visible) continue;
      switch (ind.name) {
        case 'ema_9':
          result.ema_9 = calculateEMA(bars, ind.params.period);
          break;
        case 'ema_21':
          result.ema_21 = calculateEMA(bars, ind.params.period);
          break;
        case 'rsi_14':
          result.rsi_14 = calculateRSI(bars, ind.params.period);
          break;
        case 'macd': {
          const macd = calculateMACD(bars, ind.params.fast, ind.params.slow, ind.params.signal);
          result.macd_macd = macd.macd;
          result.macd_signal = macd.signal;
          result.macd_histogram = macd.histogram;
          break;
        }
        case 'bollinger': {
          const bb = calculateBollinger(bars, ind.params.period, ind.params.stdDev);
          result.bb_upper = bb.upper;
          result.bb_middle = bb.middle;
          result.bb_lower = bb.lower;
          break;
        }
      }
    }
    return result;
  }, [bars, indicators]);

  useEffect(() => {
    if (!chart) return;
    cleanupSeries();

    for (const ind of indicators) {
      if (!ind.visible) continue;

      if (ind.panel === 'main') {
        if (ind.name === 'ema_9') {
          const s = chart.addLineSeries({ color: '#3b82f6', lineWidth: 2, title: 'EMA 9' });
          s.setData(indicatorData.ema_9 as LineData<Time>[]);
          seriesMapRef.current.ema_9 = s;
        }
        if (ind.name === 'ema_21') {
          const s = chart.addLineSeries({ color: '#f59e0b', lineWidth: 2, title: 'EMA 21' });
          s.setData(indicatorData.ema_21 as LineData<Time>[]);
          seriesMapRef.current.ema_21 = s;
        }
        if (ind.name === 'bollinger') {
          const upper = chart.addLineSeries({ color: '#a855f7', lineWidth: 1, title: 'BB Upper' });
          const middle = chart.addLineSeries({ color: '#a855f7', lineWidth: 1, title: 'BB Mid' });
          const lower = chart.addLineSeries({ color: '#a855f7', lineWidth: 1, title: 'BB Lower' });
          upper.setData(indicatorData.bb_upper as LineData<Time>[]);
          middle.setData(indicatorData.bb_middle as LineData<Time>[]);
          lower.setData(indicatorData.bb_lower as LineData<Time>[]);
          seriesMapRef.current.bb_upper = upper;
          seriesMapRef.current.bb_middle = middle;
          seriesMapRef.current.bb_lower = lower;
        }
      } else {
        if (ind.name === 'rsi_14') {
          const s = chart.addLineSeries({ color: '#06b6d4', lineWidth: 2, title: 'RSI' });
          s.setData(indicatorData.rsi_14 as LineData<Time>[]);
          const overbought = chart.addLineSeries({ color: '#ef4444', lineWidth: 1, title: '70' });
          const oversold = chart.addLineSeries({ color: '#22c55e', lineWidth: 1, title: '30' });
          const data = indicatorData.rsi_14 as LineData<Time>[];
          if (data.length > 0) {
            const firstTime = data[0].time;
            const lastTime = data[data.length - 1].time;
            overbought.setData([{ time: firstTime, value: 70 }, { time: lastTime, value: 70 }]);
            oversold.setData([{ time: firstTime, value: 30 }, { time: lastTime, value: 30 }]);
          }
          seriesMapRef.current.rsi = s;
        }
        if (ind.name === 'macd') {
          const hist = chart.addHistogramSeries({ color: '#22c55e', title: 'MACD Hist' });
          const sig = chart.addLineSeries({ color: '#f59e0b', lineWidth: 1, title: 'Signal' });
          hist.setData(indicatorData.macd_histogram as HistogramData<Time>[]);
          sig.setData(indicatorData.macd_signal as LineData<Time>[]);
          seriesMapRef.current.macd_histogram = hist;
          seriesMapRef.current.macd_signal = sig;
        }
        if (ind.name === 'volume') {
          const volData: HistogramData<Time>[] = bars.map((b) => ({
            time: Math.floor(b.timestamp / 1000) as UTCTimestamp,
            value: parseFloat(b.volume),
            color: parseFloat(b.close) >= parseFloat(b.open) ? '#22c55e80' : '#ef444480',
          }));
          const vol = chart.addHistogramSeries({ title: 'Volume' });
          vol.setData(volData);
          seriesMapRef.current.volume = vol;
        }
      }
    }

    return cleanupSeries;
  }, [chart, indicators, indicatorData, cleanupSeries]);

  useEffect(() => {
    if (!chart) return;
    chart.timeScale().fitContent();
  }, [chart, indicators]);

  return null;
}
