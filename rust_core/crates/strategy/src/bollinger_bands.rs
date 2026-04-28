use crate::base::*;
use indicators::bollinger::bollinger;
use orderbook::{Direction, PositionStatus};
use rust_decimal::Decimal;

pub struct BollingerBands {
    pub symbol: String,
    pub quantity: Decimal,
    pub period: usize,
    pub std_dev: Decimal,
}

impl BollingerBands {
    fn is_long(&self, ctx: &StrategyContext) -> bool {
        ctx.positions.iter().any(|p| {
            p.symbol == self.symbol
                && p.direction == Direction::Long
                && (p.status == PositionStatus::Open || p.status == PositionStatus::PartiallyClosed)
        })
    }
}

impl Strategy for BollingerBands {
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal> {
        let closes: Vec<Decimal> = ctx.historical_bars.iter().map(|b| b.close).collect();

        let bb_results = match bollinger(self.period, self.std_dev, &closes) {
            Ok(v) => v,
            Err(_) => return vec![],
        };

        if bb_results.is_empty() {
            return vec![];
        }

        let current_close = *closes.last().unwrap();
        let (_, current_bb) = bb_results.last().unwrap();
        let is_long = self.is_long(ctx);

        // Buy signal when price touches or goes below lower band (mean reversion)
        if current_close <= current_bb.lower && !is_long {
            return vec![Signal {
                action: SignalAction::OpenLong,
                symbol: self.symbol.clone(),
                quantity: Some(self.quantity),
                strength: 0.7,
                reason: "Price below lower Bollinger Band".to_string(),
                stop_loss: None,
                take_profit: None,
            }];
        }

        // Sell signal when price touches or goes above upper band
        if current_close >= current_bb.upper && is_long {
            return vec![Signal {
                action: SignalAction::CloseLong,
                symbol: self.symbol.clone(),
                quantity: Some(self.quantity),
                strength: 0.7,
                reason: "Price above upper Bollinger Band".to_string(),
                stop_loss: None,
                take_profit: None,
            }];
        }

        vec![]
    }
}
