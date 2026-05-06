use crate::{
    cache::{CacheKey, CacheProvider, MemoryCacheProvider},
    error::DataError,
    storage::PostgresStorage,
    StandardBar, TimeFrame,
};
use std::collections::BTreeMap;
use std::time::Duration;
use tracing::{debug, info};

/// Core engine for bar resampling.
///
/// Computes aggregated bars on-the-fly from raw 1-minute data stored in
/// PostgreSQL.  No pre-computed cache tables are used.
///
/// Caching is provided by a pluggable [`CacheProvider`] — by default an
/// in-memory LRU cache is used.
#[derive(Debug)]
pub struct AggregationEngine {
    pg_pool: Option<sqlx::PgPool>,
    cache_provider: Box<dyn CacheProvider>,
}

impl AggregationEngine {
    /// Create a new engine with the default in-memory cache (1000 entries).
    ///
    /// `pg_pool` may be `None` when running offline.
    pub fn new(pg_pool: Option<sqlx::PgPool>) -> Self {
        Self::with_memory_cache(pg_pool, 1000)
    }

    /// Create a new engine with a specific in-memory cache size.
    pub fn with_cache_size(pg_pool: Option<sqlx::PgPool>, cache_size: usize) -> Self {
        Self::with_memory_cache(pg_pool, cache_size)
    }

    /// Create a new engine with an in-memory LRU cache.
    pub fn with_memory_cache(pg_pool: Option<sqlx::PgPool>, capacity: usize) -> Self {
        Self {
            pg_pool,
            cache_provider: Box::new(MemoryCacheProvider::new(capacity)),
        }
    }

    /// Create a new engine with a custom cache provider.
    pub fn with_cache(provider: Box<dyn CacheProvider>) -> Self {
        Self {
            pg_pool: None,
            cache_provider: provider,
        }
    }

    /// Create a new engine with both a PostgreSQL pool and a custom cache provider.
    pub fn with_pool_and_cache(
        pg_pool: Option<sqlx::PgPool>,
        provider: Box<dyn CacheProvider>,
    ) -> Self {
        Self {
            pg_pool,
            cache_provider: provider,
        }
    }

    /// Return a reference to the cache provider (useful for testing / stats).
    pub fn cache_provider(&self) -> &dyn CacheProvider {
        &*self.cache_provider
    }

    /// Query bars for the given symbol and timeframe between `start` and `end`
    /// (inclusive, timestamps in seconds).
    ///
    /// Fetches raw 1-minute data from PostgreSQL and aggregates on-the-fly.
    /// Results are transparently cached.
    pub async fn get_bars(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        start: i64,
        end: i64,
    ) -> Result<Vec<StandardBar>, DataError> {
        self.get_bars_with_interval(symbol, timeframe.as_secs(), start, end)
            .await
    }

    /// Query bars for a specific interval in seconds.
    pub async fn get_bars_with_interval(
        &self,
        symbol: &str,
        interval_secs: u64,
        start: i64,
        end: i64,
    ) -> Result<Vec<StandardBar>, DataError> {
        debug!(
            symbol,
            interval_secs, start, end, "AggregationEngine::get_bars_with_interval called"
        );

        let key = CacheKey::new(symbol, interval_secs, start, end);

        // 1. Check cache
        if let Some(cached) = self.cache_provider.get(&key).await {
            debug!(symbol, interval_secs, start, end, "cache hit");
            return Ok(cached);
        }

        if let Some(ref pool) = self.pg_pool {
            let storage = PostgresStorage::from_pool(pool.clone());

            // 2. Query raw 1m data
            let raw = storage.query_bars(symbol, start, end).await?;
            if raw.is_empty() {
                return Err(DataError::NotFound(format!(
                    "no 1m bars for {} between {} and {}",
                    symbol, start, end
                )));
            }

            // 3. Aggregate
            let aggregated = if interval_secs == 60 {
                raw
            } else {
                Self::aggregate(&raw, interval_secs)?
            };

            // 4. Write to cache
            self.cache_provider
                .set(&key, &aggregated, Duration::from_secs(3600))
                .await;

            return Ok(aggregated);
        }

        Err(DataError::NotFound(format!(
            "no storage backend available for {}",
            symbol
        )))
    }

