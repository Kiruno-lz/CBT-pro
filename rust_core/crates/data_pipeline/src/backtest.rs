//! Backtest data integration module.
//!
//! Provides a high-level API for the backtesting engine to request historical
//! data from the data pipeline, ensuring data is fetched on-demand and
//! aggregated to the requested timeframe.

use crate::{
    aggregation::AggregationEngine,
    error::DataError,
    exchange::{ExchangeAdapter, ExchangeAdapterFactory},
    fetcher::{DataFetcher, DataStorage},
    StandardBar, TimeFrame,
};
use std::sync::Arc;
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// DataSource
// ---------------------------------------------------------------------------

/// Source of data for backtesting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataSource {
    /// Real market data fetched from exchanges (default).
    #[default]
    RealTime,
    /// Synthetic data generated for testing.
    Synthetic,
}

// ---------------------------------------------------------------------------
// BacktestConfig
// ---------------------------------------------------------------------------

/// Configuration for a backtest run.
#[derive(Debug, Clone)]
pub struct BacktestConfig {
    /// Trading pair symbol in standard format (e.g. `"BTC/USDT"`).
    pub symbol: String,
    /// Timeframe string (e.g. `"1h"`, `"5m"`).
    pub timeframe: String,
    /// Number of bars to request (used for synthetic fallback and validation).
    pub count: usize,
    /// Optional explicit start time (unix seconds).
    pub start_time: Option<i64>,
    /// Optional explicit end time (unix seconds).  Defaults to now.
    pub end_time: Option<i64>,
    /// Data source preference.
    pub data_source: DataSource,
    /// Maximum number of bars allowed (safety limit).
    pub max_count: usize,
}

impl BacktestConfig {
    /// Create a new config with sensible defaults.
    pub fn new(symbol: &str, timeframe: &str, count: usize) -> Self {
        Self {
            symbol: symbol.to_string(),
            timeframe: timeframe.to_string(),
            count,
            start_time: None,
            end_time: None,
            data_source: DataSource::RealTime,
            max_count: 10_000,
        }
    }

    /// Set an explicit start time.
    pub fn with_start_time(mut self, start: i64) -> Self {
        self.start_time = Some(start);
        self
    }

    /// Set an explicit end time.
    pub fn with_end_time(mut self, end: i64) -> Self {
        self.end_time = Some(end);
        self
    }

    /// Set the data source.
    pub fn with_data_source(mut self, source: DataSource) -> Self {
        self.data_source = source;
        self
    }

    /// Set the maximum count limit.
    pub fn with_max_count(mut self, max: usize) -> Self {
        self.max_count = max;
        self
    }
}

// ---------------------------------------------------------------------------
// Time range calculation
// ---------------------------------------------------------------------------

/// Calculate the `(start, end)` time range for a backtest config.
///
/// Uses explicit `start_time` and `end_time` from config when provided.
/// For synthetic data fallback, `count` determines the range if times are not set.
///
/// # Errors
/// Returns `DataError::InvalidTimeFrame` if the timeframe string is invalid.
/// Returns `DataError::InvalidInterval` if the calculated range is invalid.
pub fn calculate_time_range(config: &BacktestConfig) -> Result<(i64, i64), DataError> {
    let end_time = config
        .end_time
        .unwrap_or_else(|| chrono::Utc::now().timestamp());

    if let Some(start_time) = config.start_time {
        // Use explicit start_time and end_time
        if start_time >= end_time {
            return Err(DataError::InvalidInterval(format!(
                "start_time ({}) >= end_time ({})",
                start_time, end_time
            )));
        }

        // If both start_time and end_time are explicitly set and count > 0,
        // limit the range to count bars from the end
        if config.count > 0 && config.end_time.is_some() {
            let timeframe = TimeFrame::parse(&config.timeframe)?;
            let interval_secs = timeframe.as_secs() as i64;
            let calculated_start = end_time - (config.count as i64 * interval_secs);
            let actual_start = start_time.max(calculated_start);
            return Ok((actual_start, end_time));
        }

        return Ok((start_time, end_time));
    }

    // Fallback: use count to calculate range (for synthetic data)
    let timeframe = TimeFrame::parse(&config.timeframe)?;
    let interval_secs = timeframe.as_secs() as i64;

    if config.count == 0 {
        return Ok((end_time, end_time));
    }

    let start_time = end_time - (config.count as i64 * interval_secs);

    if start_time >= end_time {
        return Err(DataError::InvalidInterval(format!(
            "start_time ({}) >= end_time ({})",
            start_time, end_time
        )));
    }

    if config.count > config.max_count {
        return Err(DataError::InvalidInterval(format!(
            "count ({}) exceeds max_count ({})",
            config.count, config.max_count
        )));
    }

    Ok((start_time, end_time))
}

