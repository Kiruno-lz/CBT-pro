use crate::base::*;
use rust_decimal::Decimal;

pub struct EmaCrossover {
    pub symbol: String,
    pub quantity: Decimal,
    pub fast_period: usize,
    pub slow_period: usize,
}

impl EmaCrossover {
    fn ema(values: &[Decimal], period: usize) -> Option<Decimal> {
        if values.len() < period {
            return None;
        }
        let slice = &values[values.len() - period..];
        let multiplier = Decimal::from(2) / Decimal::from(period + 1);
        let mut ema = slice[0];
        for val in &slice[1..] {
            ema = (*val - ema) * multiplier + ema;
        }
        Some(ema)
    }
}

impl Strategy for EmaCrossover {
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal> {
        if ctx.current_idx < self.slow_period + 1 {
            return vec![];
        }
        let available = ctx.historical_bars;
        let closes: Vec<Decimal> = available.iter().map(|b| b.close).collect();
        let fast_ema = match Self::ema(&closes, self.fast_period) {
            Some(v) => v,
            None => return vec![],
        };
        let slow_ema = match Self::ema(&closes, self.slow_period) {
            Some(v) => v,
            None => return vec![],
        };
        let prev_fast = match Self::ema(&closes[..closes.len() - 1], self.fast_period) {
            Some(v) => v,
            None => return vec![],
        };
        let prev_slow = match Self::ema(&closes[..closes.len() - 1], self.slow_period) {
            Some(v) => v,
            None => return vec![],
        };

        if prev_fast <= prev_slow && fast_ema > slow_ema {
            vec![Signal {
                action: SignalAction::OpenLong,
                symbol: self.symbol.clone(),
                quantity: Some(self.quantity),
                strength: 1.0,
                reason: "EMA crossover bullish".to_string(),
                stop_loss: None,
                take_profit: None,
            }]
        } else if prev_fast >= prev_slow && fast_ema < slow_ema {
            vec![Signal {
                action: SignalAction::CloseLong,
                symbol: self.symbol.clone(),
                quantity: Some(self.quantity),
                strength: 1.0,
                reason: "EMA crossover bearish".to_string(),
                stop_loss: None,
                take_profit: None,
            }]
        } else {
            vec![]
        }
    }
}