    /// Query bars using an external storage backend (useful for testing or
    /// non-PostgreSQL storage).
    pub async fn get_bars_with_storage(
        &self,
        storage: &dyn crate::fetcher::DataStorage,
        symbol: &str,
        interval_secs: u64,
        start: i64,
        end: i64,
    ) -> Result<Vec<StandardBar>, DataError> {
        let key = CacheKey::new(symbol, interval_secs, start, end);

        // 1. Check cache
        if let Some(cached) = self.cache_provider.get(&key).await {
            debug!(symbol, interval_secs, start, end, "cache hit");
            return Ok(cached);
        }

        // 2. Query raw 1m data from provided storage
        let raw = storage.query_bars(symbol, start, end).await?;
        if raw.is_empty() {
            return Err(DataError::NotFound(format!(
                "no 1m bars for {} between {} and {}",
                symbol, start, end
            )));
        }

        // 3. Aggregate
        let aggregated = if interval_secs == 60 {
            raw
        } else {
            Self::aggregate(&raw, interval_secs)?
        };

        // 4. Write to cache
        self.cache_provider
            .set(&key, &aggregated, Duration::from_secs(3600))
            .await;

        Ok(aggregated)
    }

    /// Convenience method that accepts a `TimeFrame` instead of raw seconds.
    pub async fn get_bars_with_timeframe(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        start: i64,
        end: i64,
    ) -> Result<Vec<StandardBar>, DataError> {
        self.get_bars_with_interval(symbol, timeframe.as_secs(), start, end)
            .await
    }

    /// Return the most recent closed bar for a symbol / timeframe.
    ///
    /// Queries the latest raw 1-minute bar, determines its timeframe bucket,
    /// fetches all raw bars in that bucket, and aggregates them.
    pub async fn get_latest_bar(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
    ) -> Result<Option<StandardBar>, DataError> {
        if let Some(ref pool) = self.pg_pool {
            let storage = PostgresStorage::from_pool(pool.clone());

            let latest_raw = storage.query_latest(symbol).await?;
            if latest_raw.is_none() {
                return Ok(None);
            }

            let latest = latest_raw.unwrap();
            let target_secs = timeframe.as_secs() as i64;
            let bucket_start = (latest.timestamp / target_secs) * target_secs;
            let bucket_end = bucket_start + target_secs - 1;

            let raw = storage.query_bars(symbol, bucket_start, bucket_end).await?;
            if raw.is_empty() {
                return Ok(None);
            }

            let aggregated = Self::aggregate(&raw, timeframe.as_secs())?;
            return Ok(aggregated.last().cloned());
        }
        Ok(None)
    }

    /// Warm up the cache with commonly-used time ranges.
    ///
    /// Pre-fetches the last day, week, and month for each requested interval.
    pub async fn warmup(
        &self,
        storage: &PostgresStorage,
        symbol: &str,
        intervals: &[u64],
    ) -> Result<(), DataError> {
        let now = chrono::Utc::now().timestamp();
        let day_ago = now - 86400;
        let week_ago = now - 86400 * 7;
        let month_ago = now - 86400 * 30;

        for &interval in intervals {
            let _ = self
                .get_bars_with_storage(storage, symbol, interval, day_ago, now)
                .await?;
            let _ = self
                .get_bars_with_storage(storage, symbol, interval, week_ago, now)
                .await?;
            let _ = self
                .get_bars_with_storage(storage, symbol, interval, month_ago, now)
                .await?;
        }

        Ok(())
    }

