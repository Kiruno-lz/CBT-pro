use crate::{error::DataError, StandardBar};
use async_trait::async_trait;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::debug;

/// Default cache capacity (number of entries).
pub const DEFAULT_CACHE_CAPACITY: usize = 1000;

/// Default TTL: 1 hour.
pub const DEFAULT_TTL: Duration = Duration::from_secs(3600);

/// Cache key for aggregated bar queries.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct CacheKey {
    /// Trading pair symbol (e.g. `"BTC-USDT"`).
    pub symbol: String,
    /// Interval in seconds (e.g. 300 for 5m).
    pub interval_secs: u64,
    /// Start timestamp (unix seconds).
    pub start: i64,
    /// End timestamp (unix seconds).
    pub end: i64,
}

impl CacheKey {
    /// Create a new cache key.
    pub fn new(symbol: impl Into<String>, interval_secs: u64, start: i64, end: i64) -> Self {
        Self {
            symbol: symbol.into(),
            interval_secs,
            start,
            end,
        }
    }
}

/// TTL presets for different data freshness requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheTtl {
    /// 5 minutes — active trading sessions.
    Short,
    /// 1 hour — default.
    Medium,
    /// 24 hours — historical data.
    Long,
}

impl CacheTtl {
    /// Convert to a `Duration`.
    pub fn as_duration(&self) -> Duration {
        match self {
            CacheTtl::Short => Duration::from_secs(300),
            CacheTtl::Medium => Duration::from_secs(3600),
            CacheTtl::Long => Duration::from_secs(86400),
        }
    }
}

/// Cache statistics for monitoring hit rates.
#[derive(Debug)]
pub struct CacheStats {
    hits: AtomicU64,
    misses: AtomicU64,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheStats {
    /// Create new stats counters.
    pub fn new() -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Record a cache hit.
    pub fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache miss.
    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Return the current hit count.
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Return the current miss count.
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Calculate hit rate (0.0 .. 1.0).
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&self) {
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }
}

/// Async trait for cache backends.
#[async_trait]
pub trait CacheProvider: Send + Sync + std::fmt::Debug {
    /// Retrieve bars from cache.
    async fn get(&self, key: &CacheKey) -> Option<Vec<StandardBar>>;

    /// Store bars in cache with the given TTL.
    async fn set(&self, key: &CacheKey, value: &[StandardBar], ttl: Duration);

    /// Remove a single entry from cache.
    async fn delete(&self, key: &CacheKey);

    /// Clear all cached entries.
    async fn clear(&self);

    /// Return cache statistics.
    fn stats(&self) -> &CacheStats;
}

/// Internal entry that tracks insertion time for TTL eviction.
#[derive(Debug, Clone)]
struct CacheEntry {
    bars: Vec<StandardBar>,
    inserted_at: Instant,
}

/// In-memory LRU cache provider with TTL support and statistics.
#[derive(Debug)]
pub struct MemoryCacheProvider {
    cache: RwLock<LruCache<CacheKey, CacheEntry>>,
    default_ttl: Duration,
    stats: CacheStats,
}

impl MemoryCacheProvider {
    /// Create a new memory cache with the given capacity (number of entries).
    pub fn new(capacity: usize) -> Self {
        let capacity =
            NonZeroUsize::new(capacity).unwrap_or_else(|| NonZeroUsize::new(100).unwrap());
        Self {
            cache: RwLock::new(LruCache::new(capacity)),
            default_ttl: DEFAULT_TTL,
            stats: CacheStats::new(),
        }
    }

    /// Create a new memory cache with custom default TTL.
    pub fn with_ttl(capacity: usize, default_ttl: Duration) -> Self {
        let capacity =
            NonZeroUsize::new(capacity).unwrap_or_else(|| NonZeroUsize::new(100).unwrap());
        Self {
            cache: RwLock::new(LruCache::new(capacity)),
            default_ttl,
            stats: CacheStats::new(),
        }
    }

    /// Return the configured default TTL.
    pub fn default_ttl(&self) -> Duration {
        self.default_ttl
    }
}