// ---------------------------------------------------------------------------
// BacktestDataProvider
// ---------------------------------------------------------------------------

/// High-level provider that orchestrates data fetching and aggregation for
/// backtesting.
///
/// Bridges the gap between the low-level data pipeline (fetcher, storage,
/// aggregation) and the backtesting engine.
pub struct BacktestDataProvider {
    fetcher: DataFetcher,
    agg_engine: AggregationEngine,
    storage: Option<Arc<dyn DataStorage>>,
}

impl std::fmt::Debug for BacktestDataProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BacktestDataProvider")
            .field("fetcher", &self.fetcher)
            .field("agg_engine", &self.agg_engine)
            .field("has_storage", &self.storage.is_some())
            .finish()
    }
}

impl BacktestDataProvider {
    /// Create a new provider from existing components.
    pub fn new(fetcher: DataFetcher, agg_engine: AggregationEngine) -> Self {
        Self {
            fetcher,
            agg_engine,
            storage: None,
        }
    }

    /// Build a provider with a storage backend and exchange adapter.
    pub fn with_storage_and_adapter(
        storage: Arc<dyn DataStorage>,
        adapter: Box<dyn ExchangeAdapter>,
        cache_size: usize,
    ) -> Self {
        let fetcher = DataFetcher::new(storage.clone(), adapter);
        let agg_engine = AggregationEngine::with_cache_size(None, cache_size);
        Self {
            fetcher,
            agg_engine,
            storage: Some(storage),
        }
    }

    /// Convenience factory that creates everything from a database URL and
    /// exchange name.
    pub async fn from_config(
        database_url: &str,
        exchange: &str,
        cache_size: usize,
    ) -> Result<Self, DataError> {
        let storage = Arc::new(crate::storage::PostgresStorage::connect(database_url).await?);
        let adapter = ExchangeAdapterFactory::create(exchange)?;
        Ok(Self::with_storage_and_adapter(storage, adapter, cache_size))
    }

    /// Fetch bars for a backtest configuration.
    ///
    /// 1. Calculates the time range.
    /// 2. Ensures data is present (fetches missing ranges).
    /// 3. Aggregates to the requested timeframe.
    /// 4. Validates that enough bars are available.
    pub async fn get_bars(&self, config: &BacktestConfig) -> Result<Vec<StandardBar>, DataError> {
        if config.count == 0 {
            return Ok(Vec::new());
        }

        if config.data_source == DataSource::Synthetic {
            return self.generate_synthetic_bars(config);
        }

        let (start_time, end_time) = calculate_time_range(config)?;
        info!(
            symbol = %config.symbol,
            timeframe = %config.timeframe,
            count = config.count,
            start = start_time,
            end = end_time,
            "fetching backtest data"
        );

        // 1. Ensure raw 1-minute data is available
        // The fetcher has its own limit (10_000 missing bars) to prevent
        // excessive automatic downloading.  If the database already contains
        // the requested range, ensure_data returns instantly.
        self.fetcher
            .ensure_data(&config.symbol, start_time, end_time)
            .await?;

        // 2. Aggregate to requested timeframe
        let timeframe = TimeFrame::parse(&config.timeframe)?;
        let bars = if let Some(ref storage) = self.storage {
            self.agg_engine
                .get_bars_with_storage(
                    storage.as_ref(),
                    &config.symbol,
                    timeframe.as_secs(),
                    start_time,
                    end_time,
                )
                .await?
        } else {
            self.agg_engine
                .get_bars(&config.symbol, timeframe, start_time, end_time)
                .await?
        };

        // 3. Validate sufficient data
        if bars.len() < config.count {
            return Err(DataError::NotFound(format!(
                "insufficient data: requested {} bars, only {} available",
                config.count,
                bars.len()
            )));
        }

        info!(
            symbol = %config.symbol,
            count = bars.len(),
            "backtest data ready"
        );

        Ok(bars)
    }

