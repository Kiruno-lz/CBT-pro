//! Data Pipeline crate for CBT-Pro.
//!
//! Handles all data ingestion, storage, aggregation, and querying for the
//! backtesting system.  All financial values are represented with
//! `rust_decimal::Decimal` to avoid floating-point drift.

use crate::error::DataError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub mod aggregation;
pub mod backtest;
pub mod bar;
pub mod cache;
pub mod error;
pub mod exchange;
pub mod fetcher;
pub mod storage;
pub mod symbol;

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
    M3,
    M5,
    M15,
    M30,
    H1,
    H4,
    D1,
    W1,
    /// Arbitrary interval in seconds.
    Custom(u64),
}

impl TimeFrame {
    /// Return the length of the interval in seconds as `u64`.
    pub fn as_secs(&self) -> u64 {
        match self {
            TimeFrame::M1 => 60,
            TimeFrame::M3 => 180,
            TimeFrame::M5 => 300,
            TimeFrame::M15 => 900,
            TimeFrame::M30 => 1800,
            TimeFrame::H1 => 3600,
            TimeFrame::H4 => 14400,
            TimeFrame::D1 => 86400,
            TimeFrame::W1 => 604800,
            TimeFrame::Custom(secs) => *secs,
        }
    }

    /// Return the length of the interval in seconds as `i64`.
    ///
    /// **Deprecated**: Use [`as_secs`](Self::as_secs) instead.
    pub fn as_seconds(&self) -> i64 {
        self.as_secs() as i64
    }

    /// Create a `TimeFrame` from a number of seconds.
    pub fn from_secs(secs: u64) -> Result<Self, DataError> {
        match secs {
            60 => Ok(TimeFrame::M1),
            180 => Ok(TimeFrame::M3),
            300 => Ok(TimeFrame::M5),
            900 => Ok(TimeFrame::M15),
            1800 => Ok(TimeFrame::M30),
            3600 => Ok(TimeFrame::H1),
            14400 => Ok(TimeFrame::H4),
            86400 => Ok(TimeFrame::D1),
            604800 => Ok(TimeFrame::W1),
            _ => Ok(TimeFrame::Custom(secs)),
        }
    }

    /// Parse a timeframe from its common string representation.
    pub fn parse(s: &str) -> Result<Self, DataError> {
        match s.to_lowercase().as_str() {
            "1m" | "m1" => Ok(TimeFrame::M1),
            "3m" | "m3" => Ok(TimeFrame::M3),
            "5m" | "m5" => Ok(TimeFrame::M5),
            "15m" | "m15" => Ok(TimeFrame::M15),
            "30m" | "m30" => Ok(TimeFrame::M30),
            "1h" | "h1" => Ok(TimeFrame::H1),
            "4h" | "h4" => Ok(TimeFrame::H4),
            "1d" | "d1" => Ok(TimeFrame::D1),
            "1w" | "w1" => Ok(TimeFrame::W1),
            _ => {
                if let Ok(secs) = s.parse::<u64>() {
                    Ok(TimeFrame::Custom(secs))
                } else {
                    Err(DataError::InvalidTimeFrame(s.to_string()))
                }
            }
        }
    }

    /// Parse a timeframe from its common string representation.
    ///
    /// Returns `None` for unknown strings (does not attempt numeric parsing).
    pub fn from_string(s: &str) -> Option<Self> {
        Self::parse(s).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeframe_as_secs() {
        assert_eq!(TimeFrame::M1.as_secs(), 60);
        assert_eq!(TimeFrame::M3.as_secs(), 180);
        assert_eq!(TimeFrame::M5.as_secs(), 300);
        assert_eq!(TimeFrame::M15.as_secs(), 900);
        assert_eq!(TimeFrame::M30.as_secs(), 1800);
        assert_eq!(TimeFrame::H1.as_secs(), 3600);
        assert_eq!(TimeFrame::H4.as_secs(), 14400);
        assert_eq!(TimeFrame::D1.as_secs(), 86400);
        assert_eq!(TimeFrame::W1.as_secs(), 604800);
        assert_eq!(TimeFrame::Custom(90).as_secs(), 90);
    }

    #[test]
    fn test_timeframe_as_seconds_backward_compat() {
        assert_eq!(TimeFrame::M1.as_seconds(), 60);
        assert_eq!(TimeFrame::M5.as_seconds(), 300);
    }

    #[test]
    fn test_timeframe_from_secs() {
        assert_eq!(TimeFrame::from_secs(60).unwrap(), TimeFrame::M1);
        assert_eq!(TimeFrame::from_secs(180).unwrap(), TimeFrame::M3);
        assert_eq!(TimeFrame::from_secs(300).unwrap(), TimeFrame::M5);
        assert_eq!(TimeFrame::from_secs(90).unwrap(), TimeFrame::Custom(90));
    }

    #[test]
    fn test_timeframe_parse() {
        assert_eq!(TimeFrame::parse("1m").unwrap(), TimeFrame::M1);
        assert_eq!(TimeFrame::parse("M1").unwrap(), TimeFrame::M1);
        assert_eq!(TimeFrame::parse("3m").unwrap(), TimeFrame::M3);
        assert_eq!(TimeFrame::parse("15m").unwrap(), TimeFrame::M15);
        assert_eq!(TimeFrame::parse("4h").unwrap(), TimeFrame::H4);
        assert_eq!(TimeFrame::parse("1d").unwrap(), TimeFrame::D1);
        assert_eq!(TimeFrame::parse("1w").unwrap(), TimeFrame::W1);
        assert_eq!(TimeFrame::parse("90").unwrap(), TimeFrame::Custom(90));
        assert!(TimeFrame::parse("bogus").is_err());
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

    #[test]
    fn test_timeframe_ordering() {
        assert!(TimeFrame::M1 < TimeFrame::M3);
        assert!(TimeFrame::M3 < TimeFrame::M5);
        assert!(TimeFrame::M5 < TimeFrame::M15);
    }
}
