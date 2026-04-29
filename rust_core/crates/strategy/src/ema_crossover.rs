use crate::base::*;
use indicators::ema::ema;
use rust_decimal::Decimal;

pub struct EmaCrossover {
    pub symbol: String,
    pub quantity: Decimal,
    pub fast_period: usize,
    pub slow_period: usize,
}

impl Strategy for EmaCrossover {
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal> {
        let closes: Vec<Decimal> = ctx.historical_bars.iter().map(|b| b.close).collect();

        let fast_results = match ema(self.fast_period, &closes) {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        let slow_results = match ema(self.slow_period, &closes) {
            Ok(v) => v,
            Err(_) => return vec![],
        };

        if fast_results.len() < 2 || slow_results.len() < 2 {
            return vec![];
        }

        let fast_ema = fast_results.last().unwrap().value;
        let slow_ema = slow_results.last().unwrap().value;
        let prev_fast = fast_results[fast_results.len() - 2].value;
        let prev_slow = slow_results[slow_results.len() - 2].value;

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
