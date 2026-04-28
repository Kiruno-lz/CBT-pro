use crate::base::*;
use rust_decimal::Decimal;

pub struct BollingerBands {
    pub symbol: String,
    pub quantity: Decimal,
}

impl Strategy for BollingerBands {
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal> {
        // Placeholder: emit a signal every 25 bars
        if ctx.current_idx > 0 && ctx.current_idx % 25 == 0 {
            return vec![Signal {
                action: SignalAction::OpenLong,
                symbol: self.symbol.clone(),
                quantity: Some(self.quantity),
                strength: 0.7,
                reason: "BollingerBands placeholder".to_string(),
                stop_loss: None,
                take_profit: None,
            }];
        }
        vec![]
    }
}
