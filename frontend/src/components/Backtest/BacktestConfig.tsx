import { useState, useEffect, type RefObject } from 'react';
import { useAppStore } from '../../stores/useAppStore';
import { EngineWebSocket } from '../../stores/websocket';
import type { TimeFrame, StrategyDefaults, IndicatorConfig, ParamDefinition } from '../../types';

interface BacktestConfigForm {
  symbol: string;
  timeframe: TimeFrame;
  startDate: string;
  endDate: string;
  initialBalance: string;
  leverage: string;
  strategy: string;
}

function getDefaultDates() {
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);

  const sixMonthsAgo = new Date(yesterday);
  sixMonthsAgo.setMonth(sixMonthsAgo.getMonth() - 6);

  const formatDate = (date: Date) => {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, '0');
    const day = String(date.getDate()).padStart(2, '0');
    return `${year}-${month}-${day}`;
  };

  return {
    startDate: formatDate(sixMonthsAgo),
    endDate: formatDate(yesterday),
  };
}

const defaultDates = getDefaultDates();

const DEFAULT_FORM: BacktestConfigForm = {
  symbol: 'BTCUSDT',
  timeframe: 'H1',
  startDate: defaultDates.startDate,
  endDate: defaultDates.endDate,
  initialBalance: '10000',
  leverage: '10',
  strategy: 'ema_cross',
};

const TIMEFRAMES: TimeFrame[] = ['M1', 'M5', 'M15', 'M30', 'H1', 'H4', 'D1', 'W1'];

const TIMEFAME_MAP: Record<string, string> = {
  M1: '1m',
  M5: '5m',
  M15: '15m',
  M30: '30m',
  H1: '1h',
  H4: '4h',
  D1: '1d',
  W1: '1w',
};

const STRATEGY_ID_MAP: Record<string, string> = {
  always_long: 'always_long',
  ema_cross: 'ema_crossover',
  rsi_macd: 'rsi_macd',
  bollinger: 'bollinger_bands',
  breakout: 'breakout',
};

const STRATEGY_TO_INDICATORS: Record<string, IndicatorConfig[]> = {
  ema_crossover: [
    { name: 'ema_9', params: { period: 9 }, visible: true, panel: 'main' as const },
    { name: 'ema_21', params: { period: 21 }, visible: true, panel: 'main' as const },
  ],
  rsi_macd: [
    { name: 'rsi_14', params: { period: 14 }, visible: true, panel: 'sub' as const },
    { name: 'macd_12_26_9', params: { fast: 12, slow: 26, signal: 9 }, visible: true, panel: 'sub' as const },
  ],
  bollinger_bands: [
    { name: 'bollinger_20_2', params: { period: 20, stdDev: 2 }, visible: true, panel: 'main' as const },
  ],
  breakout: [],
  always_long: [],
};

const ALL_INDICATORS: IndicatorConfig[] = [
  { name: 'ema_9', params: { period: 9 }, visible: false, panel: 'main' },
  { name: 'ema_21', params: { period: 21 }, visible: false, panel: 'main' },
  { name: 'rsi_14', params: { period: 14 }, visible: false, panel: 'sub' },
  { name: 'macd_12_26_9', params: { fast: 12, slow: 26, signal: 9 }, visible: false, panel: 'sub' },
  { name: 'bollinger_20_2', params: { period: 20, stdDev: 2 }, visible: false, panel: 'main' },
  { name: 'atr_14', params: { period: 14 }, visible: false, panel: 'sub' },
  { name: 'vwap', params: {}, visible: false, panel: 'main' },
];

function dateToTimestamp(dateStr: string): number {
  return new Date(dateStr).getTime();
}

function formatSymbol(symbol: string): string {
  if (symbol.includes('-')) return symbol;
  const base = symbol.replace(/USDT$/, '');
  return `${base}-USDT`;
}

function getParamType(def: ParamDefinition): string {
  if (def.param_type.Integer) return 'Integer';
  if (def.param_type.Decimal) return 'Decimal';
  if (def.param_type.String) return 'String';
  return 'Unknown';
}

