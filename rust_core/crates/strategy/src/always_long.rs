use crate::base::*;
use rust_decimal::Decimal;

pub struct AlwaysLong {
    pub symbol: String,
    pub quantity: Decimal,
    has_position: bool,
}

impl AlwaysLong {
    pub fn new(symbol: String, quantity: Decimal) -> Self {
        Self {
            symbol,
            quantity,
            has_position: false,
        }
    }
}

impl Strategy for AlwaysLong {
    fn on_bar(&mut self, _ctx: &StrategyContext) -> Vec<Signal> {
        if self.has_position {
            return vec![];
        }
        self.has_position = true;
        vec![Signal {
            action: SignalAction::OpenLong,
            symbol: self.symbol.clone(),
            quantity: Some(self.quantity),
            strength: 1.0,
            reason: "AlwaysLong".to_string(),
            stop_loss: None,
            take_profit: None,
        }]
    }
}
