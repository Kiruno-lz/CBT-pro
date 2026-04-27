import { useAppStore } from '../../stores/useAppStore';
import type { Position } from '../../types';

function formatPnL(value: string): { text: string; color: string } {
  const num = parseFloat(value);
  if (isNaN(num)) return { text: value, color: 'text-text-secondary' };
  if (num > 0) return { text: `+${value}`, color: 'text-accent-green' };
  if (num < 0) return { text: value, color: 'text-accent-red' };
  return { text: value, color: 'text-text-secondary' };
}

export function AccountDashboard() {
  const { snapshot, bars } = useAppStore();

  const equity = snapshot?.equity ?? '0';
  const unrealizedPnl = formatPnL(snapshot?.unrealized_pnl ?? '0');
  const realizedPnl = formatPnL(snapshot?.realized_pnl_today ?? '0');
  const marginRatio = snapshot?.margin_ratio ?? '0';
  const marginUsed = snapshot?.margin_used ?? '0';
  const availableBalance = snapshot?.available_balance ?? '0';
  const positions = snapshot?.positions ?? [];

  const totalReturn = bars.length > 0 && parseFloat(equity) > 0
    ? (((parseFloat(equity) - 10000) / 10000) * 100).toFixed(2)
    : '0.00';
  const totalReturnFormatted = formatPnL(`${totalReturn}%`);

  return (
    <div className="panel">
      <div className="panel-header">
        <span className="panel-title">Account</span>
        <span className="text-2xs font-mono text-text-muted">{positions.length} POS</span>
      </div>

      <div className="panel-body space-y-3">
        <div>
          <div className="label">Equity</div>
          <div className="metric-value text-text-primary">
            ${parseFloat(equity).toLocaleString('en-US', {
              minimumFractionDigits: 2,
              maximumFractionDigits: 2,
            })}
          </div>
        </div>

        <div className="grid grid-cols-2 gap-2">
          <div>
            <div className="label">Unrealized P&L</div>
            <div className={`text-sm font-mono tabular-nums ${unrealizedPnl.color}`}>
              {unrealizedPnl.text}
            </div>
          </div>
          <div>
            <div className="label">Realized P&L</div>
            <div className={`text-sm font-mono tabular-nums ${realizedPnl.color}`}>
              {realizedPnl.text}
            </div>
          </div>
        </div>

        <div>
          <div className="label">Total Return</div>
          <div className={`text-sm font-mono tabular-nums ${totalReturnFormatted.color}`}>
            {totalReturnFormatted.text}
          </div>
        </div>

        <div className="border-t border-border-subtle pt-3 space-y-2">
          <div className="flex justify-between items-center">
            <span className="text-2xs text-text-muted">Available</span>
            <span className="text-sm font-mono tabular-nums text-text-secondary">
              ${parseFloat(availableBalance).toLocaleString('en-US', {
                minimumFractionDigits: 2,
                maximumFractionDigits: 2,
              })}
            </span>
          </div>
          <div className="flex justify-between items-center">
            <span className="text-2xs text-text-muted">Margin Used</span>
            <span className="text-sm font-mono tabular-nums text-text-secondary">
              ${parseFloat(marginUsed).toLocaleString('en-US', {
                minimumFractionDigits: 2,
                maximumFractionDigits: 2,
              })}
            </span>
          </div>
          <div className="flex justify-between items-center">
            <span className="text-2xs text-text-muted">Margin Ratio</span>
            <span className={`text-sm font-mono tabular-nums ${
              parseFloat(marginRatio) > 80 ? 'text-accent-red' : 'text-text-secondary'
            }`}>
              {marginRatio}%
            </span>
          </div>
        </div>

        {positions.length > 0 && (
          <div className="border-t border-border-subtle pt-3">
            <div className="label mb-2">Open Positions</div>
            <div className="space-y-1.5">
              {positions.map((pos) => (
                <PositionRow key={pos.id} position={pos} />
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function PositionRow({ position }: { position: Position }) {
  const directionColor = position.direction === 'Long' ? 'text-accent-green' : 'text-accent-red';
  const unrealizedPnl = formatPnL(position.unrealized_pnl);
  const size = parseFloat(position.current_size);
  const avgPrice = parseFloat(position.average_entry_price);

  return (
    <div className="bg-surface-raised rounded p-2 text-xs">
      <div className="flex justify-between items-center mb-1">
        <span className={`font-semibold ${directionColor}`}>{position.direction}</span>
        <span className="font-mono tabular-nums text-text-primary">{position.symbol}</span>
      </div>
      <div className="flex justify-between items-center text-2xs">
        <span className="text-text-muted">
          {size.toFixed(4)} @ ${avgPrice.toFixed(2)}
        </span>
        <span className={`font-mono tabular-nums ${unrealizedPnl.color}`}>{unrealizedPnl.text}</span>
      </div>
    </div>
  );
}