function generateIndicatorsFromParams(
  strategyId: string,
  params: Record<string, string | number>
): IndicatorConfig[] {
  // Start with all default indicators (all hidden by default)
  const indicators = ALL_INDICATORS.map((ind) => ({ ...ind }));

  // Generate active indicators based on CURRENT parameters
  const activeIndicators: IndicatorConfig[] = [];

  if (strategyId === 'ema_crossover') {
    for (const [key, value] of Object.entries(params)) {
      if (key.includes('period') && typeof value === 'number') {
        activeIndicators.push({
          name: `ema_${value}`,
          params: { period: value },
          visible: true,
          panel: 'main',
        });
      }
    }
  }

  if (strategyId === 'rsi_macd') {
    for (const [key, value] of Object.entries(params)) {
      if (key.includes('rsi') && key.includes('period') && typeof value === 'number') {
        activeIndicators.push({
          name: `rsi_${value}`,
          params: { period: value },
          visible: true,
          panel: 'sub',
        });
      }
    }

    const fast = params.macd_fast ?? params.fast;
    const slow = params.macd_slow ?? params.slow;
    const signal = params.macd_signal ?? params.signal;
    if (typeof fast === 'number' && typeof slow === 'number' && typeof signal === 'number') {
      activeIndicators.push({
        name: `macd_${fast}_${slow}_${signal}`,
        params: { fast, slow, signal },
        visible: true,
        panel: 'sub',
      });
    }
  }

  if (strategyId === 'bollinger_bands') {
    const period = Number(params.period ?? params.bollinger_period);
    const stdDev = Number(params.std_dev ?? params.stdDev ?? params.bollinger_std_dev);
    console.log('generateIndicatorsFromParams bollinger - period:', period, 'stdDev:', stdDev, 'types:', typeof period, typeof stdDev);
    if (!isNaN(period) && !isNaN(stdDev)) {
      activeIndicators.push({
        name: `bollinger_${period}_${stdDev}`,
        params: { period, stdDev },
        visible: true,
        panel: 'main',
      });
      console.log('Added active indicator:', `bollinger_${period}_${stdDev}`);
    } else {
      console.log('Failed to add bollinger indicator - invalid number');
    }
  }

  // Apply active indicators - activate matching ones, keep others inactive
  for (const active of activeIndicators) {
    const existing = indicators.find((ind) => ind.name === active.name);
    if (existing) {
      existing.visible = true;
    } else {
      indicators.push(active);
    }
  }

  return indicators;
}

const API_BASE = import.meta.env.DEV ? '' : (import.meta.env.VITE_API_BASE || '');

interface BacktestConfigProps {
  wsRef: RefObject<EngineWebSocket | null>;
}

