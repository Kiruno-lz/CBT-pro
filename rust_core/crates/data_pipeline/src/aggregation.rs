use crate::{error::DataError, storage::PostgresStorage, StandardBar, TimeFrame};
use rust_decimal::Decimal;
use tracing::{debug, info, warn};

/// Core engine for bar resampling and cache management.
///
/// When a PostgreSQL pool is available the engine stores and retrieves
/// pre-aggregated bars from cache tables, falling back to on-the-fly
/// computation from raw 1-minute data when necessary.
pub struct AggregationEngine {
    pg_pool: Option<sqlx::PgPool>,
}

impl AggregationEngine {
    /// Create a new engine.  `pg_pool` may be `None` when running offline.
    pub fn new(pg_pool: Option<sqlx::PgPool>) -> Self {
        Self { pg_pool }
    }

    /// Query bars for the given symbol and timeframe between `start` and `end`
    /// (inclusive, timestamps in seconds).
    ///
    /// 1. Try the aggregated cache first (if PostgreSQL is connected).
    /// 2. Fall back to raw 1-minute data and aggregate on-the-fly.
    pub async fn get_bars(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        start: i64,
        end: i64,
    ) -> Result<Vec<StandardBar>, DataError> {
        debug!(
            symbol,
            ?timeframe,
            start,
            end,
            "AggregationEngine::get_bars called"
        );

        if let Some(ref pool) = self.pg_pool {
            let storage = PostgresStorage::from_pool(pool.clone());

            // Fast path: try the cache table for this exact timeframe.
            match storage
                .query_aggregated(symbol, timeframe, start, end)
                .await
            {
                Ok(cached) if !cached.is_empty() => {
                    info!(
                        count = cached.len(),
                        "returning cached aggregated bars"
                    );
                    return Ok(cached);
                }
                Ok(_) => {
                    debug!("no cached aggregated bars found, falling back to 1m");
                }
                Err(e) => {
                    warn!(?e, "cached query failed, falling back to 1m");
                }
            }

            // Fallback: fetch raw 1m bars and aggregate.
            let raw = storage.query_bars(symbol, start, end).await?;
            if raw.is_empty() {
                return Err(DataError::NotFound(format!(
                    "no 1m bars for {} between {} and {}",
                    symbol, start, end
                )));
            }
            let aggregated = Self::aggregate_from_1m(&raw, timeframe)?;

            // Opportunistically store the result.
            if let Err(e) = self
                .store_aggregated(symbol, timeframe, &aggregated)
                .await
            {
                warn!(?e, "failed to store aggregated cache");
            }

            return Ok(aggregated);
        }

        Err(DataError::NotFound(format!(
            "no storage backend available for {}",
            symbol
        )))
    }

    /// Return the most recent closed bar for a symbol / timeframe.
    pub async fn get_latest_bar(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
    ) -> Result<Option<StandardBar>, DataError> {
        if let Some(ref pool) = self.pg_pool {
            let storage = PostgresStorage::from_pool(pool.clone());
            return storage.query_latest_aggregated(symbol, timeframe).await;
        }
        Ok(None)
    }