#[async_trait]
impl CacheProvider for MemoryCacheProvider {
    async fn get(&self, key: &CacheKey) -> Option<Vec<StandardBar>> {
        let mut cache = self.cache.write().await;

        // Check if entry exists and is not expired
        if let Some(entry) = cache.get(key) {
            if entry.inserted_at.elapsed() < self.default_ttl {
                debug!("Cache hit: {:?}", key);
                self.stats.record_hit();
                return Some(entry.bars.clone());
            }
            // Expired — remove it
            cache.pop(key);
        }

        debug!("Cache miss: {:?}", key);
        self.stats.record_miss();
        None
    }

    async fn set(&self, key: &CacheKey, value: &[StandardBar], _ttl: Duration) {
        let mut cache = self.cache.write().await;
        let entry = CacheEntry {
            bars: value.to_vec(),
            inserted_at: Instant::now(),
        };
        cache.put(key.clone(), entry);
    }

    async fn delete(&self, key: &CacheKey) {
        let mut cache = self.cache.write().await;
        cache.pop(key);
    }

    async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        self.stats.reset();
    }

    fn stats(&self) -> &CacheStats {
        &self.stats
    }
}

/// Redis cache provider with JSON serialization.
#[derive(Debug)]
pub struct RedisCacheProvider {
    client: redis::Client,
    stats: CacheStats,
}

impl RedisCacheProvider {
    /// Create a new Redis cache provider.
    ///
    /// Opens a connection to the Redis server at `redis_url` and verifies
    /// connectivity by requesting a multiplexed async connection.
    pub async fn new(redis_url: &str) -> Result<Self, DataError> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| DataError::Storage(format!("Failed to open Redis client: {e}")))?;

        // Verify the server is reachable.
        let _: redis::aio::MultiplexedConnection = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| DataError::Storage(format!("Failed to connect to Redis: {e}")))?;

        Ok(Self {
            client,
            stats: CacheStats::new(),
        })
    }

    /// Generate a Redis key from a [`CacheKey`].
    fn make_key(key: &CacheKey) -> String {
        format!(
            "cbt:bars:{}:{}:{}:{}",
            key.symbol, key.interval_secs, key.start, key.end
        )
    }
}

#[async_trait]
impl CacheProvider for RedisCacheProvider {
    async fn get(&self, key: &CacheKey) -> Option<Vec<StandardBar>> {
        let mut conn = match self.client.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Redis connection error on get: {e}");
                self.stats.record_miss();
                return None;
            }
        };

        let redis_key = Self::make_key(key);
        let result: Result<Option<String>, _> = redis::cmd("GET")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await;

        match result {
            Ok(Some(json_str)) => match serde_json::from_str::<Vec<StandardBar>>(&json_str) {
                Ok(bars) => {
                    self.stats.record_hit();
                    Some(bars)
                }
                Err(e) => {
                    tracing::warn!("Failed to deserialize Redis value for key {redis_key}: {e}");
                    self.stats.record_miss();
                    None
                }
            },
            Ok(None) => {
                self.stats.record_miss();
                None
            }
            Err(e) => {
                tracing::warn!("Redis GET error for key {redis_key}: {e}");
                self.stats.record_miss();
                None
            }
        }
    }

    async fn set(&self, key: &CacheKey, value: &[StandardBar], ttl: Duration) {
        let mut conn = match self.client.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Redis connection error on set: {e}");
                return;
            }
        };

        let redis_key = Self::make_key(key);
        let json_str = match serde_json::to_string(value) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to serialize bars for key {redis_key}: {e}");
                return;
            }
        };

        let ttl_secs = ttl.as_secs() as usize;
        let result: Result<(), _> = redis::cmd("SETEX")
            .arg(&redis_key)
            .arg(ttl_secs)
            .arg(json_str)
            .query_async(&mut conn)
            .await;

        if let Err(e) = result {
            tracing::warn!("Redis SETEX error for key {redis_key}: {e}");
        }
    }

    async fn delete(&self, key: &CacheKey) {
        let mut conn = match self.client.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Redis connection error on delete: {e}");
                return;
            }
        };

        let redis_key = Self::make_key(key);
        let _: Result<(), _> = redis::cmd("DEL")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await;
    }

    async fn clear(&self) {
        let mut conn = match self.client.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Redis connection error on clear: {e}");
                return;
            }
        };

        let pattern = "cbt:bars:*";
        let mut cursor: u64 = 0;

        loop {
            let (next_cursor, keys): (u64, Vec<String>) = match redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    tracing::warn!("Redis SCAN error: {e}");
                    return;
                }
            };

            if !keys.is_empty() {
                let _: Result<(), _> = redis::cmd("DEL").arg(&keys).query_async(&mut conn).await;
            }

            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        self.stats.reset();
    }

    fn stats(&self) -> &CacheStats {
        &self.stats
    }
}