export function BacktestConfig({ wsRef }: BacktestConfigProps) {
  const { setPlayback, setWsConnected, setIndicators, setBacktestId, setCurrentStrategy } = useAppStore();
  const [form, setForm] = useState<BacktestConfigForm>(DEFAULT_FORM);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [strategyDefaults, setStrategyDefaults] = useState<StrategyDefaults | null>(null);
  const [strategyParams, setStrategyParams] = useState<Record<string, string | number>>({});
  const [paramsExpanded, setParamsExpanded] = useState(false);

  useEffect(() => {
    const strategyId = STRATEGY_ID_MAP[form.strategy];
    if (!strategyId) return;

    setStrategyDefaults(null);
    setStrategyParams({});
    setParamsExpanded(false);

    fetch(`${API_BASE}/api/strategies/${strategyId}/defaults`)
      .then((res) => {
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return res.json();
      })
      .then((data: StrategyDefaults) => {
        setStrategyDefaults(data);
        const defaults: Record<string, string | number> = {};
        for (const [key, value] of Object.entries(data.default_params)) {
          defaults[key] = value as string | number;
        }
        setStrategyParams(defaults);
        setCurrentStrategy(data);
        
        // Generate indicators from default params
        console.log('Initial load - defaults:', defaults);
        const defaultIndicators = generateIndicatorsFromParams(strategyId, defaults);
        console.log('Initial load - defaultIndicators:', defaultIndicators.map(i => ({ name: i.name, visible: i.visible })));
        setIndicators(defaultIndicators);
      })
      .catch(() => {
        setStrategyDefaults(null);
        setStrategyParams({});
        setIndicators(ALL_INDICATORS.map(ind => ({ ...ind })));
      });
  }, [form.strategy, setIndicators, setCurrentStrategy]);

  const handleChange = (field: keyof BacktestConfigForm, value: string) => {
    setForm((prev) => ({ ...prev, [field]: value }));
    setError(null);
  };

  const handleParamChange = (name: string, value: string | number) => {
    console.log('handleParamChange:', name, value);
    setStrategyParams((prev) => {
      const updated = { ...prev, [name]: value };
      console.log('strategyParams updated:', updated);
      return updated;
    });
  };

  useEffect(() => {
    if (!strategyDefaults || Object.keys(strategyParams).length === 0) return;

    const strategyId = STRATEGY_ID_MAP[form.strategy];
    if (!strategyId) return;

    console.log('generateIndicatorsFromParams called with:', strategyId, strategyParams);
    const newIndicators = generateIndicatorsFromParams(strategyId, strategyParams);
    console.log('newIndicators:', newIndicators.map(i => ({ name: i.name, visible: i.visible })));
    const currentIndicators = useAppStore.getState().indicators;

    // Only update if the arrays are actually different
    const currentNames = currentIndicators.map(i => i.name).join(',');
    const newNames = newIndicators.map(i => i.name).join(',');
    console.log('currentNames:', currentNames);
    console.log('newNames:', newNames);

    if (currentNames !== newNames) {
      console.log('Updating indicators');
      setIndicators(newIndicators);
    } else {
      console.log('Indicators unchanged, skipping update');
    }
  }, [strategyParams, strategyDefaults, form.strategy, setIndicators]);

  const handleStart = async () => {
    setLoading(true);
    setError(null);

    try {
      const response = await fetch(`${API_BASE}/api/backtest/start`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          config: {
            symbol: formatSymbol(form.symbol),
            initial_balance: form.initialBalance,
            margin_mode: 'Isolated',
            default_leverage: form.leverage,
          },
          strategy_id: STRATEGY_ID_MAP[form.strategy] || form.strategy,
          strategy_params: strategyParams,
          timeframe: TIMEFAME_MAP[form.timeframe] || form.timeframe,
          start_time: dateToTimestamp(form.startDate),
          end_time: dateToTimestamp(form.endDate),
        }),
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const data = await response.json();
      wsRef.current?.subscribe(data.backtest_id);

      setPlayback({
        status: 'paused',
        totalBars: data.total_bars || 0,
        currentBarIndex: 0,
        currentTime: 0,
      });

      setWsConnected(true);
      setBacktestId(data.backtest_id);
      if (strategyDefaults) {
        setCurrentStrategy({ ...strategyDefaults, default_params: strategyParams });
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to start backtest');
    } finally {
      setLoading(false);
    }
  };

  const hasParamDefinitions = strategyDefaults && strategyDefaults.param_definitions.length > 0;

  return (
    <div className="panel flex-shrink-0">
      <div className="panel-header">
        <span className="panel-title">Backtest Config</span>
      </div>

      <div className="panel-body space-y-4">
        <div className="space-y-1.5">
          <label className="label">Symbol</label>
          <select
            value={form.symbol}
            onChange={(e) => handleChange('symbol', e.target.value)}
            className="input-field"
          >
            <option value="BTCUSDT">BTC/USDT</option>
            <option value="ETHUSDT">ETH/USDT</option>
            <option value="BNBUSDT">BNB/USDT</option>
          </select>
        </div>

        <div className="space-y-1.5">
          <label className="label">Timeframe</label>
          <div className="flex gap-1">
            {TIMEFRAMES.map((tf) => (
              <button
                key={tf}
                onClick={() => handleChange('timeframe', tf)}
                className={`flex-1 py-1 text-2xs font-mono rounded transition-colors ${
                  form.timeframe === tf
                    ? 'bg-accent-cyan text-surface-base'
                    : 'bg-surface-raised text-text-secondary hover:bg-surface-elevated'
                }`}
              >
                {tf}
              </button>
            ))}
          </div>
        </div>

        <div className="grid grid-cols-2 gap-2">
          <div className="space-y-1.5">
            <label className="label">Start Date</label>
            <input
              type="date"
              value={form.startDate}
              onChange={(e) => handleChange('startDate', e.target.value)}
              className="input-field"
            />
          </div>
          <div className="space-y-1.5">
            <label className="label">End Date</label>
            <input
              type="date"
              value={form.endDate}
              onChange={(e) => handleChange('endDate', e.target.value)}
              className="input-field"
            />
          </div>
        </div>

        <div className="grid grid-cols-2 gap-2">
          <div className="space-y-1.5">
            <label className="label">Initial Balance</label>
            <input
              type="number"
              value={form.initialBalance}
              onChange={(e) => handleChange('initialBalance', e.target.value)}
              className="input-field font-mono"
              min="100"
              step="100"
            />
          </div>
          <div className="space-y-1.5">
            <label className="label">Leverage</label>
            <input
              type="number"
              value={form.leverage}
              onChange={(e) => handleChange('leverage', e.target.value)}
              className="input-field font-mono"
              min="1"
              max="100"
            />
          </div>
        </div>

        <div className="space-y-1.5">
          <label className="label">Strategy</label>
          <select
            value={form.strategy}
            onChange={(e) => handleChange('strategy', e.target.value)}
            className="input-field"
          >
            <option value="always_long">Always Long</option>
            <option value="ema_cross">EMA Crossover</option>
            <option value="rsi_macd">RSI + MACD</option>
            <option value="bollinger">Bollinger Bands</option>
            <option value="breakout">Breakout</option>
          </select>
        </div>

        {hasParamDefinitions && (
          <div className="border border-surface-raised rounded overflow-hidden">
            <button
              onClick={() => setParamsExpanded((prev) => !prev)}
              className="w-full flex items-center justify-between px-3 py-2 bg-surface-raised hover:bg-surface-elevated transition-colors"
            >
              <span className="text-xs font-medium">Strategy Parameters</span>
              <svg
                className={`w-4 h-4 transition-transform ${paramsExpanded ? 'rotate-180' : ''}`}
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
              </svg>
            </button>
            {paramsExpanded && (
              <div className="p-3 space-y-3">
                {strategyDefaults.param_definitions.map((def) => {
                  const paramType = getParamType(def);
                  const value = strategyParams[def.name];

                  return (
                    <div key={def.name} className="space-y-1">
                      <label className="text-xs text-text-secondary">{def.name}</label>
                      {paramType === 'Integer' && def.param_type.Integer && (
                        <input
                          type="number"
                          min={def.param_type.Integer.min}
                          max={def.param_type.Integer.max}
                          value={value ?? def.param_type.Integer.default}
                          onChange={(e) => handleParamChange(def.name, parseInt(e.target.value, 10))}
                          className="input-field font-mono text-xs"
                        />
                      )}
                      {paramType === 'Decimal' && def.param_type.Decimal && (
                        <input
                          type="number"
                          step="0.1"
                          min={parseFloat(def.param_type.Decimal.min)}
                          max={parseFloat(def.param_type.Decimal.max)}
                          value={value ?? def.param_type.Decimal.default}
                          onChange={(e) => handleParamChange(def.name, parseFloat(e.target.value))}
                          className="input-field font-mono text-xs"
                        />
                      )}
                      {paramType === 'String' && def.param_type.String && (
                        <select
                          value={value ?? def.param_type.String.default}
                          onChange={(e) => handleParamChange(def.name, e.target.value)}
                          className="input-field text-xs"
                        >
                          {def.param_type.String.options.map((opt) => (
                            <option key={opt} value={opt}>
                              {opt}
                            </option>
                          ))}
                        </select>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}


        {error && (
          <div className="bg-accent-red/10 border border-accent-red/30 rounded p-2">
            <span className="text-xs text-accent-red">{error}</span>
          </div>
        )}

        <button onClick={handleStart} disabled={loading} className="btn-primary w-full">
          {loading ? (
            <span className="flex items-center justify-center gap-2">
              <svg className="animate-spin w-4 h-4" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>
              Starting...
            </span>
          ) : (
            'START BACKTEST'
          )}
        </button>
      </div>
    </div>
  );
}
