use crate::{error::DataError, StandardBar, TimeFrame};
use futures::Stream;
use rust_decimal::Decimal;

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
    pub fn from_1m_bars(bars: Vec<StandardBar>, target: TimeFrame) -> Vec<StandardBar> {
        if bars.is_empty() {
            return Vec::new();
        }

        let target_secs = target.as_seconds();
        let mut out: Vec<StandardBar> = Vec::new();
        let mut bucket_ts: i64 = 0;
        let mut open = Decimal::ZERO;
        let mut high = Decimal::ZERO;
        let mut low = Decimal::ZERO;
        let mut close = Decimal::ZERO;
        let mut volume = Decimal::ZERO;
        let mut count = 0usize;
        let mut symbol = String::new();
        let mut exchange = String::new();

        for bar in bars {
            let ts = (bar.timestamp / target_secs) * target_secs;

            if count == 0 || ts != bucket_ts {
                // Flush the previous bucket
                if count > 0 {
                    out.push(StandardBar {
                        timestamp: bucket_ts,
                        open,
                        high,
                        low,
                        close,
                        volume,
                        symbol: symbol.clone(),
                        exchange: exchange.clone(),
                        confirmed: true,
                    });
                }
                // Start a new bucket
                bucket_ts = ts;
                open = bar.open;
                high = bar.high;
                low = bar.low;
                close = bar.close;
                volume = bar.volume;
                symbol = bar.symbol;
                exchange = bar.exchange;
                count = 1;
            } else {
                if bar.high > high {
                    high = bar.high;
                }
                if bar.low < low {
                    low = bar.low;
                }
                close = bar.close;
                volume += bar.volume;
                count += 1;
            }
        }

        // Flush the final bucket
        if count > 0 {
            out.push(StandardBar {
                timestamp: bucket_ts,
                open,
                high,
                low,
                close,
                volume,
                symbol,
                exchange,
                confirmed: true,
            });
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let agg = BarBuilder::from_1m_bars(bars, TimeFrame::M5);
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

        let agg = BarBuilder::from_1m_bars(bars, TimeFrame::H1);
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

        let agg = BarBuilder::from_1m_bars(bars, TimeFrame::M5);
        assert_eq!(agg.len(), 1);
        let bar = &agg[0];
        // Volume = 0.33333333 * 3 = 0.99999999  (exact Decimal math, no fp drift)
        assert_eq!(bar.volume, Decimal::from_str_exact("0.99999999").unwrap());
        assert_eq!(bar.open, Decimal::from_str_exact("100.12345678").unwrap());
        assert_eq!(bar.high, Decimal::from_str_exact("101.23456789").unwrap());
        assert_eq!(bar.low, Decimal::from_str_exact("99.87654321").unwrap());
        assert_eq!(bar.close, Decimal::from_str_exact("100.50000000").unwrap());
    }
}
