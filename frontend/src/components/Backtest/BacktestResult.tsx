import { useAppStore } from '../../stores/useAppStore';

function formatPnL(value: number, suffix = ''): { text: string; color: string } {
  if (value > 0) return { text: `+${value.toFixed(2)}${suffix}`, color: 'text-accent-green' };
  if (value < 0) return { text: `${value.toFixed(2)}${suffix}`, color: 'text-accent-red' };
  return { text: `0.00${suffix}`, color: 'text-text-secondary' };
}

function MetricCard({
  label,
  value,
  subValue,
  color = 'text-text-primary',
}: {
  label: string;
  value: string;
  subValue?: string;
  color?: string;
}) {
  return (
    <div className="bg-surface-raised rounded p-2">
      <div className="label mb-0.5">{label}</div>
      <div className={`text-sm font-mono tabular-nums font-semibold ${color}`}>
        {value}
      </div>
      {subValue && (
        <div className="text-2xs text-text-muted font-mono mt-0.5">{subValue}</div>
      )}
    </div>
  );
}

export function BacktestResult() {
  const { backtestResult } = useAppStore();

  if (!backtestResult) return null;

  const {
    final_equity,
    total_return_pct,
    max_drawdown_pct,
    sharpe_ratio,
    total_trades,
    win_rate,
    profit_factor,
    avg_trade_return,
    trades,
  } = backtestResult;

  const totalReturnFormatted = formatPnL(total_return_pct, '%');
  const maxDrawdownFormatted = formatPnL(max_drawdown_pct, '%');
  const avgTradeFormatted = formatPnL(avg_trade_return, '%');
  const winRateFormatted = (win_rate * 100).toFixed(1);

  const winners = trades.filter((t) => parseFloat(t.realized_pnl || '0') > 0);
  const losers = trades.filter((t) => parseFloat(t.realized_pnl || '0') < 0);

  return (
    <div className="panel flex-shrink-0">
      <div className="panel-header">
        <span className="panel-title">Backtest Result</span>
        <span className="text-2xs font-mono text-accent-cyan">
          {backtestResult.backtest_id.slice(0, 8)}
        </span>
      </div>

      <div className="panel-body space-y-4">
        <div className="bg-surface-elevated rounded p-3 text-center">
          <div className="label">Final Equity</div>
          <div className="text-xl font-mono tabular-nums font-bold text-text-primary mt-1">
            ${parseFloat(final_equity).toLocaleString('en-US', {
              minimumFractionDigits: 2,
              maximumFractionDigits: 2,
            })}
          </div>
          <div className={`text-sm font-mono tabular-nums mt-1 ${totalReturnFormatted.color}`}>
            {totalReturnFormatted.text}
          </div>
        </div>

        <div className="grid grid-cols-2 gap-2">
          <MetricCard
            label="Max Drawdown"
            value={maxDrawdownFormatted.text}
            color={max_drawdown_pct > 20 ? 'text-accent-red' : 'text-accent-amber'}
          />
          <MetricCard
            label="Sharpe Ratio"
            value={sharpe_ratio.toFixed(2)}
            color={sharpe_ratio > 1 ? 'text-accent-green' : 'text-text-secondary'}
          />
        </div>

        <div className="border-t border-border-subtle pt-4">
          <div className="label mb-2">Trade Statistics</div>
          <div className="space-y-2">
            <div className="flex justify-between items-center text-xs">
              <span className="text-text-muted">Total Trades</span>
              <span className="font-mono tabular-nums text-text-primary">
                {total_trades}
              </span>
            </div>
            <div className="flex justify-between items-center text-xs">
              <span className="text-text-muted">Win Rate</span>
              <span className={`font-mono tabular-nums ${
                parseFloat(winRateFormatted) >= 50 ? 'text-accent-green' : 'text-accent-red'
              }`}>
                {winRateFormatted}%
              </span>
            </div>
            <div className="flex justify-between items-center text-xs">
              <span className="text-text-muted">Profit Factor</span>
              <span className={`font-mono tabular-nums ${
                profit_factor >= 1.5 ? 'text-accent-green' : profit_factor >= 1 ? 'text-accent-amber' : 'text-accent-red'
              }`}>
                {profit_factor.toFixed(2)}
              </span>
            </div>
            <div className="flex justify-between items-center text-xs">
              <span className="text-text-muted">Avg Trade Return</span>
              <span className={`font-mono tabular-nums ${avgTradeFormatted.color}`}>
                {avgTradeFormatted.text}
              </span>
            </div>
          </div>
        </div>

        {trades.length > 0 && (
          <div className="border-t border-border-subtle pt-4">
            <div className="label mb-2">Trade Summary</div>
            <div className="flex gap-4 text-xs">
              <div className="flex items-center gap-1.5">
                <div className="w-2 h-2 rounded-full bg-accent-green" />
                <span className="text-text-muted">Winners</span>
                <span className="font-mono tabular-nums text-accent-green">
                  {winners.length}
                </span>
              </div>
              <div className="flex items-center gap-1.5">
                <div className="w-2 h-2 rounded-full bg-accent-red" />
                <span className="text-text-muted">Losers</span>
                <span className="font-mono tabular-nums text-accent-red">
                  {losers.length}
                </span>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}