    /// Aggregate a slice of 1-minute bars into the target timeframe.
    ///
    /// Bucketing formula: `bucket_ts = (bar.timestamp / target_secs) * target_secs`
    pub fn aggregate_from_1m(
        bars: &[StandardBar],
        target: TimeFrame,
    ) -> Result<Vec<StandardBar>, DataError> {
        if bars.is_empty() {
            return Ok(Vec::new());
        }
        if target < TimeFrame::M1 {
            return Err(DataError::InvalidTimeFrame(format!(
                "cannot aggregate to finer granularity {:?}",
                target
            )));
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
                bucket_ts = ts;
                open = bar.open;
                high = bar.high;
                low = bar.low;
                close = bar.close;
                volume = bar.volume;
                symbol.clone_from(&bar.symbol);
                exchange.clone_from(&bar.exchange);
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

        info!(
            input = bars.len(),
            output = out.len(),
            ?target,
            "aggregation complete"
        );
        Ok(out)
    }

    /// Store aggregated bars into the cache table (best-effort).
    pub async fn store_aggregated(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        bars: &[StandardBar],
    ) -> Result<(), DataError> {
        if let Some(ref pool) = self.pg_pool {
            let storage = PostgresStorage::from_pool(pool.clone());
            storage
                .insert_aggregated(symbol, timeframe, bars)
                .await?;
        }
        Ok(())
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
            .map(|i| make_bar(i * 60, "100", "110", "90", "105", "10"))
            .collect();

        let agg = AggregationEngine::aggregate_from_1m(&bars, TimeFrame::M5).unwrap();
        assert_eq!(agg.len(), 1);
        let bar = &agg[0];
        assert_eq!(bar.timestamp, 0);
        assert_eq!(bar.open, Decimal::from(100));
        assert_eq!(bar.high, Decimal::from(110));
        assert_eq!(bar.low, Decimal::from(90));
        assert_eq!(bar.close, Decimal::from(105));
        assert_eq!(bar.volume, Decimal::from(50));
    }

    #[test]
    fn test_aggregate_m1_to_h1() {
        let bars: Vec<StandardBar> = (0..60)
            .map(|i| make_bar(i * 60, "50000", "51000", "49000", "50500", "1.5"))
            .collect();

        let agg = AggregationEngine::aggregate_from_1m(&bars, TimeFrame::H1).unwrap();
        assert_eq!(agg.len(), 1);
        let bar = &agg[0];
        assert_eq!(bar.timestamp, 0);
        assert_eq!(bar.open, Decimal::from(50000));
        assert_eq!(bar.high, Decimal::from(51000));
        assert_eq!(bar.low, Decimal::from(49000));
        assert_eq!(bar.close, Decimal::from(50500));
        assert_eq!(bar.volume, Decimal::from(90)); // 1.5 * 60 = 90
    }

    #[test]
    fn test_aggregate_multi_buckets() {
        // 10 minutes => two 5-minute buckets
        let bars: Vec<StandardBar> = (0..10)
            .map(|i| make_bar(i * 60, "100", "110", "90", "105", "10"))
            .collect();

        let agg = AggregationEngine::aggregate_from_1m(&bars, TimeFrame::M5).unwrap();
        assert_eq!(agg.len(), 2);
        assert_eq!(agg[0].timestamp, 0);
        assert_eq!(agg[0].volume, Decimal::from(50));
        assert_eq!(agg[1].timestamp, 300);
        assert_eq!(agg[1].volume, Decimal::from(50));
    }

    #[test]
    fn test_aggregate_empty() {
        let agg = AggregationEngine::aggregate_from_1m(&[], TimeFrame::M5).unwrap();
        assert!(agg.is_empty());
    }

    #[test]
    fn test_aggregate_preserves_decimal_precision() {
        let bars = vec![
            make_bar(0, "100.12345678", "101.00000001", "99.99999999", "100.50000000", "0.11111111"),
            make_bar(60, "100.50000000", "102.00000000", "100.00000000", "101.00000000", "0.22222222"),
            make_bar(120, "101.00000000", "103.00000000", "100.50000000", "102.00000000", "0.33333333"),
        ];

        let agg = AggregationEngine::aggregate_from_1m(&bars, TimeFrame::M5).unwrap();
        assert_eq!(agg.len(), 1);
        let bar = &agg[0];
        assert_eq!(bar.volume, Decimal::from_str_exact("0.66666666").unwrap());
        assert_eq!(bar.open, Decimal::from_str_exact("100.12345678").unwrap());
        assert_eq!(bar.high, Decimal::from_str_exact("103.00000000").unwrap());
        assert_eq!(bar.low, Decimal::from_str_exact("99.99999999").unwrap());
        assert_eq!(bar.close, Decimal::from_str_exact("102.00000000").unwrap());
    }
}
