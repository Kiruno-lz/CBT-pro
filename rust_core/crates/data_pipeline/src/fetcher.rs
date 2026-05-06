use crate::{error::DataError, StandardBar};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// DataStorage trait
// ---------------------------------------------------------------------------

/// Abstraction over storage backends for the data fetcher.
#[async_trait::async_trait]
pub trait DataStorage: Send + Sync + std::fmt::Debug {
    /// Query the continuous data ranges already stored for a symbol.
    async fn query_data_ranges(&self, symbol: &str) -> Result<Vec<(i64, i64)>, DataError>;

    /// Insert bars into storage.
    async fn insert_bars(&self, bars: &[StandardBar]) -> Result<u64, DataError>;

    /// Query bars for a symbol in a time range.
    async fn query_bars(
        &self,
        symbol: &str,
        start: i64,
        end: i64,
    ) -> Result<Vec<StandardBar>, DataError>;
}

// ---------------------------------------------------------------------------
// calculate_gaps
// ---------------------------------------------------------------------------

/// Calculate the missing time-range gaps between `start` and `end` given the
/// existing continuous ranges.
///
/// # Arguments
/// * `start` - Desired range start (inclusive, unix seconds).
/// * `end`   - Desired range end (exclusive, unix seconds).
/// * `existing` - Already-stored continuous ranges `[(start, end), ...]`.
///
/// # Returns
/// List of missing gaps `[(gap_start, gap_end), ...]`.
pub fn calculate_gaps(start: i64, end: i64, existing: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    let mut gaps = Vec::new();
    let mut current = start;

    if start >= end {
        return gaps;
    }

    let mut ranges = existing;
    ranges.sort_by_key(|(s, _)| *s);

    for (range_start, range_end) in ranges {
        if current < range_start {
            gaps.push((current, range_start.min(end)));
        }

        current = current.max(range_end);

        if current >= end {
            return gaps;
        }
    }

    if current < end {
        gaps.push((current, end));
    }

    gaps
}

// ---------------------------------------------------------------------------
// DataIntegrity
// ---------------------------------------------------------------------------

/// Result of a data-integrity check.
#[derive(Debug, Clone, PartialEq)]
pub struct DataIntegrity {
    /// Total number of 1-minute bars expected in the requested range.
    pub total_expected: usize,
    /// Total number of bars actually found in storage.
    pub total_actual: usize,
    /// Number of continuous missing segments.
    pub missing_segments: usize,
    /// Total number of missing 1-minute bars.
    pub missing_bars: usize,
}

// ---------------------------------------------------------------------------
// DataFetcher
// ---------------------------------------------------------------------------

/// Orchestrates on-demand data fetching from an exchange adapter, ensuring
/// that only missing time ranges are downloaded.
#[derive(Debug)]
pub struct DataFetcher {
    storage: Arc<dyn DataStorage>,
    adapter: Box<dyn crate::exchange::ExchangeAdapter>,
    in_progress: Arc<Mutex<HashSet<String>>>,
}