    /// Aggregate a slice of bars into the target interval (in seconds).
    ///
    /// Uses a bucketing algorithm: `bucket_ts = (bar.timestamp / interval_secs) * interval_secs`
    ///
    /// # Arguments
    /// * `bars` - Input bars (must be sorted by timestamp ascending)
    /// * `interval_secs` - Target interval in seconds (must be >= 60)
    pub fn aggregate(
        bars: &[StandardBar],
        interval_secs: u64,
    ) -> Result<Vec<StandardBar>, DataError> {
        if bars.is_empty() {
            return Ok(Vec::new());
        }

        if interval_secs < 60 {
            return Err(DataError::InvalidInterval(format!(
                "Interval must be >= 60 seconds, got {}",
                interval_secs
            )));
        }

        let mut buckets: BTreeMap<i64, Vec<&StandardBar>> = BTreeMap::new();

        // Bucket bars by their aligned timestamp
        for bar in bars {
            let bucket_ts = (bar.timestamp / interval_secs as i64) * interval_secs as i64;
            buckets.entry(bucket_ts).or_default().push(bar);
        }

        // Aggregate each bucket
        let mut aggregated = Vec::with_capacity(buckets.len());

        for (bucket_ts, bucket_bars) in buckets {
            if bucket_bars.is_empty() {
                continue;
            }

            let first = bucket_bars.first().unwrap();
            let last = bucket_bars.last().unwrap();

            let open = first.open;
            let close = last.close;
            let high = bucket_bars.iter().map(|b| b.high).max().unwrap();
            let low = bucket_bars.iter().map(|b| b.low).min().unwrap();
            let volume = bucket_bars.iter().map(|b| b.volume).sum();

            let symbol = first.symbol.clone();
            let exchange = first.exchange.clone();

            aggregated.push(StandardBar {
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
            output = aggregated.len(),
            interval_secs,
            "aggregation complete"
        );

        Ok(aggregated)
    }

    /// Aggregate a slice of 1-minute bars into the target timeframe.
    ///
    /// Delegates to [`aggregate`](Self::aggregate) internally.
    ///
    /// Bucketing formula: `bucket_ts = (bar.timestamp / target_secs) * target_secs`
    pub fn aggregate_from_1m(
        bars: &[StandardBar],
        target: TimeFrame,
    ) -> Result<Vec<StandardBar>, DataError> {
        Self::aggregate(bars, target.as_secs())
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

    // ========================================================================
    // Existing aggregation tests (must continue to pass)
    // ========================================================================

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
            make_bar(
                0,
                "100.12345678",
                "101.00000001",
                "99.99999999",
                "100.50000000",
                "0.11111111",
            ),
            make_bar(
                60,
                "100.50000000",
                "102.00000000",
                "100.00000000",
                "101.00000000",
                "0.22222222",
            ),
            make_bar(
                120,
                "101.00000000",
                "103.00000000",
                "100.50000000",
                "102.00000000",
                "0.33333333",
            ),
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

    // ========================================================================
    // Tests for the `aggregate` static method
    // ========================================================================

    #[test]
    fn test_aggregate_static_m1_to_m5() {
        let bars = vec![
            make_bar(0, "100", "110", "90", "105", "1000"),
            make_bar(60, "105", "115", "95", "110", "2000"),
            make_bar(120, "110", "120", "100", "115", "3000"),
            make_bar(180, "115", "125", "105", "120", "4000"),
            make_bar(240, "120", "130", "110", "125", "5000"),
        ];

        let aggregated = AggregationEngine::aggregate(&bars, 300).unwrap();

        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].timestamp, 0);
        assert_eq!(aggregated[0].open, Decimal::from(100));
        assert_eq!(aggregated[0].high, Decimal::from(130));
        assert_eq!(aggregated[0].low, Decimal::from(90));
        assert_eq!(aggregated[0].close, Decimal::from(125));
        assert_eq!(aggregated[0].volume, Decimal::from(15000));
    }

    #[test]
    fn test_aggregate_static_custom_interval() {
        let bars = vec![
            make_bar(0, "100", "110", "90", "105", "1000"),
            make_bar(60, "105", "115", "95", "110", "2000"),
            make_bar(120, "110", "120", "100", "115", "3000"),
        ];

        // 180s (3m)
        let aggregated = AggregationEngine::aggregate(&bars, 180).unwrap();
        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].timestamp, 0);
        assert_eq!(aggregated[0].open, Decimal::from(100));
        assert_eq!(aggregated[0].high, Decimal::from(120));
        assert_eq!(aggregated[0].low, Decimal::from(90));
        assert_eq!(aggregated[0].close, Decimal::from(115));
        assert_eq!(aggregated[0].volume, Decimal::from(6000));

