use crate::base::*;
use rust_decimal::Decimal;

pub struct Breakout {
    pub symbol: String,
    pub quantity: Decimal,
}

impl Strategy for Breakout {
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal> {
        // Placeholder: emit a signal every 30 bars
        if ctx.current_idx > 0 && ctx.current_idx % 30 == 0 {
            return vec![Signal {
                action: SignalAction::OpenLong,
                symbol: self.symbol.clone(),
                quantity: Some(self.quantity),
                strength: 0.9,
                reason: "Breakout placeholder".to_string(),
                stop_loss: None,
                take_profit: None,
            }];
        }
        vec![]
    }
}
