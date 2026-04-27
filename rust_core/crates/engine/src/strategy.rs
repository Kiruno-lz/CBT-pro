use std::sync::atomic::{AtomicBool, Ordering};
use rust_decimal::Decimal;
use data_pipeline::StandardBar;
use crate::Signal;

pub trait Strategy: Send + Sync {
    fn on_bar(&self, bars: &[StandardBar], current_idx: usize) -> Option<Signal>;
}

pub struct AlwaysLong {
    pub symbol: String,
    pub quantity: Decimal,
    has_position: AtomicBool,
}

impl AlwaysLong {
    pub fn new(symbol: String, quantity: Decimal) -> Self {
        Self {
            symbol,
            quantity,
            has_position: AtomicBool::new(false),
        }
    }
}

impl Strategy for AlwaysLong {
    fn on_bar(&self, _bars: &[StandardBar], _current_idx: usize) -> Option<Signal> {
        if self.has_position.load(Ordering::Relaxed) {
            return None;
        }
        self.has_position.store(true, Ordering::Relaxed);
        Some(Signal {
            action: "open_long".to_string(),
            symbol: self.symbol.clone(),
            quantity: self.quantity,
            strength: 1.0,
            reason: "AlwaysLong".to_string(),
            timestamp: 0,
        })
    }
}

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
    fn on_bar(&self, bars: &[StandardBar], current_idx: usize) -> Option<Signal> {
        if current_idx < self.slow_period + 1 {
            return None;
        }
        let available = &bars[..current_idx];
        let closes: Vec<Decimal> = available.iter().map(|b| b.close).collect();
        let fast_ema = Self::ema(&closes, self.fast_period)?;
        let slow_ema = Self::ema(&closes, self.slow_period)?;
        let prev_fast = Self::ema(&closes[..closes.len() - 1], self.fast_period)?;
        let prev_slow = Self::ema(&closes[..closes.len() - 1], self.slow_period)?;

        if prev_fast <= prev_slow && fast_ema > slow_ema {
            Some(Signal {
                action: "open_long".to_string(),
                symbol: self.symbol.clone(),
                quantity: self.quantity,
                strength: 1.0,
                reason: "EMA crossover bullish".to_string(),
                timestamp: 0,
            })
        } else if prev_fast >= prev_slow && fast_ema < slow_ema {
            Some(Signal {
                action: "close_long".to_string(),
                symbol: self.symbol.clone(),
                quantity: self.quantity,
                strength: 1.0,
                reason: "EMA crossover bearish".to_string(),
                timestamp: 0,
            })
        } else {
            None
        }
    }
}

pub struct RsiMacd {
    pub symbol: String,
    pub quantity: Decimal,
}

impl Strategy for RsiMacd {
    fn on_bar(&self, _bars: &[StandardBar], current_idx: usize) -> Option<Signal> {
        // Placeholder: emit a signal every 20 bars
        if current_idx > 0 && current_idx % 20 == 0 {
            return Some(Signal {
                action: "open_long".to_string(),
                symbol: self.symbol.clone(),
                quantity: self.quantity,
                strength: 0.8,
                reason: "RSI+MACD placeholder".to_string(),
                timestamp: 0,
            });
        }
        None
    }
}

pub struct BollingerBands {
    pub symbol: String,
    pub quantity: Decimal,
}

impl Strategy for BollingerBands {
    fn on_bar(&self, _bars: &[StandardBar], current_idx: usize) -> Option<Signal> {
        // Placeholder: emit a signal every 25 bars
        if current_idx > 0 && current_idx % 25 == 0 {
            return Some(Signal {
                action: "open_long".to_string(),
                symbol: self.symbol.clone(),
                quantity: self.quantity,
                strength: 0.7,
                reason: "BollingerBands placeholder".to_string(),
                timestamp: 0,
            });
        }
        None
    }
}

pub struct Breakout {
    pub symbol: String,
    pub quantity: Decimal,
}

impl Strategy for Breakout {
    fn on_bar(&self, _bars: &[StandardBar], current_idx: usize) -> Option<Signal> {
        // Placeholder: emit a signal every 30 bars
        if current_idx > 0 && current_idx % 30 == 0 {
            return Some(Signal {
                action: "open_long".to_string(),
                symbol: self.symbol.clone(),
                quantity: self.quantity,
                strength: 0.9,
                reason: "Breakout placeholder".to_string(),
                timestamp: 0,
            });
        }
        None
    }
}
