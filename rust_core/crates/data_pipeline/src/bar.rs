use crate::{error::DataError, StandardBar, TimeFrame};
use futures::Stream;

/// Trait for any async stream that yields `StandardBar` results.
pub trait BarStream: Stream<Item = Result<StandardBar, DataError>> + Send + Unpin {}

/// Blanket implementation so every compatible stream is a `BarStream`.
impl<T> BarStream for T where T: Stream<Item = Result<StandardBar, DataError>> + Send + Unpin {}

/// Convenience builder for constructing bars and resampling 1-minute data.
#[derive(Debug, Clone)]
pub struct BarBuilder {
    pub symbol: String,
    pub exchange: String,
    pub timeframe: TimeFrame,
}

impl BarBuilder {
    /// Create a new builder for the given symbol, exchange and target timeframe.
    pub fn new(symbol: &str, exchange: &str, timeframe: TimeFrame) -> Self {
        Self {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            timeframe,
        }
    }

    /// Resample a vector of 1-minute bars into the requested `target` timeframe.
    ///
    /// The bars are assumed to be sorted by `timestamp` in ascending order.
    ///
    /// Delegates to [`AggregationEngine::aggregate`](crate::aggregation::AggregationEngine::aggregate).
    pub fn from_1m_bars(
        bars: Vec<StandardBar>,
        target: TimeFrame,
    ) -> Result<Vec<StandardBar>, DataError> {
        crate::aggregation::AggregationEngine::aggregate(&bars, target.as_secs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn make_bar(ts: i64, o: &str, h: &str, l: &str, c: &str, v: &str) -> StandardBar {
        StandardBar {
            timestamp: ts,
            open: o.parse().unwrap(),
            high: h.parse().unwrap(),
            low: l.parse().unwrap(),
            close: c.parse().unwrap(),
            volume: v.parse().unwrap(),
            symbol: "BTC-USDT".to_string(),
            exchange: "binance".to_string(),
            confirmed: true,
        }
    }

    #[test]
    fn test_aggregate_m1_to_m5() {
        let bars: Vec<StandardBar> = (0..5)
            .map(|i| make_bar(i * 60, "100.0", "105.0", "95.0", "102.0", "1.0"))
            .collect();

        let agg = BarBuilder::from_1m_bars(bars, TimeFrame::M5).unwrap();
        assert_eq!(agg.len(), 1);
        let bar = &agg[0];
        assert_eq!(bar.timestamp, 0);
        assert_eq!(bar.open, Decimal::from(100));
        assert_eq!(bar.high, Decimal::from(105));
        assert_eq!(bar.low, Decimal::from(95));
        assert_eq!(bar.close, Decimal::from(102));
        assert_eq!(bar.volume, Decimal::from(5));
    }

    #[test]
    fn test_aggregate_m1_to_h1() {
        let bars: Vec<StandardBar> = (0..60)
            .map(|i| make_bar(i * 60, "100.0", "105.0", "95.0", "102.0", "1.0"))
            .collect();

        let agg = BarBuilder::from_1m_bars(bars, TimeFrame::H1).unwrap();
        assert_eq!(agg.len(), 1);
        let bar = &agg[0];
        assert_eq!(bar.timestamp, 0);
        assert_eq!(bar.open, Decimal::from(100));
        assert_eq!(bar.high, Decimal::from(105));
        assert_eq!(bar.low, Decimal::from(95));
        assert_eq!(bar.close, Decimal::from(102));
        assert_eq!(bar.volume, Decimal::from(60));
    }

    #[test]
    fn test_decimal_precision() {
        let bars: Vec<StandardBar> = (0..3)
            .map(|i| {
                make_bar(
                    i * 60,
                    "100.12345678",
                    "101.23456789",
                    "99.87654321",
                    "100.50000000",
                    "0.33333333",
                )
            })
            .collect();

        let agg = BarBuilder::from_1m_bars(bars, TimeFrame::M5).unwrap();
        assert_eq!(agg.len(), 1);
        let bar = &agg[0];
        // Volume = 0.33333333 * 3 = 0.99999999  (exact Decimal math, no fp drift)
        assert_eq!(bar.volume, Decimal::from_str_exact("0.99999999").unwrap());
        assert_eq!(bar.open, Decimal::from_str_exact("100.12345678").unwrap());
        assert_eq!(bar.high, Decimal::from_str_exact("101.23456789").unwrap());
        assert_eq!(bar.low, Decimal::from_str_exact("99.87654321").unwrap());
        assert_eq!(bar.close, Decimal::from_str_exact("100.50000000").unwrap());
    }

    #[test]
    fn test_bar_builder_empty() {
        let bars: Vec<StandardBar> = vec![];
        let agg = BarBuilder::from_1m_bars(bars, TimeFrame::M5).unwrap();
        assert!(agg.is_empty());
    }
}