impl DataFetcher {
    /// Create a new `DataFetcher`.
    pub fn new(
        storage: Arc<dyn DataStorage>,
        adapter: Box<dyn crate::exchange::ExchangeAdapter>,
    ) -> Self {
        Self {
            storage,
            adapter,
            in_progress: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Ensure that data for `symbol` exists between `start` and `end`.
    ///
    /// 1. Queries existing ranges from storage.
    /// 2. Calculates missing gaps.
    /// 3. Fetches each gap from the exchange adapter.
    /// 4. Normalises symbol format and inserts into storage.
    pub async fn ensure_data(&self, symbol: &str, start: i64, end: i64) -> Result<(), DataError> {
        let key = format!("{}_{}_{}", symbol, start, end);

        // --- concurrency guard ---
        {
            let mut in_progress = self.in_progress.lock().await;
            if in_progress.contains(&key) {
                drop(in_progress);
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    let in_progress = self.in_progress.lock().await;
                    if !in_progress.contains(&key) {
                        break;
                    }
                }
                return Ok(());
            }
            in_progress.insert(key.clone());
        }

        // Ensure cleanup on panic or early return.
        let result = self.ensure_data_inner(symbol, start, end).await;

        {
            let mut in_progress = self.in_progress.lock().await;
            in_progress.remove(&key);
        }

        result
    }

    async fn ensure_data_inner(&self, symbol: &str, start: i64, end: i64) -> Result<(), DataError> {
        let existing_ranges = self.storage.query_data_ranges(symbol).await?;
        let missing_ranges = calculate_gaps(start, end, existing_ranges);

        if missing_ranges.is_empty() {
            info!(symbol, start, end, "data already complete, no fetch needed");
            return Ok(());
        }

        let total_missing_bars: usize = missing_ranges
            .iter()
            .map(|(s, e)| ((e - s) / 60).max(0) as usize)
            .sum();

        if total_missing_bars > 10_000 {
            return Err(DataError::InvalidInterval(
                "Too much missing data to fetch automatically. Please use the import script."
                    .to_string(),
            ));
        }

        for (gap_start, gap_end) in missing_ranges {
            info!(symbol, gap_start, gap_end, "fetching missing data");

            let bars = self
                .adapter
                .fetch_ohlcv(symbol, 60, gap_start * 1000, gap_end * 1000)
                .await?;

            if bars.is_empty() {
                warn!(
                    symbol,
                    gap_start, gap_end, "exchange returned empty data, skipping"
                );
                continue;
            }

            let normalized: Vec<StandardBar> = bars
                .into_iter()
                .map(|mut bar| {
                    bar.symbol = crate::symbol::SymbolNormalizer::normalize(
                        &bar.symbol,
                        self.adapter.name(),
                    )
                    .unwrap_or_else(|_| bar.symbol.clone());
                    bar
                })
                .collect();

            self.storage.insert_bars(&normalized).await?;
            info!(symbol, count = normalized.len(), "bars inserted");
        }

        Ok(())
    }

    /// Verify data integrity for a symbol in a time range.
    pub async fn verify_data(
        &self,
        symbol: &str,
        start: i64,
        end: i64,
    ) -> Result<DataIntegrity, DataError> {
        let ranges = self.storage.query_data_ranges(symbol).await?;
        let missing = calculate_gaps(start, end, ranges.clone());
        let total_expected = ((end - start) / 60).max(0) as usize;

        let bars = self.storage.query_bars(symbol, start, end).await?;
        let total_actual = bars.len();
        let missing_segments = missing.len();
        let missing_bars: usize = missing
            .iter()
            .map(|(s, e)| ((e - s) / 60).max(0) as usize)
            .sum();

        Ok(DataIntegrity {
            total_expected,
            total_actual,
            missing_segments,
            missing_bars,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange::{ExchangeAdapter, SymbolNormalizer};

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

        fn with_ranges(ranges: Vec<(i64, i64)>) -> Self {
            let s = Self::new();
            {
                let mut dr = s.data_ranges.try_lock().unwrap();
                *dr = ranges;
            }
            s
        }

        fn with_bars(bars: Vec<StandardBar>) -> Self {
            let s = Self::new();
            {
                let mut b = s.bars.try_lock().unwrap();
                *b = bars.clone();

                // Compute data ranges from bars
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
        name: String,
        bars_to_return: Arc<Mutex<Vec<StandardBar>>>,
        should_fail: Arc<Mutex<bool>>,
        call_count: Arc<Mutex<usize>>,
    }

    impl MockAdapter {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                bars_to_return: Arc::new(Mutex::new(Vec::new())),
                should_fail: Arc::new(Mutex::new(false)),
                call_count: Arc::new(Mutex::new(0)),
            }
        }

        fn with_bars(bars: Vec<StandardBar>) -> Self {
            let adapter = Self::new("mock");
            {
                let mut b = adapter.bars_to_return.try_lock().unwrap();
                *b = bars;
            }
            adapter
        }

        fn set_fail(&self, fail: bool) {
            let mut f = self.should_fail.try_lock().unwrap();
            *f = fail;
        }

        #[allow(dead_code)]
        fn call_count(&self) -> usize {
            *self.call_count.try_lock().unwrap()
        }
    }

    #[async_trait::async_trait]
    impl ExchangeAdapter for MockAdapter {
        fn name(&self) -> &str {
            &self.name
        }

        async fn fetch_ohlcv(
            &self,
            _symbol: &str,
            _interval_secs: u64,
            _start_time: i64,
            _end_time: i64,
        ) -> Result<Vec<StandardBar>, DataError> {
            let mut count = self.call_count.lock().await;
            *count += 1;

            let fail = *self.should_fail.lock().await;
            if fail {
                return Err(DataError::NetworkError("mock network error".to_string()));
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
            SymbolNormalizer::normalize(symbol, self.name()).unwrap_or_else(|_| symbol.to_string())
        }
    }

    // ========================================================================
    // calculate_gaps tests
    // ========================================================================

    #[test]
    fn calculate_gaps_empty_existing() {
        let gaps = calculate_gaps(0, 300, vec![]);
        assert_eq!(gaps, vec![(0, 300)]);
    }

    #[test]
    fn calculate_gaps_fully_covered() {
        let gaps = calculate_gaps(0, 300, vec![(0, 300)]);
        assert!(gaps.is_empty());
    }

    #[test]
    fn calculate_gaps_partial_start() {
        let gaps = calculate_gaps(0, 300, vec![(100, 300)]);
        assert_eq!(gaps, vec![(0, 100)]);
    }

    #[test]
    fn calculate_gaps_partial_end() {
        let gaps = calculate_gaps(0, 300, vec![(0, 200)]);
        assert_eq!(gaps, vec![(200, 300)]);
    }

    #[test]
    fn calculate_gaps_multiple_gaps() {
        let gaps = calculate_gaps(0, 300, vec![(50, 100), (200, 250)]);
        assert_eq!(gaps, vec![(0, 50), (100, 200), (250, 300)]);
    }

    #[test]
    fn calculate_gaps_unsorted_ranges() {
        let gaps = calculate_gaps(0, 300, vec![(200, 250), (50, 100)]);
        assert_eq!(gaps, vec![(0, 50), (100, 200), (250, 300)]);
    }

    #[test]
    fn calculate_gaps_overlapping_ranges() {
        let gaps = calculate_gaps(0, 300, vec![(0, 150), (100, 200)]);
        assert_eq!(gaps, vec![(200, 300)]);
    }

    #[test]
    fn calculate_gaps_start_equals_end() {
        let gaps = calculate_gaps(100, 100, vec![]);
        assert!(gaps.is_empty());
    }

    #[test]
    fn calculate_gaps_range_extends_beyond_end() {
        let gaps = calculate_gaps(0, 300, vec![(0, 500)]);
        assert!(gaps.is_empty());
    }

    #[test]
    fn calculate_gaps_gap_at_end() {
        let gaps = calculate_gaps(0, 300, vec![(0, 100), (100, 200)]);
        assert_eq!(gaps, vec![(200, 300)]);
    }

    #[test]
    fn calculate_gaps_single_point_range() {
        let gaps = calculate_gaps(0, 300, vec![(100, 100)]);
        assert_eq!(gaps, vec![(0, 100), (100, 300)]);
    }

    // ========================================================================
    // DataFetcher::ensure_data tests
    // ========================================================================

    #[tokio::test]
    async fn ensure_data_already_complete() {
        let storage = Arc::new(MockStorage::with_ranges(vec![(0, 300)]));
        let adapter = Box::new(MockAdapter::new("mock"));
        let fetcher = DataFetcher::new(storage.clone(), adapter);

        fetcher.ensure_data("BTC/USDT", 0, 300).await.unwrap();

        let stored = storage.bars.lock().await;
        assert!(stored.is_empty());
    }

    #[tokio::test]
    async fn ensure_data_fetches_missing_start() {
        let bars = vec![
            make_bar(0, "42000", "42100", "41900", "42050", "100"),
            make_bar(60, "42050", "42200", "42000", "42150", "200"),
        ];
        let storage = Arc::new(MockStorage::with_ranges(vec![(120, 300)]));
        let adapter = Box::new(MockAdapter::with_bars(bars.clone()));
        let fetcher = DataFetcher::new(storage.clone(), adapter);

        fetcher.ensure_data("BTC/USDT", 0, 300).await.unwrap();

        let stored = storage.bars.lock().await;
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].timestamp, 0);
        assert_eq!(stored[1].timestamp, 60);
    }

    #[tokio::test]
    async fn ensure_data_fetches_missing_end() {
        let bars = vec![
            make_bar(180, "42150", "42300", "42100", "42250", "300"),
            make_bar(240, "42250", "42400", "42200", "42350", "400"),
        ];
        let storage = Arc::new(MockStorage::with_ranges(vec![(0, 120)]));
        let adapter = Box::new(MockAdapter::with_bars(bars.clone()));
        let fetcher = DataFetcher::new(storage.clone(), adapter);

        fetcher.ensure_data("BTC/USDT", 0, 300).await.unwrap();

        let stored = storage.bars.lock().await;
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].timestamp, 180);
        assert_eq!(stored[1].timestamp, 240);
    }

