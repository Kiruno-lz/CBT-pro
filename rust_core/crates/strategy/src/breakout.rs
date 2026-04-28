use crate::base::*;
use orderbook::{Direction, PositionStatus};
use rust_decimal::Decimal;

pub struct Breakout {
    pub symbol: String,
    pub quantity: Decimal,
    pub lookback: usize,
    pub threshold_pct: Decimal,
}

impl Breakout {
    fn is_long(&self, ctx: &StrategyContext) -> bool {
        ctx.positions.iter().any(|p| {
            p.symbol == self.symbol
                && p.direction == Direction::Long
                && (p.status == PositionStatus::Open || p.status == PositionStatus::PartiallyClosed)
        })
    }
}

impl Strategy for Breakout {
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal> {
        if ctx.historical_bars.len() <= self.lookback {
            return vec![];
        }

        let recent_bars =
            &ctx.historical_bars[ctx.historical_bars.len() - self.lookback..ctx.historical_bars.len()];
        let current_close = ctx.current_bar.close;

        let highest_high = recent_bars.iter().map(|b| b.high).max().unwrap();
        let lowest_low = recent_bars.iter().map(|b| b.low).min().unwrap();

        let range = highest_high - lowest_low;
        let threshold = range * self.threshold_pct / Decimal::from(100);

        let is_long = self.is_long(ctx);

        // OpenLong when price breaks above resistance
        if current_close > highest_high + threshold && !is_long {
            return vec![Signal {
                action: SignalAction::OpenLong,
                symbol: self.symbol.clone(),
                quantity: Some(self.quantity),
                strength: 0.9,
                reason: "Breakout above resistance".to_string(),
                stop_loss: None,
                take_profit: None,
            }];
        }

        // CloseLong when price breaks below support
        if current_close < lowest_low - threshold && is_long {
            return vec![Signal {
                action: SignalAction::CloseLong,
                symbol: self.symbol.clone(),
                quantity: Some(self.quantity),
                strength: 0.9,
                reason: "Breakdown below support".to_string(),
                stop_loss: None,
                take_profit: None,
            }];
        }

        vec![]
    }
}