        // 90s (custom)
        let aggregated = AggregationEngine::aggregate(&bars, 90).unwrap();
        assert_eq!(aggregated.len(), 2);
        assert_eq!(aggregated[0].timestamp, 0);
        assert_eq!(aggregated[0].volume, Decimal::from(3000));
        assert_eq!(aggregated[1].timestamp, 90);
        assert_eq!(aggregated[1].volume, Decimal::from(3000));
    }

    #[test]
    fn test_aggregate_static_empty() {
        let bars: Vec<StandardBar> = vec![];
        let aggregated = AggregationEngine::aggregate(&bars, 300).unwrap();
        assert!(aggregated.is_empty());
    }

    #[test]
    fn test_aggregate_static_invalid_interval() {
        let bars = vec![make_bar(0, "100", "110", "90", "105", "1000")];
        let result = AggregationEngine::aggregate(&bars, 30);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Interval must be >= 60 seconds"),
            "Error should mention minimum interval: {}",
            err
        );
    }

    #[test]
    fn test_aggregate_static_single_bar() {
        let bars = vec![make_bar(0, "100", "110", "90", "105", "1000")];
        let aggregated = AggregationEngine::aggregate(&bars, 300).unwrap();
        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].timestamp, 0);
        assert_eq!(aggregated[0].open, Decimal::from(100));
        assert_eq!(aggregated[0].high, Decimal::from(110));
        assert_eq!(aggregated[0].low, Decimal::from(90));
        assert_eq!(aggregated[0].close, Decimal::from(105));
        assert_eq!(aggregated[0].volume, Decimal::from(1000));
    }

    #[test]
    fn test_aggregate_static_gaps_in_data() {
        // Bars at 0, 60, 180 (gap at 120)
        let bars = vec![
            make_bar(0, "100", "110", "90", "105", "1000"),
            make_bar(60, "105", "115", "95", "110", "2000"),
            make_bar(180, "110", "120", "100", "115", "3000"),
        ];

        let aggregated = AggregationEngine::aggregate(&bars, 300).unwrap();
        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].timestamp, 0);
        assert_eq!(aggregated[0].open, Decimal::from(100));
        assert_eq!(aggregated[0].high, Decimal::from(120));
        assert_eq!(aggregated[0].low, Decimal::from(90));
        assert_eq!(aggregated[0].close, Decimal::from(115));
        assert_eq!(aggregated[0].volume, Decimal::from(6000));
    }

    #[test]
    fn test_aggregate_static_multiple_buckets_with_gaps() {
        // Two complete buckets and a partial third
        let bars = vec![
            make_bar(0, "100", "110", "90", "105", "1000"),
            make_bar(60, "105", "115", "95", "110", "2000"),
            make_bar(300, "110", "120", "100", "115", "3000"),
            make_bar(360, "115", "125", "105", "120", "4000"),
            make_bar(600, "120", "130", "110", "125", "5000"),
        ];

        let aggregated = AggregationEngine::aggregate(&bars, 300).unwrap();
        assert_eq!(aggregated.len(), 3);
        assert_eq!(aggregated[0].timestamp, 0);
        assert_eq!(aggregated[0].volume, Decimal::from(3000));
        assert_eq!(aggregated[1].timestamp, 300);
        assert_eq!(aggregated[1].volume, Decimal::from(7000));
        assert_eq!(aggregated[2].timestamp, 600);
        assert_eq!(aggregated[2].volume, Decimal::from(5000));
    }

    #[test]
    fn test_aggregate_static_m1_to_m3() {
        let bars = vec![
            make_bar(0, "100", "110", "90", "105", "1000"),
            make_bar(60, "105", "115", "95", "110", "2000"),
            make_bar(120, "110", "120", "100", "115", "3000"),
        ];

        let aggregated = AggregationEngine::aggregate(&bars, 180).unwrap();
        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].timestamp, 0);
        assert_eq!(aggregated[0].open, Decimal::from(100));
        assert_eq!(aggregated[0].high, Decimal::from(120));
        assert_eq!(aggregated[0].low, Decimal::from(90));
        assert_eq!(aggregated[0].close, Decimal::from(115));
        assert_eq!(aggregated[0].volume, Decimal::from(6000));
    }

    #[test]
    fn test_aggregate_static_m1_to_h1() {
        let bars: Vec<StandardBar> = (0..60)
            .map(|i| make_bar(i * 60, "50000", "51000", "49000", "50500", "150"))
            .collect();

        let aggregated = AggregationEngine::aggregate(&bars, 3600).unwrap();
        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].timestamp, 0);
        assert_eq!(aggregated[0].open, Decimal::from(50000));
        assert_eq!(aggregated[0].high, Decimal::from(51000));
        assert_eq!(aggregated[0].low, Decimal::from(49000));
        assert_eq!(aggregated[0].close, Decimal::from(50500));
        assert_eq!(aggregated[0].volume, Decimal::from(9000));
    }

    #[test]
    fn test_aggregate_static_m5_to_h1() {
        // Simulate 5m bars aggregated to 1h
        let bars = vec![
            make_bar(0, "100", "110", "90", "105", "1000"),
            make_bar(300, "105", "115", "95", "110", "2000"),
            make_bar(600, "110", "120", "100", "115", "3000"),
            make_bar(900, "115", "125", "105", "120", "4000"),
            make_bar(1200, "120", "130", "110", "125", "5000"),
            make_bar(1500, "125", "135", "115", "130", "6000"),
            make_bar(1800, "130", "140", "120", "135", "7000"),
            make_bar(2100, "135", "145", "125", "140", "8000"),
            make_bar(2400, "140", "150", "130", "145", "9000"),
            make_bar(2700, "145", "155", "135", "150", "10000"),
            make_bar(3000, "150", "160", "140", "155", "11000"),
            make_bar(3300, "155", "165", "145", "160", "12000"),
        ];

        let aggregated = AggregationEngine::aggregate(&bars, 3600).unwrap();
        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].timestamp, 0);
        assert_eq!(aggregated[0].open, Decimal::from(100));
        assert_eq!(aggregated[0].high, Decimal::from(165));
        assert_eq!(aggregated[0].low, Decimal::from(90));
        assert_eq!(aggregated[0].close, Decimal::from(160));
        assert_eq!(aggregated[0].volume, Decimal::from(78000));
    }

    #[test]
    fn test_aggregate_static_incomplete_period() {
        // Only 3 bars in a 5-minute bucket
        let bars = vec![
            make_bar(0, "100", "110", "90", "105", "1000"),
            make_bar(60, "105", "115", "95", "110", "2000"),
            make_bar(120, "110", "120", "100", "115", "3000"),
        ];

        let aggregated = AggregationEngine::aggregate(&bars, 300).unwrap();
        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].timestamp, 0);
        assert_eq!(aggregated[0].open, Decimal::from(100));
        assert_eq!(aggregated[0].high, Decimal::from(120));
        assert_eq!(aggregated[0].low, Decimal::from(90));
        assert_eq!(aggregated[0].close, Decimal::from(115));
        assert_eq!(aggregated[0].volume, Decimal::from(6000));
    }

    #[test]
    fn test_aggregate_static_preserves_decimal_precision() {
        let bars = vec![
            make_bar(
                0,
                "100.12345678",
                "101.00000001",
                "99.99999999",
                "100.50000000",
                "0.11111111",
            ),
            make_bar(
                60,
                "100.50000000",
                "102.00000000",
                "100.00000000",
                "101.00000000",
                "0.22222222",
            ),
            make_bar(
                120,
                "101.00000000",
                "103.00000000",
                "100.50000000",
                "102.00000000",
                "0.33333333",
            ),
        ];

        let aggregated = AggregationEngine::aggregate(&bars, 300).unwrap();
        assert_eq!(aggregated.len(), 1);
        assert_eq!(
            aggregated[0].volume,
            Decimal::from_str_exact("0.66666666").unwrap()
        );
        assert_eq!(
            aggregated[0].open,
            Decimal::from_str_exact("100.12345678").unwrap()
        );
        assert_eq!(
            aggregated[0].high,
            Decimal::from_str_exact("103.00000000").unwrap()
        );
        assert_eq!(
            aggregated[0].low,
            Decimal::from_str_exact("99.99999999").unwrap()
        );
        assert_eq!(
            aggregated[0].close,
            Decimal::from_str_exact("102.00000000").unwrap()
        );
    }

    #[test]
    fn test_aggregate_static_different_symbols_in_same_bucket() {
        // This tests that the algorithm correctly uses the first bar's symbol/exchange
        let bars = vec![
            StandardBar {
                timestamp: 0,
                open: Decimal::from(100),
                high: Decimal::from(110),
                low: Decimal::from(90),
                close: Decimal::from(105),
                volume: Decimal::from(1000),
                symbol: "BTC-USDT".to_string(),
                exchange: "binance".to_string(),
                confirmed: true,
            },
            StandardBar {
                timestamp: 60,
                open: Decimal::from(105),
                high: Decimal::from(115),
                low: Decimal::from(95),
                close: Decimal::from(110),
                volume: Decimal::from(2000),
                symbol: "ETH-USDT".to_string(),
                exchange: "okx".to_string(),
                confirmed: true,
            },
        ];

        let aggregated = AggregationEngine::aggregate(&bars, 300).unwrap();
        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].symbol, "BTC-USDT");
        assert_eq!(aggregated[0].exchange, "binance");
    }

    #[test]
    fn test_aggregate_from_1m_delegates_to_aggregate() {
        let bars = vec![
            make_bar(0, "100", "110", "90", "105", "1000"),
            make_bar(60, "105", "115", "95", "110", "2000"),
            make_bar(120, "110", "120", "100", "115", "3000"),
            make_bar(180, "115", "125", "105", "120", "4000"),
            make_bar(240, "120", "130", "110", "125", "5000"),
        ];

        let agg_old = AggregationEngine::aggregate_from_1m(&bars, TimeFrame::M5).unwrap();
        let agg_new = AggregationEngine::aggregate(&bars, 300).unwrap();

        assert_eq!(agg_old.len(), agg_new.len());
        for (old, new) in agg_old.iter().zip(agg_new.iter()) {
            assert_eq!(old.timestamp, new.timestamp);
            assert_eq!(old.open, new.open);
            assert_eq!(old.high, new.high);
            assert_eq!(old.low, new.low);
            assert_eq!(old.close, new.close);
            assert_eq!(old.volume, new.volume);
        }
    }

    // ========================================================================
    // New caching integration tests
    // ========================================================================

    /// A mock storage that returns predefined bars (no DB needed).
    #[derive(Debug)]
    struct MockStorage {
        bars: Vec<StandardBar>,
        query_count: std::sync::atomic::AtomicUsize,
    }

    #[async_trait::async_trait]
    impl crate::fetcher::DataStorage for MockStorage {
        async fn query_data_ranges(&self, _symbol: &str) -> Result<Vec<(i64, i64)>, DataError> {
            Ok(vec![])
        }

        async fn insert_bars(&self, _bars: &[StandardBar]) -> Result<u64, DataError> {
            Ok(0)
        }

        async fn query_bars(
            &self,
            _symbol: &str,
            _start: i64,
            _end: i64,
        ) -> Result<Vec<StandardBar>, DataError> {
            self.query_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(self.bars.clone())
        }
    }

    #[tokio::test]
    async fn test_engine_cache_hit_avoids_storage_query() {
        let bars = vec![
            make_bar(0, "100", "110", "90", "105", "1000"),
            make_bar(60, "105", "115", "95", "110", "2000"),
            make_bar(120, "110", "120", "100", "115", "3000"),
        ];

        let storage = MockStorage {
            bars: bars.clone(),
            query_count: std::sync::atomic::AtomicUsize::new(0),
        };

        let engine = AggregationEngine::new(None);

        // First query — should hit storage
        let result1 = engine
            .get_bars_with_storage(&storage, "BTC-USDT", 180, 0, 120)
            .await
            .unwrap();
        assert_eq!(result1.len(), 1);
        assert_eq!(
            storage
                .query_count
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );

        // Second query — should hit cache, not storage
        let result2 = engine
            .get_bars_with_storage(&storage, "BTC-USDT", 180, 0, 120)
            .await
            .unwrap();
        assert_eq!(result2, result1);
        assert_eq!(
            storage
                .query_count
                .load(std::sync::atomic::Ordering::SeqCst),
            1 // still 1!
        );

        // Cache stats should show a hit
        assert_eq!(engine.cache_provider().stats().hits(), 1);
        assert_eq!(engine.cache_provider().stats().misses(), 1);
    }

    #[tokio::test]
    async fn test_engine_cache_miss_queries_storage() {
        let bars = vec![
            make_bar(0, "100", "110", "90", "105", "1000"),
            make_bar(60, "105", "115", "95", "110", "2000"),
        ];

        let storage = MockStorage {
            bars: bars.clone(),
            query_count: std::sync::atomic::AtomicUsize::new(0),
        };

        let engine = AggregationEngine::new(None);

        let result = engine
            .get_bars_with_storage(&storage, "BTC-USDT", 300, 0, 60)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            storage
                .query_count
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert_eq!(engine.cache_provider().stats().misses(), 1);
    }

    #[tokio::test]
    async fn test_engine_different_keys_cached_independently() {
        let bars = vec![
            make_bar(0, "100", "110", "90", "105", "1000"),
            make_bar(60, "105", "115", "95", "110", "2000"),
        ];

        let storage = MockStorage {
            bars: bars.clone(),
            query_count: std::sync::atomic::AtomicUsize::new(0),
        };

        let engine = AggregationEngine::new(None);

        // Query two different keys
        let _ = engine
            .get_bars_with_storage(&storage, "BTC-USDT", 300, 0, 60)
            .await
            .unwrap();
        let _ = engine
            .get_bars_with_storage(&storage, "ETH-USDT", 300, 0, 60)
            .await
            .unwrap();

        // Both should have queried storage (cache misses)
        assert_eq!(
            storage
                .query_count
                .load(std::sync::atomic::Ordering::SeqCst),
            2
        );
        assert_eq!(engine.cache_provider().stats().misses(), 2);

        // Re-query both — should hit cache
        let _ = engine
            .get_bars_with_storage(&storage, "BTC-USDT", 300, 0, 60)
            .await
            .unwrap();
        let _ = engine
            .get_bars_with_storage(&storage, "ETH-USDT", 300, 0, 60)
            .await
            .unwrap();

        // No additional storage queries
        assert_eq!(
            storage
                .query_count
                .load(std::sync::atomic::Ordering::SeqCst),
            2
        );
        assert_eq!(engine.cache_provider().stats().hits(), 2);
    }

    #[tokio::test]
    async fn test_engine_with_custom_cache_provider() {
        let cache = MemoryCacheProvider::new(10);
        let engine = AggregationEngine::with_cache(Box::new(cache));

        let bars = vec![
            make_bar(0, "100", "110", "90", "105", "1000"),
            make_bar(60, "105", "115", "95", "110", "2000"),
        ];

        let storage = MockStorage {
            bars: bars.clone(),
            query_count: std::sync::atomic::AtomicUsize::new(0),
        };

        let _ = engine
            .get_bars_with_storage(&storage, "BTC-USDT", 300, 0, 60)
            .await
            .unwrap();

        // Cache should have recorded a miss
        assert_eq!(engine.cache_provider().stats().misses(), 1);
    }

    #[tokio::test]
    async fn test_engine_backward_compat_with_cache_size() {
        // Ensure old `with_cache_size` still works
        let engine = AggregationEngine::with_cache_size(None, 500);

        let bars = vec![
            make_bar(0, "100", "110", "90", "105", "1000"),
            make_bar(60, "105", "115", "95", "110", "2000"),
        ];

        let storage = MockStorage {
            bars: bars.clone(),
            query_count: std::sync::atomic::AtomicUsize::new(0),
        };

        let result = engine
            .get_bars_with_storage(&storage, "BTC-USDT", 300, 0, 60)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_engine_cache_is_used_with_1m_interval() {
        // When interval == 60, bars are returned raw but still cached
        let bars = vec![
            make_bar(0, "100", "110", "90", "105", "1000"),
            make_bar(60, "105", "115", "95", "110", "2000"),
        ];

        let storage = MockStorage {
            bars: bars.clone(),
            query_count: std::sync::atomic::AtomicUsize::new(0),
        };

        let engine = AggregationEngine::new(None);

        let result1 = engine
            .get_bars_with_storage(&storage, "BTC-USDT", 60, 0, 60)
            .await
            .unwrap();
        assert_eq!(result1.len(), 2); // raw bars, no aggregation

        let result2 = engine
            .get_bars_with_storage(&storage, "BTC-USDT", 60, 0, 60)
            .await
            .unwrap();
        assert_eq!(result2, result1);

        // Only one storage query
        assert_eq!(
            storage
                .query_count
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[tokio::test]
    async fn test_engine_clear_cache() {
        let bars = vec![
            make_bar(0, "100", "110", "90", "105", "1000"),
            make_bar(60, "105", "115", "95", "110", "2000"),
        ];

        let storage = MockStorage {
            bars: bars.clone(),
            query_count: std::sync::atomic::AtomicUsize::new(0),
        };

        let engine = AggregationEngine::new(None);

        // Query and cache
        let _ = engine
            .get_bars_with_storage(&storage, "BTC-USDT", 300, 0, 60)
            .await
            .unwrap();

        // Clear cache
        engine.cache_provider().clear().await;

        // Re-query should hit storage again
        let _ = engine
            .get_bars_with_storage(&storage, "BTC-USDT", 300, 0, 60)
            .await
            .unwrap();

        assert_eq!(
            storage
                .query_count
                .load(std::sync::atomic::Ordering::SeqCst),
            2
        );
    }
}