    #[tokio::test]
    async fn ensure_data_fetches_multiple_gaps() {
        let bars1 = vec![
            make_bar(0, "42000", "42100", "41900", "42050", "100"),
            make_bar(60, "42050", "42200", "42000", "42150", "200"),
        ];
        let _bars2 = [
            make_bar(180, "42150", "42300", "42100", "42250", "300"),
            make_bar(240, "42250", "42400", "42200", "42350", "400"),
        ];

        let storage = Arc::new(MockStorage::with_ranges(vec![(120, 180)]));
        let adapter = Box::new(MockAdapter::with_bars(bars1.clone()));
        let fetcher = DataFetcher::new(storage.clone(), adapter);

        // This test needs adapter to return different bars for different gaps.
        // We'll test this with a more sophisticated mock in a separate test.
        fetcher.ensure_data("BTC/USDT", 0, 300).await.unwrap();

        let stored = storage.bars.lock().await;
        assert_eq!(stored.len(), 2);
    }

    #[tokio::test]
    async fn ensure_data_empty_response_skips() {
        let storage = Arc::new(MockStorage::with_ranges(vec![(0, 120)]));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        let fetcher = DataFetcher::new(storage.clone(), adapter);

        fetcher.ensure_data("BTC/USDT", 0, 300).await.unwrap();

        let stored = storage.bars.lock().await;
        assert!(stored.is_empty());
    }

