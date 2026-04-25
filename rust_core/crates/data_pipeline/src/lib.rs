//! Data Pipeline crate for CBT-Pro.
//!
//! Handles all data ingestion, storage, aggregation, and querying for the
//! backtesting system.  All financial values are represented with
//! `rust_decimal::Decimal` to avoid floating-point drift.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub mod aggregation;
pub mod bar;
pub mod error;
pub mod exchange;
pub mod storage;

/// A single OHLCV bar normalised across every exchange.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StandardBar {
    /// Unix timestamp in **seconds**.
    pub timestamp: i64,
    /// Opening price.
    pub open: Decimal,
    /// Highest price seen during the interval.
    pub high: Decimal,
    /// Lowest price seen during the interval.
    pub low: Decimal,
    /// Closing price.
    pub close: Decimal,
    /// Traded volume.
    pub volume: Decimal,
    /// Trading pair, normalised (e.g. `"BTC-USDT"`).
    pub symbol: String,
    /// Exchange identifier (e.g. `"binance"`, `"okx"`).
    pub exchange: String,
    /// `true` when the bar / candle has closed.
    pub confirmed: bool,
}

/// Supported chart intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TimeFrame {
    M1,
    M5,
    M15,
    M30,
    H1,
    H4,
    D1,
    W1,
}

impl TimeFrame {
    /// Return the length of the interval in seconds.
    pub fn as_seconds(&self) -> i64 {
        match self {
            TimeFrame::M1 => 60,
            TimeFrame::M5 => 300,
            TimeFrame::M15 => 900,
            TimeFrame::M30 => 1800,
            TimeFrame::H1 => 3600,
            TimeFrame::H4 => 14400,
            TimeFrame::D1 => 86400,
            TimeFrame::W1 => 604800,
        }
    }

    /// Parse a timeframe from its common string representation.
    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "M1" | "1m" | "1M" => Some(TimeFrame::M1),
            "M5" | "5m" | "5M" => Some(TimeFrame::M5),
            "M15" | "15m" | "15M" => Some(TimeFrame::M15),
            "M30" | "30m" | "30M" => Some(TimeFrame::M30),
            "H1" | "1h" | "1H" => Some(TimeFrame::H1),
            "H4" | "4h" | "4H" => Some(TimeFrame::H4),
            "D1" | "1d" | "1D" => Some(TimeFrame::D1),
            "W1" | "1w" | "1W" => Some(TimeFrame::W1),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeframe_seconds() {
        assert_eq!(TimeFrame::M1.as_seconds(), 60);
        assert_eq!(TimeFrame::M5.as_seconds(), 300);
        assert_eq!(TimeFrame::M15.as_seconds(), 900);
        assert_eq!(TimeFrame::M30.as_seconds(), 1800);
        assert_eq!(TimeFrame::H1.as_seconds(), 3600);
        assert_eq!(TimeFrame::H4.as_seconds(), 14400);
        assert_eq!(TimeFrame::D1.as_seconds(), 86400);
        assert_eq!(TimeFrame::W1.as_seconds(), 604800);
    }

    #[test]
    fn test_timeframe_from_string() {
        assert_eq!(TimeFrame::from_string("M1"), Some(TimeFrame::M1));
        assert_eq!(TimeFrame::from_string("1m"), Some(TimeFrame::M1));
        assert_eq!(TimeFrame::from_string("15m"), Some(TimeFrame::M15));
        assert_eq!(TimeFrame::from_string("4h"), Some(TimeFrame::H4));
        assert_eq!(TimeFrame::from_string("1d"), Some(TimeFrame::D1));
        assert_eq!(TimeFrame::from_string("1w"), Some(TimeFrame::W1));
        assert_eq!(TimeFrame::from_string("bogus"), None);
    }
}