    /// Verify data integrity for a backtest configuration.
    pub async fn verify_data(
        &self,
        config: &BacktestConfig,
    ) -> Result<crate::fetcher::DataIntegrity, DataError> {
        let (start_time, end_time) = calculate_time_range(config)?;
        self.fetcher
            .verify_data(&config.symbol, start_time, end_time)
            .await
    }

    /// Generate synthetic bars for testing (fallback mode).
    fn generate_synthetic_bars(
        &self,
        config: &BacktestConfig,
    ) -> Result<Vec<StandardBar>, DataError> {
        use rand::rngs::SmallRng;
        use rand::{Rng, SeedableRng};
        use rust_decimal::Decimal;

        let timeframe = TimeFrame::parse(&config.timeframe)?;
        let interval_secs = timeframe.as_secs() as i64;
        let (start_time, end_time) = calculate_time_range(config)?;

        let count = ((end_time - start_time) / interval_secs).max(0) as usize;
        if count == 0 {
            return Ok(Vec::new());
        }

        let mut rng = SmallRng::seed_from_u64(42);
        let mut price = Decimal::from(42000);
        let mut bars = Vec::with_capacity(count);

        for i in 0..count {
            let ts = start_time + (i as i64) * interval_secs;
            let open = price;
            let delta = Decimal::from(rng.gen_range(-50i64..=50i64));
            let close = open + delta;
            let high_offset = Decimal::from(rng.gen_range(5i64..=25i64));
            let low_offset = Decimal::from(rng.gen_range(5i64..=25i64));
            let high = open.max(close) + high_offset;
            let low = open.min(close) - low_offset;
            let volume = Decimal::from(rng.gen_range(50i64..=500i64));

            bars.push(StandardBar {
                timestamp: ts,
                open,
                high,
                low,
                close,
                volume,
                symbol: config.symbol.clone(),
                exchange: "synthetic".to_string(),
                confirmed: true,
            });
            price = close;
        }

        warn!(count = bars.len(), "using synthetic data for backtest");
        Ok(bars)
    }
}