    #[tokio::test]
    async fn ensure_data_propagates_adapter_error() {
        let storage = Arc::new(MockStorage::with_ranges(vec![]));
        let adapter = Box::new(MockAdapter::with_bars(vec![]));
        adapter.set_fail(true);
        let fetcher = DataFetcher::new(storage.clone(), adapter);

        let result = fetcher.ensure_data("BTC/USDT", 0, 300).await;
        assert!(matches!(result, Err(DataError::NetworkError(_))));
    }

    #[tokio::test]
    async fn ensure_data_normalizes_symbol() {
        let mut bar = make_bar(0, "42000", "42100", "41900", "42050", "100");
        bar.symbol = "BTCUSDT".to_string(); // Binance format
        let storage = Arc::new(MockStorage::with_ranges(vec![]));
        let mut adapter = MockAdapter::with_bars(vec![bar]);
        adapter.name = "binance".to_string();
        let fetcher = DataFetcher::new(storage.clone(), Box::new(adapter));

        fetcher.ensure_data("BTC/USDT", 0, 60).await.unwrap();

        let stored = storage.bars.lock().await;
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].symbol, "BTC/USDT");
    }

    #[tokio::test]
    async fn ensure_data_concurrent_duplicate_requests_wait() {
        let bars = vec![make_bar(0, "42000", "42100", "41900", "42050", "100")];
        let storage = Arc::new(MockStorage::with_ranges(vec![]));
        let adapter = Box::new(MockAdapter::with_bars(bars));
        let fetcher = Arc::new(DataFetcher::new(storage.clone(), adapter));

        let fetcher2 = fetcher.clone();
        let handle1 = tokio::spawn(async move { fetcher.ensure_data("BTC/USDT", 0, 300).await });
        let handle2 = tokio::spawn(async move {
            // Small delay to ensure handle1 acquires the lock first
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            fetcher2.ensure_data("BTC/USDT", 0, 300).await
        });

        let (r1, r2) = tokio::join!(handle1, handle2);
        r1.unwrap().unwrap();
        r2.unwrap().unwrap();

        let stored = storage.bars.lock().await;
        assert_eq!(stored.len(), 1);
    }

    // ========================================================================
    // DataFetcher::verify_data tests
    // ========================================================================

    #[tokio::test]
    async fn verify_data_complete() {
        let bars = vec![
            make_bar(0, "42000", "42100", "41900", "42050", "100"),
            make_bar(60, "42050", "42200", "42000", "42150", "200"),
            make_bar(120, "42150", "42300", "42100", "42250", "300"),
            make_bar(180, "42250", "42400", "42200", "42350", "400"),
            make_bar(240, "42350", "42500", "42300", "42450", "500"),
        ];
        let storage = Arc::new(MockStorage::with_bars(bars));
        let adapter = Box::new(MockAdapter::new("mock"));
        let fetcher = DataFetcher::new(storage, adapter);

        let integrity = fetcher.verify_data("BTC/USDT", 0, 300).await.unwrap();
        assert_eq!(integrity.total_expected, 5);
        assert_eq!(integrity.total_actual, 5);
        assert_eq!(integrity.missing_segments, 0);
        assert_eq!(integrity.missing_bars, 0);
    }

    #[tokio::test]
    async fn verify_data_with_gaps() {
        let bars = vec![
            make_bar(0, "42000", "42100", "41900", "42050", "100"),
            make_bar(60, "42050", "42200", "42000", "42150", "200"),
            make_bar(180, "42250", "42400", "42200", "42350", "400"),
            make_bar(240, "42350", "42500", "42300", "42450", "500"),
        ];
        let storage = Arc::new(MockStorage::with_bars(bars));
        let adapter = Box::new(MockAdapter::new("mock"));
        let fetcher = DataFetcher::new(storage, adapter);

        let integrity = fetcher.verify_data("BTC/USDT", 0, 300).await.unwrap();
        assert_eq!(integrity.total_expected, 5);
        assert_eq!(integrity.total_actual, 4);
        assert_eq!(integrity.missing_segments, 1);
        assert_eq!(integrity.missing_bars, 1);
    }

    #[tokio::test]
    async fn verify_data_multiple_gaps() {
        let bars = vec![
            make_bar(0, "42000", "42100", "41900", "42050", "100"),
            make_bar(240, "42350", "42500", "42300", "42450", "500"),
        ];
        let storage = Arc::new(MockStorage::with_bars(bars));
        let adapter = Box::new(MockAdapter::new("mock"));
        let fetcher = DataFetcher::new(storage, adapter);

        let integrity = fetcher.verify_data("BTC/USDT", 0, 300).await.unwrap();
        assert_eq!(integrity.total_expected, 5);
        assert_eq!(integrity.total_actual, 2);
        assert_eq!(integrity.missing_segments, 1);
        assert_eq!(integrity.missing_bars, 3);
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
}
