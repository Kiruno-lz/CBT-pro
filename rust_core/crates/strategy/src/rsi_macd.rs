use crate::base::*;
use indicators::macd::macd;
use indicators::rsi::rsi;
use orderbook::{Direction, PositionStatus};
use rust_decimal::Decimal;

pub struct RsiMacd {
    pub symbol: String,
    pub quantity: Decimal,
    pub rsi_period: usize,
    pub macd_fast: usize,
    pub macd_slow: usize,
    pub macd_signal: usize,
}

impl RsiMacd {
    fn is_long(&self, ctx: &StrategyContext) -> bool {
        ctx.positions.iter().any(|p| {
            p.symbol == self.symbol
                && p.direction == Direction::Long
                && (p.status == PositionStatus::Open || p.status == PositionStatus::PartiallyClosed)
        })
    }
}

impl Strategy for RsiMacd {
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal> {
        let closes: Vec<Decimal> = ctx.historical_bars.iter().map(|b| b.close).collect();

        let rsi_results = match rsi(self.rsi_period, &closes) {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        let macd_results = match macd(self.macd_fast, self.macd_slow, self.macd_signal, &closes) {
            Ok(v) => v,
            Err(_) => return vec![],
        };

        if rsi_results.len() < 1 || macd_results.len() < 2 {
            return vec![];
        }

        let current_rsi = rsi_results.last().unwrap().value;
        let (_, current_macd) = macd_results.last().unwrap();
        let (_, prev_macd) = &macd_results[macd_results.len() - 2];

        let is_long = self.is_long(ctx);

        // OpenLong when RSI < 30 (oversold) AND MACD histogram turns positive
        if current_rsi < Decimal::from(30)
            && current_macd.histogram > Decimal::ZERO
            && prev_macd.histogram <= Decimal::ZERO
            && !is_long
        {
            return vec![Signal {
                action: SignalAction::OpenLong,
                symbol: self.symbol.clone(),
                quantity: Some(self.quantity),
                strength: 0.8,
                reason: "RSI oversold + MACD histogram bullish".to_string(),
                stop_loss: None,
                take_profit: None,
            }];
        }

        // CloseLong when RSI > 70 (overbought) OR MACD histogram turns negative
        if is_long
            && (current_rsi > Decimal::from(70)
                || (current_macd.histogram < Decimal::ZERO && prev_macd.histogram >= Decimal::ZERO))
        {
            return vec![Signal {
                action: SignalAction::CloseLong,
                symbol: self.symbol.clone(),
                quantity: Some(self.quantity),
                strength: 0.8,
                reason: "RSI overbought or MACD histogram bearish".to_string(),
                stop_loss: None,
                take_profit: None,
            }];
        }

        vec![]
    }
}