/// Warm up common trading pairs in the background.
pub async fn warmup_data(fetcher: &DataFetcher, symbols: &[&str], duration_secs: i64) {
    let now = chrono::Utc::now().timestamp();
    let start = now - duration_secs;

    for symbol in symbols {
        if let Err(e) = fetcher.ensure_data(symbol, start, now).await {
            warn!(symbol, error = %e, "warmup failed");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange::{ExchangeAdapter, SymbolNormalizer};
    use crate::fetcher::DataStorage;
    use rust_decimal::Decimal;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // ========================================================================
    // Mock storage
    // ========================================================================

    #[derive(Debug, Clone)]
    struct MockStorage {
        bars: Arc<Mutex<Vec<StandardBar>>>,
        data_ranges: Arc<Mutex<Vec<(i64, i64)>>>,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                bars: Arc::new(Mutex::new(Vec::new())),
                data_ranges: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_bars(bars: Vec<StandardBar>) -> Self {
            let s = Self::new();
            {
                let mut b = s.bars.try_lock().unwrap();
                *b = bars.clone();

                let mut ranges = Vec::new();
                if !bars.is_empty() {
                    let mut sorted = bars.clone();
                    sorted.sort_by_key(|b| b.timestamp);
                    let mut range_start = sorted[0].timestamp;
                    let mut range_end = sorted[0].timestamp + 60;

                    for bar in sorted.iter().skip(1) {
                        if bar.timestamp == range_end {
                            range_end = bar.timestamp + 60;
                        } else {
                            ranges.push((range_start, range_end));
                            range_start = bar.timestamp;
                            range_end = bar.timestamp + 60;
                        }
                    }
                    ranges.push((range_start, range_end));
                }

                let mut dr = s.data_ranges.try_lock().unwrap();
                *dr = ranges;
            }
            s
        }
    }

    #[async_trait::async_trait]
    impl DataStorage for MockStorage {
        async fn query_data_ranges(&self, _symbol: &str) -> Result<Vec<(i64, i64)>, DataError> {
            let ranges = self.data_ranges.lock().await.clone();
            Ok(ranges)
        }

        async fn insert_bars(&self, bars: &[StandardBar]) -> Result<u64, DataError> {
            let mut stored = self.bars.lock().await;
            let mut inserted: u64 = 0;
            for bar in bars {
                if !stored
                    .iter()
                    .any(|b| b.symbol == bar.symbol && b.timestamp == bar.timestamp)
                {
                    stored.push(bar.clone());
                    inserted += 1;
                }
            }
            Ok(inserted)
        }

        async fn query_bars(
            &self,
            _symbol: &str,
            start: i64,
            end: i64,
        ) -> Result<Vec<StandardBar>, DataError> {
            let stored = self.bars.lock().await;
            let filtered: Vec<StandardBar> = stored
                .iter()
                .filter(|b| b.timestamp >= start && b.timestamp < end)
                .cloned()
                .collect();
            Ok(filtered)
        }
    }

    // ========================================================================
    // Mock adapter
    // ========================================================================

    #[derive(Debug, Clone)]
    struct MockAdapter {
        bars_to_return: Arc<Mutex<Vec<StandardBar>>>,
        should_fail: Arc<Mutex<bool>>,
    }

    impl MockAdapter {
        fn with_bars(bars: Vec<StandardBar>) -> Self {
            Self {
                bars_to_return: Arc::new(Mutex::new(bars)),
                should_fail: Arc::new(Mutex::new(false)),
            }
        }

        fn set_fail(&self, fail: bool) {
            let mut f = self.should_fail.try_lock().unwrap();
            *f = fail;
        }
    }

    #[async_trait::async_trait]
    impl ExchangeAdapter for MockAdapter {
        fn name(&self) -> &str {
            "mock"
        }

        async fn fetch_ohlcv(
            &self,
            _symbol: &str,
            _interval_secs: u64,
            _start_time: i64,
            _end_time: i64,
        ) -> Result<Vec<StandardBar>, DataError> {
            let fail = *self.should_fail.lock().await;
            if fail {
                return Err(DataError::Exchange("mock fetch failure".to_string()));
            }
            let bars = self.bars_to_return.lock().await.clone();
            Ok(bars)
        }

        async fn fetch_symbols(&self) -> Result<Vec<String>, DataError> {
            Ok(vec![])
        }

        fn min_interval_secs(&self) -> u64 {
            60
        }

        fn max_limit_per_request(&self) -> usize {
            1000
        }

        fn normalize_symbol(&self, symbol: &str) -> String {
            SymbolNormalizer::normalize(symbol, "mock").unwrap_or_else(|_| symbol.to_string())
        }
    }

    // ========================================================================
    // Helper
    // ========================================================================

    fn make_bar(ts: i64, o: &str, h: &str, l: &str, c: &str, v: &str) -> StandardBar {
        StandardBar {
            timestamp: ts,
            open: o.parse().unwrap(),
            high: h.parse().unwrap(),
            low: l.parse().unwrap(),
            close: c.parse().unwrap(),
            volume: v.parse().unwrap(),
            symbol: "BTC/USDT".to_string(),
            exchange: "mock".to_string(),
            confirmed: true,
        }
    }

    fn make_config(count: usize) -> BacktestConfig {
        BacktestConfig::new("BTC/USDT", "1h", count)
    }

    // ========================================================================
    // calculate_time_range tests
    // ========================================================================

    #[test]
    fn calculate_time_range_basic() {
        // Test with explicit start_time and end_time
        let config = BacktestConfig::new("BTC/USDT", "1h", 10)
            .with_start_time(0)
            .with_end_time(36000);
        let (start, end) = calculate_time_range(&config).unwrap();
        assert_eq!(end, 36000);
        assert_eq!(start, 0);
    }

    #[test]
    fn calculate_time_range_uses_explicit_start_time() {
        // Test that explicit start_time is used directly, not reverse-calculated
        let config = BacktestConfig::new("BTC/USDT", "1h", 10)
            .with_start_time(1000)
            .with_end_time(36000);
        let (start, end) = calculate_time_range(&config).unwrap();
        assert_eq!(start, 1000);
        assert_eq!(end, 36000);
    }

    #[test]
    fn calculate_time_range_limits_with_count() {
        // When count is smaller than the full range, the returned start should be
        // limited to end - count * interval
        let config = BacktestConfig::new("BTC/USDT", "1h", 5)
            .with_start_time(0)
            .with_end_time(36000); // 10 hours
        let (start, end) = calculate_time_range(&config).unwrap();
        assert_eq!(end, 36000);
        // Should be limited to 5 hours from the end: 36000 - 5*3600 = 18000
        assert_eq!(start, 18000);
    }

    #[test]
    fn calculate_time_range_limits_with_count_respects_min_start() {
        // When count would push start before the explicit start_time, use start_time
        let config = BacktestConfig::new("BTC/USDT", "1h", 20)
            .with_start_time(0)
            .with_end_time(36000); // 10 hours, but count=20 > 10
        let (start, end) = calculate_time_range(&config).unwrap();
        assert_eq!(end, 36000);
        // Count=20 would calculate start = 36000 - 20*3600 = -36000
        // But we should respect the explicit start_time of 0
        assert_eq!(start, 0);
    }

    #[test]
    fn calculate_time_range_uses_now_when_no_end_time() {
        let config = BacktestConfig::new("BTC/USDT", "1h", 1);
        let (start, end) = calculate_time_range(&config).unwrap();
        let now = chrono::Utc::now().timestamp();
        assert!(end <= now);
        assert!(end > now - 5); // within 5 seconds
        assert_eq!(start, end - 3600);
    }

    #[test]
    fn calculate_time_range_invalid_timeframe() {
        let config = BacktestConfig::new("BTC/USDT", "invalid", 10);
        let result = calculate_time_range(&config);
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), DataError::InvalidTimeFrame(ref s) if s == "invalid")
        );
    }

    #[test]
    fn calculate_time_range_exceeds_max_count() {
        let config = BacktestConfig::new("BTC/USDT", "1h", 100).with_max_count(50);
        let result = calculate_time_range(&config);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("exceeds max_count"));
    }

    #[test]
    fn calculate_time_range_zero_count() {
        let config = BacktestConfig::new("BTC/USDT", "1h", 0).with_end_time(3600);
        let (start, end) = calculate_time_range(&config).unwrap();
        assert_eq!(start, end);
        assert_eq!(start, 3600);
    }

    // ========================================================================
    // BacktestConfig builder tests
    // ========================================================================

    #[test]
    fn backtest_config_default_data_source_is_realtime() {
        let config = BacktestConfig::new("BTC/USDT", "1h", 10);
        assert_eq!(config.data_source, DataSource::RealTime);
    }

    #[test]
    fn backtest_config_builder_methods() {
        let config = BacktestConfig::new("BTC/USDT", "1h", 10)
            .with_end_time(1000)
            .with_data_source(DataSource::Synthetic)
            .with_max_count(5000);

        assert_eq!(config.end_time, Some(1000));
        assert_eq!(config.data_source, DataSource::Synthetic);
        assert_eq!(config.max_count, 5000);
    }

    // ========================================================================
    // BacktestDataProvider::get_bars tests
    // ========================================================================

    #[tokio::test]
    async fn provider_get_bars_from_existing_storage() {
        // Create 10 hours of 1-minute bars
        let bars: Vec<StandardBar> = (0..600)
            .map(|i| make_bar(i * 60, "42000", "42100", "41900", "42050", "100"))
            .collect();

        let storage = Arc::new(MockStorage::with_bars(bars));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let provider = BacktestDataProvider::with_storage_and_adapter(storage, adapter, 100);

        let config = make_config(10).with_end_time(600 * 60); // 600 minutes = 10 hours

        let result = provider.get_bars(&config).await;
        assert!(result.is_ok());
        let bars = result.unwrap();
        assert_eq!(bars.len(), 10); // 10 hourly bars
    }

    #[tokio::test]
    async fn provider_get_bars_fetches_missing_data() {
        let fetched_bars = vec![
            make_bar(0, "42000", "42100", "41900", "42050", "100"),
            make_bar(60, "42050", "42200", "42000", "42150", "200"),
        ];

        let storage = Arc::new(MockStorage::with_bars(vec![]));
        let adapter = Box::new(MockAdapter::with_bars(fetched_bars));
        let provider =
            BacktestDataProvider::with_storage_and_adapter(storage.clone(), adapter, 100);

        let config = BacktestConfig::new("BTC/USDT", "1m", 2).with_end_time(120);

        let result = provider.get_bars(&config).await;
        assert!(result.is_ok());
        let bars = result.unwrap();
        assert_eq!(bars.len(), 2);

        // Verify data was stored
        let stored = storage.bars.lock().await;
        assert_eq!(stored.len(), 2);
    }

    #[tokio::test]
    async fn provider_get_bars_insufficient_data() {
        let bars = vec![make_bar(0, "42000", "42100", "41900", "42050", "100")];

        let storage = Arc::new(MockStorage::with_bars(bars));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let provider = BacktestDataProvider::with_storage_and_adapter(storage, adapter, 100);

        let config = BacktestConfig::new("BTC/USDT", "1m", 10).with_end_time(600);

        let result = provider.get_bars(&config).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("insufficient data"));
    }

    #[tokio::test]
    async fn provider_get_bars_empty_storage() {
        let storage = Arc::new(MockStorage::with_bars(vec![]));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let provider = BacktestDataProvider::with_storage_and_adapter(storage, adapter, 100);

        let config = BacktestConfig::new("BTC/USDT", "1m", 5).with_end_time(300);

        let result = provider.get_bars(&config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn provider_get_bars_propagates_adapter_error() {
        let storage = Arc::new(MockStorage::with_bars(vec![]));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        adapter.set_fail(true);
        let provider = BacktestDataProvider::with_storage_and_adapter(storage, adapter, 100);

        let config = BacktestConfig::new("BTC/USDT", "1m", 5).with_end_time(300);

        let result = provider.get_bars(&config).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DataError::Exchange(_)));
    }

    #[tokio::test]
    async fn provider_get_bars_different_timeframes() {
        // Create 24 hours of 1-minute bars
        let bars: Vec<StandardBar> = (0..1440)
            .map(|i| make_bar(i * 60, "42000", "42100", "41900", "42050", "100"))
            .collect();

        let storage = Arc::new(MockStorage::with_bars(bars));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let provider = BacktestDataProvider::with_storage_and_adapter(storage, adapter, 100);

        // Test 1h timeframe
        let config_h1 = BacktestConfig::new("BTC/USDT", "1h", 24).with_end_time(1440 * 60);
        let bars_h1 = provider.get_bars(&config_h1).await.unwrap();
        assert_eq!(bars_h1.len(), 24);

        // Test 4h timeframe
        let config_h4 = BacktestConfig::new("BTC/USDT", "4h", 6).with_end_time(1440 * 60);
        let bars_h4 = provider.get_bars(&config_h4).await.unwrap();
        assert_eq!(bars_h4.len(), 6);

        // Test 1d timeframe
        let config_d1 = BacktestConfig::new("BTC/USDT", "1d", 1).with_end_time(1440 * 60);
        let bars_d1 = provider.get_bars(&config_d1).await.unwrap();
        assert_eq!(bars_d1.len(), 1);
    }

    #[tokio::test]
    async fn provider_get_bars_single_bar() {
        let bars = vec![make_bar(0, "42000", "42100", "41900", "42050", "100")];

        let storage = Arc::new(MockStorage::with_bars(bars));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let provider = BacktestDataProvider::with_storage_and_adapter(storage, adapter, 100);

        let config = BacktestConfig::new("BTC/USDT", "1m", 1).with_end_time(60);

        let result = provider.get_bars(&config).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn provider_get_bars_concurrent_requests() {
        let bars: Vec<StandardBar> = (0..600)
            .map(|i| make_bar(i * 60, "42000", "42100", "41900", "42050", "100"))
            .collect();

        let storage = Arc::new(MockStorage::with_bars(bars));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let provider = Arc::new(BacktestDataProvider::with_storage_and_adapter(
            storage, adapter, 100,
        ));

        let config = make_config(5).with_end_time(600 * 60);

        let mut handles = vec![];
        for _ in 0..5 {
            let p = provider.clone();
            let c = config.clone();
            handles.push(tokio::spawn(async move { p.get_bars(&c).await }));
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
            assert_eq!(result.unwrap().len(), 5);
        }
    }

    // ========================================================================
    // Synthetic data tests
    // ========================================================================

    #[tokio::test]
    async fn provider_get_bars_synthetic_fallback() {
        let storage = Arc::new(MockStorage::with_bars(vec![]));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let provider = BacktestDataProvider::with_storage_and_adapter(storage, adapter, 100);

        let config = BacktestConfig::new("BTC/USDT", "1h", 5)
            .with_end_time(3600 * 5)
            .with_data_source(DataSource::Synthetic);

        let result = provider.get_bars(&config).await;
        assert!(result.is_ok());
        let bars = result.unwrap();
        assert_eq!(bars.len(), 5);
        assert_eq!(bars[0].symbol, "BTC/USDT");
        assert_eq!(bars[0].exchange, "synthetic");
    }

    #[tokio::test]
    async fn provider_synthetic_bars_have_valid_ohlcv() {
        let storage = Arc::new(MockStorage::with_bars(vec![]));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let provider = BacktestDataProvider::with_storage_and_adapter(storage, adapter, 100);

        let config = BacktestConfig::new("BTC/USDT", "1h", 10)
            .with_end_time(3600 * 10)
            .with_data_source(DataSource::Synthetic);

        let bars = provider.get_bars(&config).await.unwrap();
        assert_eq!(bars.len(), 10);

        for bar in &bars {
            assert!(bar.high >= bar.low);
            assert!(bar.high >= bar.open);
            assert!(bar.high >= bar.close);
            assert!(bar.low <= bar.open);
            assert!(bar.low <= bar.close);
            assert!(bar.volume > Decimal::ZERO);
        }
    }

    // ========================================================================
    // verify_data tests
    // ========================================================================

    #[tokio::test]
    async fn provider_verify_data_complete() {
        let bars: Vec<StandardBar> = (0..60)
            .map(|i| make_bar(i * 60, "42000", "42100", "41900", "42050", "100"))
            .collect();

        let storage = Arc::new(MockStorage::with_bars(bars));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let provider = BacktestDataProvider::with_storage_and_adapter(storage, adapter, 100);

        let config = BacktestConfig::new("BTC/USDT", "1m", 60).with_end_time(3600);

        let integrity = provider.verify_data(&config).await.unwrap();
        assert_eq!(integrity.total_expected, 60);
        assert_eq!(integrity.total_actual, 60);
        assert_eq!(integrity.missing_segments, 0);
        assert_eq!(integrity.missing_bars, 0);
    }

    #[tokio::test]
    async fn provider_verify_data_with_gaps() {
        let bars = vec![
            make_bar(0, "42000", "42100", "41900", "42050", "100"),
            make_bar(60, "42050", "42200", "42000", "42150", "200"),
            make_bar(180, "42150", "42300", "42100", "42250", "300"),
        ];

        let storage = Arc::new(MockStorage::with_bars(bars));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let provider = BacktestDataProvider::with_storage_and_adapter(storage, adapter, 100);

        let config = BacktestConfig::new("BTC/USDT", "1m", 4).with_end_time(240);

        let integrity = provider.verify_data(&config).await.unwrap();
        assert_eq!(integrity.total_expected, 4);
        assert_eq!(integrity.total_actual, 3);
        assert_eq!(integrity.missing_segments, 1);
        assert_eq!(integrity.missing_bars, 1);
    }

    // ========================================================================
    // DataSource enum tests
    // ========================================================================

    #[test]
    fn datasource_default_is_realtime() {
        let source: DataSource = Default::default();
        assert_eq!(source, DataSource::RealTime);
    }

    #[test]
    fn datasource_equality() {
        assert_eq!(DataSource::RealTime, DataSource::RealTime);
        assert_eq!(DataSource::Synthetic, DataSource::Synthetic);
        assert_ne!(DataSource::RealTime, DataSource::Synthetic);
    }

    // ========================================================================
    // Edge case tests
    // ========================================================================

    #[tokio::test]
    async fn provider_get_bars_unsupported_symbol() {
        let storage = Arc::new(MockStorage::with_bars(vec![]));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let provider = BacktestDataProvider::with_storage_and_adapter(storage, adapter, 100);

        let config = BacktestConfig::new("UNKNOWN/PAIR", "1h", 5).with_end_time(3600 * 5);

        // The mock adapter returns empty bars for any symbol, so we get insufficient data
        let result = provider.get_bars(&config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn provider_get_bars_invalid_timeframe() {
        let storage = Arc::new(MockStorage::with_bars(vec![]));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let provider = BacktestDataProvider::with_storage_and_adapter(storage, adapter, 100);

        let config = BacktestConfig::new("BTC/USDT", "bogus", 5);

        let result = provider.get_bars(&config).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DataError::InvalidTimeFrame(_)
        ));
    }

    #[tokio::test]
    async fn provider_get_bars_large_count() {
        // 10_000 minutes = ~166.67 hours of data
        let bars: Vec<StandardBar> = (0..10_000)
            .map(|i| make_bar(i * 60, "42000", "42100", "41900", "42050", "100"))
            .collect();

        let storage = Arc::new(MockStorage::with_bars(bars));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let provider = BacktestDataProvider::with_storage_and_adapter(storage, adapter, 100);

        // Request 100 hourly bars ending at hour 100 (360_000 seconds).
        // With 10_000 minutes of data, bucket 0 covers [0,3600), bucket 99
        // covers [356_400, 360_000).  The raw data runs up to 599_940 so
        // everything is present.
        let config = BacktestConfig::new("BTC/USDT", "1h", 100)
            .with_end_time(100 * 3600)
            .with_max_count(10_000);

        let result = provider.get_bars(&config).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 100);
    }

    #[tokio::test]
    async fn provider_get_bars_different_exchanges() {
        // This test verifies that the provider works regardless of exchange
        let bars: Vec<StandardBar> = (0..120)
            .map(|i| StandardBar {
                timestamp: i * 60,
                open: Decimal::from(42000),
                high: Decimal::from(42100),
                low: Decimal::from(41900),
                close: Decimal::from(42050),
                volume: Decimal::from(100),
                symbol: "ETH/USDT".to_string(),
                exchange: "okx".to_string(),
                confirmed: true,
            })
            .collect();

        let storage = Arc::new(MockStorage::with_bars(bars));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let provider = BacktestDataProvider::with_storage_and_adapter(storage, adapter, 100);

        let config = BacktestConfig::new("ETH/USDT", "1h", 2).with_end_time(120 * 60);

        let result = provider.get_bars(&config).await;
        assert!(result.is_ok());
        let bars = result.unwrap();
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].symbol, "ETH/USDT");
    }

    #[tokio::test]
    async fn provider_new_constructor() {
        let storage = Arc::new(MockStorage::with_bars(vec![]));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let fetcher = DataFetcher::new(storage, adapter);
        let agg_engine = AggregationEngine::with_cache_size(None, 100);
        let provider = BacktestDataProvider::new(fetcher, agg_engine);

        assert_eq!(
            provider
                .agg_engine
                .get_bars("TEST", TimeFrame::M1, 0, 60)
                .await
                .is_err(),
            true
        );
    }
}