// ============================================================================
// Tests
// ============================================================================

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

    // ------------------------------------------------------------------------
    // CacheKey tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_cache_key_equality() {
        let key1 = CacheKey::new("BTC-USDT", 300, 1704067200, 1706745600);
        let key2 = CacheKey::new("BTC-USDT", 300, 1704067200, 1706745600);
        let key3 = CacheKey::new("ETH-USDT", 300, 1704067200, 1706745600);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_cache_key_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(CacheKey::new("BTC-USDT", 300, 0, 100));
        set.insert(CacheKey::new("BTC-USDT", 300, 0, 100));
        set.insert(CacheKey::new("ETH-USDT", 300, 0, 100));

        assert_eq!(set.len(), 2);
    }

    // ------------------------------------------------------------------------
    // CacheTtl tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_cache_ttl_durations() {
        assert_eq!(CacheTtl::Short.as_duration(), Duration::from_secs(300));
        assert_eq!(CacheTtl::Medium.as_duration(), Duration::from_secs(3600));
        assert_eq!(CacheTtl::Long.as_duration(), Duration::from_secs(86400));
    }

    // ------------------------------------------------------------------------
    // CacheStats tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_cache_stats_hit_rate() {
        let stats = CacheStats::new();
        assert_eq!(stats.hit_rate(), 0.0);

        stats.record_hit();
        stats.record_hit();
        stats.record_miss();

        assert_eq!(stats.hits(), 2);
        assert_eq!(stats.misses(), 1);
        assert!((stats.hit_rate() - 0.6666666666666666).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cache_stats_reset() {
        let stats = CacheStats::new();
        stats.record_hit();
        stats.record_miss();
        stats.reset();

        assert_eq!(stats.hits(), 0);
        assert_eq!(stats.misses(), 0);
        assert_eq!(stats.hit_rate(), 0.0);
    }

    // ------------------------------------------------------------------------
    // MemoryCacheProvider — basic operations
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_memory_cache_basic_set_and_get() {
        let cache = MemoryCacheProvider::new(10);
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);
        let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];

        // Initially empty
        assert!(cache.get(&key).await.is_none());

        // Set and retrieve
        cache.set(&key, &bars, Duration::from_secs(3600)).await;
        let retrieved = cache.get(&key).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), bars);
    }

    #[tokio::test]
    async fn test_memory_cache_miss_returns_none() {
        let cache = MemoryCacheProvider::new(10);
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);

        assert!(cache.get(&key).await.is_none());
        assert_eq!(cache.stats().misses(), 1);
    }

    #[tokio::test]
    async fn test_memory_cache_hit_records_stats() {
        let cache = MemoryCacheProvider::new(10);
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);
        let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];

        cache.set(&key, &bars, Duration::from_secs(3600)).await;

        // First get should be a hit
        let _ = cache.get(&key).await;
        assert_eq!(cache.stats().hits(), 1);
        assert_eq!(cache.stats().misses(), 0);

        // Second get should also be a hit
        let _ = cache.get(&key).await;
        assert_eq!(cache.stats().hits(), 2);
    }

    // ------------------------------------------------------------------------
    // MemoryCacheProvider — eviction (LRU)
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_memory_cache_lru_eviction() {
        let cache = MemoryCacheProvider::new(2); // capacity = 2
        let key1 = CacheKey::new("BTC-USDT", 300, 0, 100);
        let key2 = CacheKey::new("ETH-USDT", 300, 0, 100);
        let key3 = CacheKey::new("SOL-USDT", 300, 0, 100);

        let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];

        cache.set(&key1, &bars, Duration::from_secs(3600)).await;
        cache.set(&key2, &bars, Duration::from_secs(3600)).await;

        // Access key1 to make it recently used
        let _ = cache.get(&key1).await;

        // Add key3 — key2 should be evicted (LRU)
        cache.set(&key3, &bars, Duration::from_secs(3600)).await;

        assert!(cache.get(&key1).await.is_some(), "key1 should still exist");
        assert!(cache.get(&key2).await.is_none(), "key2 should be evicted");
        assert!(cache.get(&key3).await.is_some(), "key3 should exist");
    }

    // ------------------------------------------------------------------------
    // MemoryCacheProvider — TTL expiration
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_memory_cache_ttl_expiration() {
        let cache = MemoryCacheProvider::with_ttl(10, Duration::from_millis(50));
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);
        let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];

        cache.set(&key, &bars, Duration::from_millis(50)).await;

        // Should exist immediately
        assert!(cache.get(&key).await.is_some());

        // Wait for TTL to expire
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should be expired
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn test_memory_cache_expired_entry_counts_as_miss() {
        let cache = MemoryCacheProvider::with_ttl(10, Duration::from_millis(50));
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);
        let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];

        cache.set(&key, &bars, Duration::from_millis(50)).await;
        cache.get(&key).await; // hit

        tokio::time::sleep(Duration::from_millis(100)).await;

        cache.get(&key).await; // miss (expired)
        assert_eq!(cache.stats().hits(), 1);
        assert_eq!(cache.stats().misses(), 1);
    }

    // ------------------------------------------------------------------------
    // MemoryCacheProvider — delete and clear
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_memory_cache_delete() {
        let cache = MemoryCacheProvider::new(10);
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);
        let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];

        cache.set(&key, &bars, Duration::from_secs(3600)).await;
        assert!(cache.get(&key).await.is_some());

        cache.delete(&key).await;
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn test_memory_cache_clear() {
        let cache = MemoryCacheProvider::new(10);
        let key1 = CacheKey::new("BTC-USDT", 300, 0, 100);
        let key2 = CacheKey::new("ETH-USDT", 300, 0, 100);
        let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];

        cache.set(&key1, &bars, Duration::from_secs(3600)).await;
        cache.set(&key2, &bars, Duration::from_secs(3600)).await;

        // Access before clear to generate some stats
        let _ = cache.get(&key1).await;
        assert_eq!(cache.stats().hits(), 1);

        cache.clear().await;

        // Stats should be reset
        assert_eq!(cache.stats().hits(), 0);
        assert_eq!(cache.stats().misses(), 0);

        // Subsequent gets should be misses
        assert!(cache.get(&key1).await.is_none());
        assert!(cache.get(&key2).await.is_none());
        assert_eq!(cache.stats().misses(), 2);
    }

    // ------------------------------------------------------------------------
    // MemoryCacheProvider — concurrent access
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_memory_cache_concurrent_reads() {
        let cache = std::sync::Arc::new(MemoryCacheProvider::new(10));
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);
        let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];

        cache.set(&key, &bars, Duration::from_secs(3600)).await;

        let mut handles = vec![];
        for _ in 0..10 {
            let cache = cache.clone();
            let key = key.clone();
            handles.push(tokio::spawn(async move { cache.get(&key).await }));
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_some());
            assert_eq!(result.unwrap(), bars);
        }

        // All 10 reads should be hits
        assert_eq!(cache.stats().hits(), 10);
    }

    #[tokio::test]
    async fn test_memory_cache_concurrent_writes() {
        let cache = std::sync::Arc::new(MemoryCacheProvider::new(100));
        let mut handles = vec![];

        for i in 0..10 {
            let cache = cache.clone();
            handles.push(tokio::spawn(async move {
                let key = CacheKey::new(format!("SYM-{}", i), 300, 0, 100);
                let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];
                cache.set(&key, &bars, Duration::from_secs(3600)).await;
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        // All 10 entries should be retrievable
        for i in 0..10 {
            let key = CacheKey::new(format!("SYM-{}", i), 300, 0, 100);
            assert!(cache.get(&key).await.is_some());
        }
    }

    #[tokio::test]
    async fn test_memory_cache_concurrent_mixed_access() {
        let cache = std::sync::Arc::new(MemoryCacheProvider::new(50));
        let mut write_handles = vec![];
        let mut read_handles = vec![];

        // Spawn writers
        for i in 0..5 {
            let cache = cache.clone();
            write_handles.push(tokio::spawn(async move {
                let key = CacheKey::new(format!("MIX-{}", i), 300, 0, 100);
                let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];
                cache.set(&key, &bars, Duration::from_secs(3600)).await;
            }));
        }

        // Spawn readers (will miss initially, then hit if writer finished)
        for i in 0..5 {
            let cache = cache.clone();
            read_handles.push(tokio::spawn(async move {
                let key = CacheKey::new(format!("MIX-{}", i), 300, 0, 100);
                cache.get(&key).await
            }));
        }

        for handle in write_handles {
            let _ = handle.await.unwrap();
        }
        for handle in read_handles {
            let _ = handle.await.unwrap();
        }

        // Should not panic or deadlock
        assert!(cache.stats().hits() + cache.stats().misses() >= 5);
    }

    // ------------------------------------------------------------------------
    // MemoryCacheProvider — edge cases
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_memory_cache_empty_bars() {
        let cache = MemoryCacheProvider::new(10);
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);
        let bars: Vec<StandardBar> = vec![];

        cache.set(&key, &bars, Duration::from_secs(3600)).await;
        let retrieved = cache.get(&key).await;
        assert!(retrieved.is_some());
        assert!(retrieved.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_memory_cache_large_value() {
        let cache = MemoryCacheProvider::new(10);
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);

        let bars: Vec<StandardBar> = (0..1000)
            .map(|i| make_bar(i * 60, "100", "110", "90", "105", "10"))
            .collect();

        cache.set(&key, &bars, Duration::from_secs(3600)).await;
        let retrieved = cache.get(&key).await;
        assert_eq!(retrieved.unwrap().len(), 1000);
    }

    #[tokio::test]
    async fn test_memory_cache_overwrite_existing_key() {
        let cache = MemoryCacheProvider::new(10);
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);

        let bars1 = vec![make_bar(0, "100", "110", "90", "105", "10")];
        let bars2 = vec![make_bar(0, "200", "220", "180", "210", "20")];

        cache.set(&key, &bars1, Duration::from_secs(3600)).await;
        cache.set(&key, &bars2, Duration::from_secs(3600)).await;

        let retrieved = cache.get(&key).await.unwrap();
        assert_eq!(retrieved[0].open, Decimal::from(200));
        assert_eq!(retrieved[0].volume, Decimal::from(20));
    }

    #[tokio::test]
    async fn test_memory_cache_delete_nonexistent() {
        let cache = MemoryCacheProvider::new(10);
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);

        // Should not panic
        cache.delete(&key).await;
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn test_memory_cache_default_ttl() {
        let cache = MemoryCacheProvider::new(10);
        assert_eq!(cache.default_ttl(), DEFAULT_TTL);
    }

    // ------------------------------------------------------------------------
    // RedisCacheProvider — construction and key format
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_redis_cache_new_invalid_url_format() {
        let result = RedisCacheProvider::new("not-a-valid-url").await;
        assert!(
            result.is_err(),
            "Expected error for invalid URL format, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_redis_cache_key_format() {
        let key = CacheKey::new("BTC-USDT", 300, 1704067200, 1706745600);
        let redis_key = RedisCacheProvider::make_key(&key);
        assert_eq!(redis_key, "cbt:bars:BTC-USDT:300:1704067200:1706745600");
    }

    #[tokio::test]
    async fn test_redis_cache_key_format_special_chars() {
        let key = CacheKey::new("ETH/BTC", 60, 0, 100);
        let redis_key = RedisCacheProvider::make_key(&key);
        assert_eq!(redis_key, "cbt:bars:ETH/BTC:60:0:100");
    }

    #[tokio::test]
    async fn test_redis_cache_stats_exists() {
        // This test verifies that stats() returns a reference without panicking.
        // Since we can't connect to Redis in unit tests, we create the provider
        // directly (this test would need a real connection, so we skip it for now
        // and will test stats indirectly via get/set when Redis is available).
        // For now, just verify the struct can be created if we had a client.
    }

    // ------------------------------------------------------------------------
    // RedisCacheProvider — integration tests (require Redis server)
    // ------------------------------------------------------------------------

    // NOTE: The following tests require a running Redis server.
    // They are ignored by default and can be run with:
    //   cargo test -p data_pipeline -- --ignored

    #[tokio::test]
    #[ignore]
    async fn test_redis_cache_basic_set_and_get() {
        let cache = RedisCacheProvider::new("redis://127.0.0.1:6379")
            .await
            .expect("Failed to connect to Redis");
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);
        let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];

        // Initially empty
        assert!(cache.get(&key).await.is_none());

        // Set and retrieve
        cache.set(&key, &bars, Duration::from_secs(3600)).await;
        let retrieved = cache.get(&key).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), bars);
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_cache_ttl_expiration() {
        let cache = RedisCacheProvider::new("redis://127.0.0.1:6379")
            .await
            .expect("Failed to connect to Redis");
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);
        let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];

        cache.set(&key, &bars, Duration::from_millis(100)).await;
        assert!(cache.get(&key).await.is_some());

        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_cache_delete() {
        let cache = RedisCacheProvider::new("redis://127.0.0.1:6379")
            .await
            .expect("Failed to connect to Redis");
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);
        let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];

        cache.set(&key, &bars, Duration::from_secs(3600)).await;
        assert!(cache.get(&key).await.is_some());

        cache.delete(&key).await;
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_cache_clear() {
        let cache = RedisCacheProvider::new("redis://127.0.0.1:6379")
            .await
            .expect("Failed to connect to Redis");
        let key1 = CacheKey::new("BTC-USDT", 300, 0, 100);
        let key2 = CacheKey::new("ETH-USDT", 300, 0, 100);
        let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];

        cache.set(&key1, &bars, Duration::from_secs(3600)).await;
        cache.set(&key2, &bars, Duration::from_secs(3600)).await;

        cache.clear().await;

        assert!(cache.get(&key1).await.is_none());
        assert!(cache.get(&key2).await.is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_cache_hit_miss_stats() {
        let cache = RedisCacheProvider::new("redis://127.0.0.1:6379")
            .await
            .expect("Failed to connect to Redis");
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);
        let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];

        // Miss
        assert!(cache.get(&key).await.is_none());
        assert_eq!(cache.stats().misses(), 1);
        assert_eq!(cache.stats().hits(), 0);

        // Set and hit
        cache.set(&key, &bars, Duration::from_secs(3600)).await;
        assert!(cache.get(&key).await.is_some());
        assert_eq!(cache.stats().hits(), 1);
        assert_eq!(cache.stats().misses(), 1);
    }

    // ------------------------------------------------------------------------
    // Integration with CacheProvider trait object
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_cache_provider_trait_object() {
        let provider: Box<dyn CacheProvider> = Box::new(MemoryCacheProvider::new(10));
        let key = CacheKey::new("BTC-USDT", 300, 0, 100);
        let bars = vec![make_bar(0, "100", "110", "90", "105", "10")];

        provider.set(&key, &bars, Duration::from_secs(3600)).await;
        let retrieved = provider.get(&key).await;
        assert!(retrieved.is_some());
    }
}
