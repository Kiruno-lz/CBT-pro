use crate::base::*;
use rust_decimal::Decimal;

pub struct RsiMacd {
    pub symbol: String,
    pub quantity: Decimal,
}

impl Strategy for RsiMacd {
    fn on_bar(&mut self, ctx: &StrategyContext) -> Vec<Signal> {
        // Placeholder: emit a signal every 20 bars
        if ctx.current_idx > 0 && ctx.current_idx % 20 == 0 {
            return vec![Signal {
                action: SignalAction::OpenLong,
                symbol: self.symbol.clone(),
                quantity: Some(self.quantity),
                strength: 0.8,
                reason: "RSI+MACD placeholder".to_string(),
                stop_loss: None,
                take_profit: None,
            }];
        }
        vec![]
    }
}
