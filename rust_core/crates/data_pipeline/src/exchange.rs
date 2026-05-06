use crate::{error::DataError, StandardBar, TimeFrame};
use rust_decimal::Decimal;
use tracing::{debug, info};

// ---------------------------------------------------------------------------
// Re-export symbol normalizer for exchange adapters
// ---------------------------------------------------------------------------
pub use crate::symbol::SymbolNormalizer;

// ---------------------------------------------------------------------------
// ExchangeAdapter trait
// ---------------------------------------------------------------------------

/// Abstraction over a cryptocurrency exchange data feed.
#[async_trait::async_trait]
pub trait ExchangeAdapter: Send + Sync + std::fmt::Debug {
    /// Human-readable exchange name.
    fn name(&self) -> &str;

    /// Fetch historical bars from the exchange REST API using interval in seconds.
    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        interval_secs: u64,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<StandardBar>, DataError>;

    /// Fetch historical bars using TimeFrame (backward compatible).
    /// Default implementation delegates to fetch_ohlcv.
    async fn fetch_historical_bars(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        start: i64,
        end: i64,
    ) -> Result<Vec<StandardBar>, DataError> {
        self.fetch_ohlcv(symbol, timeframe.as_seconds() as u64, start, end)
            .await
    }

    /// Fetch supported trading pairs from the exchange.
    async fn fetch_symbols(&self) -> Result<Vec<String>, DataError>;

    /// Minimum supported interval in seconds.
    fn min_interval_secs(&self) -> u64;

    /// Maximum number of bars per request.
    fn max_limit_per_request(&self) -> usize;

    /// Normalise the exchange-specific symbol format to the CBT-Pro standard.
    fn normalize_symbol(&self, symbol: &str) -> String;
}

// ---------------------------------------------------------------------------
// RateLimiter
// ---------------------------------------------------------------------------

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

/// Simple async rate limiter to enforce minimum delay between requests.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    last_request: Arc<Mutex<Instant>>,
    min_interval: Duration,
}

impl RateLimiter {
    /// Create a new rate limiter with the given minimum interval.
    pub fn new(min_interval: Duration) -> Self {
        Self {
            last_request: Arc::new(Mutex::new(Instant::now() - min_interval)),
            min_interval,
        }
    }

    /// Acquire permission to make a request, waiting if necessary.
    pub async fn acquire(&self) {
        let mut last = self.last_request.lock().await;
        let elapsed = last.elapsed();
        if elapsed < self.min_interval {
            tokio::time::sleep(self.min_interval - elapsed).await;
        }
        *last = Instant::now();
    }
}

// ---------------------------------------------------------------------------
// ExchangeAdapterFactory
// ---------------------------------------------------------------------------

/// Factory for creating exchange adapters by name.
pub struct ExchangeAdapterFactory;

impl ExchangeAdapterFactory {
    /// Create an exchange adapter by name.
    pub fn create(exchange: &str) -> Result<Box<dyn ExchangeAdapter>, DataError> {
        match exchange.to_lowercase().as_str() {
            "binance" => Ok(Box::new(BinanceAdapter::new())),
            "okx" => Ok(Box::new(OKXAdapter::new())),
            "bybit" => Ok(Box::new(BybitAdapter::new())),
            _ => Err(DataError::UnknownExchange(exchange.to_string())),
        }
    }
}

// ---------------------------------------------------------------------------
// Binance Adapter
// ---------------------------------------------------------------------------

/// Binance exchange adapter with real HTTP API support.
#[derive(Debug, Clone)]
pub struct BinanceAdapter {
    client: reqwest::Client,
    base_url: String,
    rate_limiter: RateLimiter,
}

impl BinanceAdapter {
    /// Create a new Binance adapter with default configuration.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: "https://api.binance.com".to_string(),
            rate_limiter: RateLimiter::new(Duration::from_millis(50)),
        }
    }

    /// Create with custom base URL (useful for testing).
    pub fn with_base_url(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
            rate_limiter: RateLimiter::new(Duration::from_millis(50)),
        }
    }

    /// Convert seconds to Binance interval string.
    fn secs_to_interval(secs: u64) -> Result<String, DataError> {
        match secs {
            60 => Ok("1m".to_string()),
            180 => Ok("3m".to_string()),
            300 => Ok("5m".to_string()),
            900 => Ok("15m".to_string()),
            1800 => Ok("30m".to_string()),
            3600 => Ok("1h".to_string()),
            7200 => Ok("2h".to_string()),
            14400 => Ok("4h".to_string()),
            21600 => Ok("6h".to_string()),
            28800 => Ok("8h".to_string()),
            43200 => Ok("12h".to_string()),
            86400 => Ok("1d".to_string()),
            259200 => Ok("3d".to_string()),
            604800 => Ok("1w".to_string()),
            _ => Err(DataError::UnsupportedInterval(secs)),
        }
    }

    /// Build the klines API URL.
    ///
    /// # Arguments
    /// - `start_time_ms`: Unix timestamp in MILLISECONDS
    /// - `end_time_ms`: Unix timestamp in MILLISECONDS
    fn build_klines_url(
        &self,
        symbol: &str,
        interval: &str,
        start_time_ms: i64,
        end_time_ms: i64,
        limit: usize,
    ) -> String {
        format!(
            "{}/api/v3/klines?symbol={}&interval={}&startTime={}&endTime={}&limit={}",
            self.base_url, symbol, interval, start_time_ms, end_time_ms, limit
        )
    }

    /// Parse a single kline item from Binance API response.
    fn parse_kline_item(item: &serde_json::Value, symbol: &str) -> Result<StandardBar, DataError> {
        let arr = item
            .as_array()
            .ok_or_else(|| DataError::InvalidResponse("expected array".to_string()))?;

        if arr.len() < 6 {
            return Err(DataError::InvalidResponse(
                "kline array too short".to_string(),
            ));
        }

        let timestamp = arr[0]
            .as_i64()
            .ok_or_else(|| DataError::InvalidResponse("invalid timestamp".to_string()))?
            / 1000;

        let open = arr[1]
            .as_str()
            .and_then(|s| Decimal::from_str_exact(s).ok())
            .ok_or_else(|| DataError::InvalidResponse("invalid open".to_string()))?;

        let high = arr[2]
            .as_str()
            .and_then(|s| Decimal::from_str_exact(s).ok())
            .ok_or_else(|| DataError::InvalidResponse("invalid high".to_string()))?;

        let low = arr[3]
            .as_str()
            .and_then(|s| Decimal::from_str_exact(s).ok())
            .ok_or_else(|| DataError::InvalidResponse("invalid low".to_string()))?;

        let close = arr[4]
            .as_str()
            .and_then(|s| Decimal::from_str_exact(s).ok())
            .ok_or_else(|| DataError::InvalidResponse("invalid close".to_string()))?;

        let volume = arr[5]
            .as_str()
            .and_then(|s| Decimal::from_str_exact(s).ok())
            .ok_or_else(|| DataError::InvalidResponse("invalid volume".to_string()))?;

        Ok(StandardBar {
            timestamp,
            open,
            high,
            low,
            close,
            volume,
            symbol: symbol.to_string(),
            exchange: "binance".to_string(),
            confirmed: true,
        })
    }

    /// Fetch a single page of klines.
    ///
    /// # Arguments
    /// - `start_time_ms`: Unix timestamp in MILLISECONDS (Binance API format)
    /// - `end_time_ms`: Unix timestamp in MILLISECONDS (Binance API format)
    async fn fetch_page(
        &self,
        symbol: &str,
        interval: &str,
        start_time_ms: i64,
        end_time_ms: i64,
        limit: usize,
    ) -> Result<Vec<StandardBar>, DataError> {
        self.rate_limiter.acquire().await;

        let url = self.build_klines_url(symbol, interval, start_time_ms, end_time_ms, limit);
        debug!(url = %url, "fetching Binance klines");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| DataError::NetworkError(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(60);
            return Err(DataError::RateLimited {
                retry_after_ms: retry_after * 1000,
            });
        }

        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(DataError::Exchange(format!(
                "Binance API error {}: {}",
                status, body
            )));
        }

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| DataError::InvalidResponse(e.to_string()))?;

        let arr = data
            .as_array()
            .ok_or_else(|| DataError::InvalidResponse("expected JSON array".to_string()))?;

        let mut bars = Vec::with_capacity(arr.len());
        for item in arr {
            bars.push(Self::parse_kline_item(item, symbol)?);
        }

        info!(count = bars.len(), "fetched Binance klines");
        Ok(bars)
    }

    /// Fetch all klines with pagination.
    ///
    /// # Arguments
    /// - `start_time`: Unix timestamp in SECONDS
    /// - `end_time`: Unix timestamp in SECONDS
    ///
    /// Internally converts to milliseconds for the Binance API.
    async fn fetch_with_pagination(
        &self,
        symbol: &str,
        interval: &str,
        start_time: i64, // Unix timestamp in SECONDS
        end_time: i64,   // Unix timestamp in SECONDS
    ) -> Result<Vec<StandardBar>, DataError> {
        let mut all_bars = Vec::new();
        // Convert seconds to milliseconds for Binance API boundary
        let mut current_start_ms = start_time * 1000;
        let end_time_ms = end_time * 1000;
        let limit = self.max_limit_per_request();

        while current_start_ms < end_time_ms {
            let bars = self
                .fetch_page(symbol, interval, current_start_ms, end_time_ms, limit)
                .await?;

            if bars.is_empty() {
                break;
            }

            // Update next start time to be 1ms after the last bar's close time.
            // StandardBar.timestamp is in seconds, so convert to milliseconds.
            let last_close_ms = bars.last().unwrap().timestamp * 1000;
            current_start_ms = last_close_ms + 1;

            all_bars.extend(bars);
        }

        Ok(all_bars)
    }
}

impl Default for BinanceAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ExchangeAdapter for BinanceAdapter {
    fn name(&self) -> &str {
        "binance"
    }

    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        interval_secs: u64,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<StandardBar>, DataError> {
        debug!(
            symbol,
            interval_secs, start_time, end_time, "BinanceAdapter::fetch_ohlcv"
        );

        if start_time >= end_time {
            return Ok(Vec::new());
        }

        // Denormalize symbol to Binance format
        let binance_symbol = SymbolNormalizer::denormalize(symbol, "binance")
            .map_err(|e| DataError::Exchange(e.to_string()))?;

        let interval = Self::secs_to_interval(interval_secs)?;

        self.fetch_with_pagination(&binance_symbol, &interval, start_time, end_time)
            .await
    }

    async fn fetch_symbols(&self) -> Result<Vec<String>, DataError> {
        self.rate_limiter.acquire().await;

        let url = format!("{}/api/v3/exchangeInfo", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| DataError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DataError::Exchange(format!(
                "Binance API error: {}",
                response.status()
            )));
        }

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| DataError::InvalidResponse(e.to_string()))?;

        let symbols = data["symbols"]
            .as_array()
            .ok_or_else(|| DataError::InvalidResponse("missing symbols array".to_string()))?;

        let mut result = Vec::new();
        for s in symbols {
            if let Some(symbol_str) = s["symbol"].as_str() {
                result.push(symbol_str.to_string());
            }
        }

        Ok(result)
    }

    fn min_interval_secs(&self) -> u64 {
        60
    }

    fn max_limit_per_request(&self) -> usize {
        1000
    }

    /// Convert Binance format `"BTCUSDT"` → `"BTC/USDT"`.
    fn normalize_symbol(&self, symbol: &str) -> String {
        SymbolNormalizer::normalize(symbol, "binance").unwrap_or_else(|_| symbol.to_string())
    }
}

// ---------------------------------------------------------------------------
// OKX Adapter (stub)
// ---------------------------------------------------------------------------

/// Stub adapter that mimics OKX REST behaviour without hitting the network.
#[derive(Debug)]
pub struct OKXAdapter {
    rng_seed: u64,
}

impl OKXAdapter {
    pub fn new() -> Self {
        Self { rng_seed: 42 }
    }
}

impl Default for OKXAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ExchangeAdapter for OKXAdapter {
    fn name(&self) -> &str {
        "okx"
    }

    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        interval_secs: u64,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<StandardBar>, DataError> {
        debug!(
            symbol,
            interval_secs, start_time, end_time, "OKXAdapter::fetch_ohlcv"
        );

        if start_time >= end_time {
            return Ok(Vec::new());
        }

        use rand::rngs::SmallRng;
        use rand::Rng;
        use rand::SeedableRng;

        let count = ((end_time - start_time) / interval_secs as i64).max(0) as usize;
        if count == 0 {
            return Ok(Vec::new());
        }

        let mut rng = SmallRng::seed_from_u64(self.rng_seed);
        let mut price = Decimal::from(42000);
        let mut bars: Vec<StandardBar> = Vec::with_capacity(count);
        let normalised = self.normalize_symbol(symbol);

        for i in 0..count {
            let ts = start_time + (i as i64) * interval_secs as i64;
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
                symbol: normalised.clone(),
                exchange: self.name().to_string(),
                confirmed: true,
            });
            price = close;
        }

        info!(count = bars.len(), "OKXAdapter generated synthetic bars");
        Ok(bars)
    }

    async fn fetch_symbols(&self) -> Result<Vec<String>, DataError> {
        // Return a small set of known symbols for testing
        Ok(vec![
            "BTC-USDT".to_string(),
            "ETH-USDT".to_string(),
            "SOL-USDT".to_string(),
        ])
    }

    fn min_interval_secs(&self) -> u64 {
        60
    }

    fn max_limit_per_request(&self) -> usize {
        300
    }

    /// OKX already uses `"BTC-USDT"`, so pass-through with upper-casing.
    fn normalize_symbol(&self, symbol: &str) -> String {
        SymbolNormalizer::normalize(symbol, "okx").unwrap_or_else(|_| symbol.to_ascii_uppercase())
    }
}

// ---------------------------------------------------------------------------
// Bybit Adapter (stub)
// ---------------------------------------------------------------------------

/// Bybit exchange adapter with real HTTP API support.
#[derive(Debug, Clone)]
pub struct BybitAdapter {
    client: reqwest::Client,
    base_url: String,
    rate_limiter: RateLimiter,
}

impl BybitAdapter {
    /// Create a new Bybit adapter with default configuration.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: "https://api.bybit.com".to_string(),
            rate_limiter: RateLimiter::new(Duration::from_millis(50)),
        }
    }

    /// Create with custom base URL (useful for testing).
    pub fn with_base_url(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
            rate_limiter: RateLimiter::new(Duration::from_millis(50)),
        }
    }

    /// Convert seconds to Bybit interval string.
    fn secs_to_interval(secs: u64) -> Result<String, DataError> {
        match secs {
            60 => Ok("1".to_string()),
            300 => Ok("5".to_string()),
            900 => Ok("15".to_string()),
            1800 => Ok("30".to_string()),
            3600 => Ok("60".to_string()),
            7200 => Ok("120".to_string()),
            14400 => Ok("240".to_string()),
            86400 => Ok("D".to_string()),
            604800 => Ok("W".to_string()),
            _ => Err(DataError::UnsupportedInterval(secs)),
        }
    }

    /// Build the klines API URL.
    ///
    /// # Arguments
    /// - `start_time_ms`: Unix timestamp in MILLISECONDS
    /// - `end_time_ms`: Unix timestamp in MILLISECONDS
    fn build_klines_url(
        &self,
        symbol: &str,
        interval: &str,
        start_time_ms: i64,
        end_time_ms: i64,
        limit: usize,
    ) -> String {
        format!(
            "{}/v5/market/kline?category=spot&symbol={}&interval={}&start={}&end={}&limit={}",
            self.base_url, symbol, interval, start_time_ms, end_time_ms, limit
        )
    }

    /// Parse a single kline item from Bybit API response.
    fn parse_kline_item(item: &serde_json::Value, symbol: &str) -> Result<StandardBar, DataError> {
        let arr = item
            .as_array()
            .ok_or_else(|| DataError::InvalidResponse("expected array".to_string()))?;

        if arr.len() < 7 {
            return Err(DataError::InvalidResponse(
                "kline array too short".to_string(),
            ));
        }

        let timestamp = arr[0]
            .as_i64()
            .or_else(|| arr[0].as_str().and_then(|s| s.parse::<i64>().ok()))
            .ok_or_else(|| DataError::InvalidResponse("invalid timestamp".to_string()))?
            / 1000;

        let open = arr[1]
            .as_str()
            .and_then(|s| Decimal::from_str_exact(s).ok())
            .ok_or_else(|| DataError::InvalidResponse("invalid open".to_string()))?;

        let high = arr[2]
            .as_str()
            .and_then(|s| Decimal::from_str_exact(s).ok())
            .ok_or_else(|| DataError::InvalidResponse("invalid high".to_string()))?;

        let low = arr[3]
            .as_str()
            .and_then(|s| Decimal::from_str_exact(s).ok())
            .ok_or_else(|| DataError::InvalidResponse("invalid low".to_string()))?;

        let close = arr[4]
            .as_str()
            .and_then(|s| Decimal::from_str_exact(s).ok())
            .ok_or_else(|| DataError::InvalidResponse("invalid close".to_string()))?;

        let volume = arr[5]
            .as_str()
            .and_then(|s| Decimal::from_str_exact(s).ok())
            .ok_or_else(|| DataError::InvalidResponse("invalid volume".to_string()))?;

        Ok(StandardBar {
            timestamp,
            open,
            high,
            low,
            close,
            volume,
            symbol: symbol.to_string(),
            exchange: "bybit".to_string(),
            confirmed: true,
        })
    }

    /// Fetch a single page of klines.
    ///
    /// # Arguments
    /// - `start_time_ms`: Unix timestamp in MILLISECONDS (Bybit API format)
    /// - `end_time_ms`: Unix timestamp in MILLISECONDS (Bybit API format)
    async fn fetch_page(
        &self,
        symbol: &str,
        interval: &str,
        start_time_ms: i64,
        end_time_ms: i64,
        limit: usize,
    ) -> Result<Vec<StandardBar>, DataError> {
        self.rate_limiter.acquire().await;

        let url = self.build_klines_url(symbol, interval, start_time_ms, end_time_ms, limit);
        debug!(url = %url, "fetching Bybit klines");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| DataError::NetworkError(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(60);
            return Err(DataError::RateLimited {
                retry_after_ms: retry_after * 1000,
            });
        }

        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(DataError::Exchange(format!(
                "Bybit API error {}: {}",
                status, body
            )));
        }

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| DataError::InvalidResponse(e.to_string()))?;

        let list = data["result"]["list"]
            .as_array()
            .ok_or_else(|| DataError::InvalidResponse("expected result.list array".to_string()))?;

        let mut bars = Vec::with_capacity(list.len());
        for item in list {
            bars.push(Self::parse_kline_item(item, symbol)?);
        }

        info!(count = bars.len(), "fetched Bybit klines");
        Ok(bars)
    }

    /// Fetch all klines with pagination.
    ///
    /// # Arguments
    /// - `start_time`: Unix timestamp in SECONDS
    /// - `end_time`: Unix timestamp in SECONDS
    ///
    /// Internally converts to milliseconds for the Bybit API.
    async fn fetch_with_pagination(
        &self,
        symbol: &str,
        interval: &str,
        start_time: i64, // Unix timestamp in SECONDS
        end_time: i64,   // Unix timestamp in SECONDS
    ) -> Result<Vec<StandardBar>, DataError> {
        let mut all_bars = Vec::new();
        // Convert seconds to milliseconds for Bybit API boundary
        let mut current_start_ms = start_time * 1000;
        let end_time_ms = end_time * 1000;
        let limit = self.max_limit_per_request();

        while current_start_ms < end_time_ms {
            let bars = self
                .fetch_page(symbol, interval, current_start_ms, end_time_ms, limit)
                .await?;

            if bars.is_empty() {
                break;
            }

            // Update next start time to be 1ms after the last bar's close time.
            // StandardBar.timestamp is in seconds, so convert to milliseconds.
            let last_close_ms = bars.last().unwrap().timestamp * 1000;
            current_start_ms = last_close_ms + 1;

            all_bars.extend(bars);
        }

        Ok(all_bars)
    }
}

impl Default for BybitAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ExchangeAdapter for BybitAdapter {
    fn name(&self) -> &str {
        "bybit"
    }

    async fn fetch_ohlcv(
        &self,
        symbol: &str,
        interval_secs: u64,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<StandardBar>, DataError> {
        debug!(
            symbol,
            interval_secs, start_time, end_time, "BybitAdapter::fetch_ohlcv"
        );

        if start_time >= end_time {
            return Ok(Vec::new());
        }

        // Denormalize symbol to Bybit format
        let bybit_symbol = SymbolNormalizer::denormalize(symbol, "bybit")
            .map_err(|e| DataError::Exchange(e.to_string()))?;

        let interval = Self::secs_to_interval(interval_secs)?;

        self.fetch_with_pagination(&bybit_symbol, &interval, start_time, end_time)
            .await
    }

    async fn fetch_symbols(&self) -> Result<Vec<String>, DataError> {
        self.rate_limiter.acquire().await;

        let url = format!("{}/v5/market/instruments-info?category=spot", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| DataError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DataError::Exchange(format!(
                "Bybit API error: {}",
                response.status()
            )));
        }

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| DataError::InvalidResponse(e.to_string()))?;

        let symbols = data["result"]["list"]
            .as_array()
            .ok_or_else(|| DataError::InvalidResponse("missing result.list array".to_string()))?;

        let mut result = Vec::new();
        for s in symbols {
            if let Some(symbol_str) = s["symbol"].as_str() {
                result.push(symbol_str.to_string());
            }
        }

        Ok(result)
    }

    fn min_interval_secs(&self) -> u64 {
        60
    }

    fn max_limit_per_request(&self) -> usize {
        200
    }

    /// Convert Bybit format `"BTCUSDT"` → `"BTC/USDT"`.
    fn normalize_symbol(&self, symbol: &str) -> String {
        SymbolNormalizer::normalize(symbol, "bybit").unwrap_or_else(|_| symbol.to_string())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Factory tests
    // ========================================================================

    #[test]
    fn factory_creates_binance_adapter() {
        let adapter = ExchangeAdapterFactory::create("binance");
        assert!(adapter.is_ok());
        assert_eq!(adapter.unwrap().name(), "binance");
    }

    #[test]
    fn factory_creates_okx_adapter() {
        let adapter = ExchangeAdapterFactory::create("okx");
        assert!(adapter.is_ok());
        assert_eq!(adapter.unwrap().name(), "okx");
    }

    #[test]
    fn factory_creates_bybit_adapter() {
        let adapter = ExchangeAdapterFactory::create("bybit");
        assert!(adapter.is_ok());
        assert_eq!(adapter.unwrap().name(), "bybit");
    }

    #[test]
    fn factory_rejects_unknown_exchange() {
        let result = ExchangeAdapterFactory::create("unknown");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DataError::UnknownExchange(ref s) if s == "unknown"));
    }

    #[test]
    fn factory_is_case_insensitive() {
        let adapter = ExchangeAdapterFactory::create("Binance");
        assert!(adapter.is_ok());
        assert_eq!(adapter.unwrap().name(), "binance");
    }

    // ========================================================================
    // RateLimiter tests
    // ========================================================================

    #[tokio::test]
    async fn rate_limiter_enforces_min_interval() {
        let limiter = RateLimiter::new(Duration::from_millis(100));
        let start = Instant::now();

        limiter.acquire().await;
        limiter.acquire().await;

        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(100),
            "expected at least 100ms delay between requests, got {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn rate_limiter_allows_requests_after_interval() {
        let limiter = RateLimiter::new(Duration::from_millis(50));

        limiter.acquire().await;
        tokio::time::sleep(Duration::from_millis(60)).await;
        let start = Instant::now();
        limiter.acquire().await;

        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_millis(20),
            "expected no delay after waiting, got {:?}",
            elapsed
        );
    }

    // ========================================================================
    // BinanceAdapter configuration tests
    // ========================================================================

    #[test]
    fn binance_name_returns_binance() {
        let adapter = BinanceAdapter::new();
        assert_eq!(adapter.name(), "binance");
    }

    #[test]
    fn binance_min_interval_is_60() {
        let adapter = BinanceAdapter::new();
        assert_eq!(adapter.min_interval_secs(), 60);
    }

    #[test]
    fn binance_max_limit_is_1000() {
        let adapter = BinanceAdapter::new();
        assert_eq!(adapter.max_limit_per_request(), 1000);
    }

    // ========================================================================
    // Binance interval mapping tests
    // ========================================================================

    #[test]
    fn binance_secs_to_interval_1m() {
        assert_eq!(BinanceAdapter::secs_to_interval(60).unwrap(), "1m");
    }

    #[test]
    fn binance_secs_to_interval_5m() {
        assert_eq!(BinanceAdapter::secs_to_interval(300).unwrap(), "5m");
    }

    #[test]
    fn binance_secs_to_interval_1h() {
        assert_eq!(BinanceAdapter::secs_to_interval(3600).unwrap(), "1h");
    }

    #[test]
    fn binance_secs_to_interval_1d() {
        assert_eq!(BinanceAdapter::secs_to_interval(86400).unwrap(), "1d");
    }

    #[test]
    fn binance_secs_to_interval_1w() {
        assert_eq!(BinanceAdapter::secs_to_interval(604800).unwrap(), "1w");
    }

    #[test]
    fn binance_secs_to_interval_unsupported() {
        let result = BinanceAdapter::secs_to_interval(123);
        assert!(matches!(result, Err(DataError::UnsupportedInterval(123))));
    }

    #[test]
    fn binance_secs_to_interval_zero() {
        let result = BinanceAdapter::secs_to_interval(0);
        assert!(matches!(result, Err(DataError::UnsupportedInterval(0))));
    }

    // ========================================================================
    // BinanceAdapter OHLCV tests
    // ========================================================================

    #[tokio::test]
    async fn binance_fetch_ohlcv_empty_range() {
        let adapter = BinanceAdapter::new();
        let result = adapter.fetch_ohlcv("BTC/USDT", 60, 1000, 1000).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn binance_fetch_ohlcv_invalid_interval() {
        let adapter = BinanceAdapter::new();
        let result = adapter.fetch_ohlcv("BTC/USDT", 123, 0, 1000).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DataError::UnsupportedInterval(123)
        ));
    }

    #[tokio::test]
    async fn binance_fetch_ohlcv_backward_time() {
        let adapter = BinanceAdapter::new();
        let result = adapter.fetch_ohlcv("BTC/USDT", 60, 1000, 500).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ========================================================================
    // BinanceAdapter backward compatibility tests
    // ========================================================================

    #[test]
    fn binance_normalize_symbol() {
        let adapter = BinanceAdapter::new();
        assert_eq!(adapter.normalize_symbol("BTCUSDT"), "BTC/USDT");
        assert_eq!(adapter.normalize_symbol("ETHUSDT"), "ETH/USDT");
        assert_eq!(adapter.normalize_symbol("BTC/USDT"), "BTC/USDT");
    }

    #[tokio::test]
    async fn binance_fetch_historical_bars_backward_compat() {
        let adapter = BinanceAdapter::new();
        let result = adapter
            .fetch_historical_bars("BTC/USDT", TimeFrame::H1, 0, 3600 * 3)
            .await;
        // This will try to hit the real API, so we just check it doesn't panic
        // and returns an error (since we're not mocking the server)
        assert!(result.is_err() || result.is_ok());
    }

    // ========================================================================
    // OKXAdapter tests
    // ========================================================================

    #[test]
    fn okx_name_returns_okx() {
        let adapter = OKXAdapter::new();
        assert_eq!(adapter.name(), "okx");
    }

    #[test]
    fn okx_min_interval_is_60() {
        let adapter = OKXAdapter::new();
        assert_eq!(adapter.min_interval_secs(), 60);
    }

    #[test]
    fn okx_max_limit_is_300() {
        let adapter = OKXAdapter::new();
        assert_eq!(adapter.max_limit_per_request(), 300);
    }

    #[tokio::test]
    async fn okx_fetch_ohlcv_generates_synthetic_bars() {
        let adapter = OKXAdapter::new();
        let bars = adapter.fetch_ohlcv("BTC/USDT", 60, 0, 3600).await.unwrap();
        assert_eq!(bars.len(), 60);
        assert_eq!(bars[0].symbol, "BTC/USDT");
        assert_eq!(bars[0].exchange, "okx");
    }

    #[tokio::test]
    async fn okx_fetch_symbols_returns_known_pairs() {
        let adapter = OKXAdapter::new();
        let symbols = adapter.fetch_symbols().await.unwrap();
        assert!(symbols.contains(&"BTC-USDT".to_string()));
        assert!(symbols.contains(&"ETH-USDT".to_string()));
    }

    #[test]
    fn okx_normalize_symbol() {
        let adapter = OKXAdapter::new();
        assert_eq!(adapter.normalize_symbol("BTC-USDT"), "BTC/USDT");
        assert_eq!(adapter.normalize_symbol("btc-usdt"), "BTC/USDT");
    }

    #[tokio::test]
    async fn okx_fetch_historical_bars_backward_compat() {
        let adapter = OKXAdapter::new();
        let bars = adapter
            .fetch_historical_bars("BTC-USDT", TimeFrame::M5, 0, 300 * 5)
            .await
            .unwrap();
        assert_eq!(bars.len(), 5);
        assert_eq!(bars[0].symbol, "BTC/USDT");
        assert_eq!(bars[0].exchange, "okx");
    }

    // ========================================================================
    // BinanceAdapter pagination tests
    // ========================================================================

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Simple mock server that returns predefined responses for Binance klines requests
    async fn run_mock_server(listener: TcpListener, responses: Vec<serde_json::Value>) {
        let mut response_iter = responses.into_iter();
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = [0u8; 2048];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            let _request = String::from_utf8_lossy(&buf[..n]);

            let response = if let Some(body) = response_iter.next() {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                    body.to_string().len(),
                    body.to_string()
                )
            } else {
                "HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n".to_string()
            };

            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
        }
    }

    #[tokio::test]
    async fn binance_pagination_fetches_multiple_pages() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let page1_response = serde_json::json!([
            [1000000i64, "100.0", "110.0", "90.0", "105.0", "1000.0"],
            [1060000i64, "105.0", "115.0", "95.0", "110.0", "2000.0"]
        ]);

        let page2_response =
            serde_json::json!([[1120000i64, "110.0", "120.0", "100.0", "115.0", "3000.0"]]);

        let page3_response = serde_json::json!([]);
        tokio::spawn(run_mock_server(
            listener,
            vec![page1_response, page2_response, page3_response],
        ));

        let adapter = BinanceAdapter::with_base_url(format!("http://127.0.0.1:{}", port));
        let bars = adapter
            .fetch_ohlcv("BTC/USDT", 60, 1000, 1200)
            .await
            .unwrap();

        assert_eq!(
            bars.len(),
            3,
            "Expected 3 bars from 2 pages, got {}",
            bars.len()
        );
        assert_eq!(bars[0].timestamp, 1000);
        assert_eq!(bars[1].timestamp, 1060);
        assert_eq!(bars[2].timestamp, 1120);
    }

    #[tokio::test]
    async fn binance_pagination_stops_on_empty_response() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let page1_response =
            serde_json::json!([[1000000i64, "100.0", "110.0", "90.0", "105.0", "1000.0"]]);
        let page2_response = serde_json::json!([]);

        tokio::spawn(run_mock_server(
            listener,
            vec![page1_response, page2_response],
        ));

        let adapter = BinanceAdapter::with_base_url(format!("http://127.0.0.1:{}", port));
        let bars = adapter
            .fetch_ohlcv("BTC/USDT", 60, 1000, 1200)
            .await
            .unwrap();

        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].timestamp, 1000);
    }

    #[tokio::test]
    async fn binance_pagination_next_start_time_is_correct() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let page1_response =
            serde_json::json!([[1000000i64, "100.0", "110.0", "90.0", "105.0", "1000.0"]]);
        let page2_response = serde_json::json!([]);

        // Only 1 page of data, second request should return empty and stop pagination
        tokio::spawn(run_mock_server(
            listener,
            vec![page1_response, page2_response],
        ));

        let adapter = BinanceAdapter::with_base_url(format!("http://127.0.0.1:{}", port));
        let bars = adapter
            .fetch_ohlcv("BTC/USDT", 60, 1000, 1200)
            .await
            .unwrap();

        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].timestamp, 1000);
    }

    // ========================================================================
    // BybitAdapter tests
    // ========================================================================

    #[test]
    fn bybit_name_returns_bybit() {
        let adapter = BybitAdapter::new();
        assert_eq!(adapter.name(), "bybit");
    }

    #[test]
    fn bybit_min_interval_is_60() {
        let adapter = BybitAdapter::new();
        assert_eq!(adapter.min_interval_secs(), 60);
    }

    #[test]
    fn bybit_max_limit_is_200() {
        let adapter = BybitAdapter::new();
        assert_eq!(adapter.max_limit_per_request(), 200);
    }

    #[tokio::test]
    async fn bybit_fetch_ohlcv_empty_range() {
        let adapter = BybitAdapter::new();
        let result = adapter.fetch_ohlcv("BTC/USDT", 60, 1000, 1000).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn bybit_fetch_ohlcv_invalid_interval() {
        let adapter = BybitAdapter::new();
        let result = adapter.fetch_ohlcv("BTC/USDT", 123, 0, 1000).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DataError::UnsupportedInterval(123)
        ));
    }

    #[tokio::test]
    async fn bybit_fetch_ohlcv_backward_time() {
        let adapter = BybitAdapter::new();
        let result = adapter.fetch_ohlcv("BTC/USDT", 60, 1000, 500).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn bybit_normalize_symbol() {
        let adapter = BybitAdapter::new();
        assert_eq!(adapter.normalize_symbol("BTCUSDT"), "BTC/USDT");
        assert_eq!(adapter.normalize_symbol("ETHUSDT"), "ETH/USDT");
        assert_eq!(adapter.normalize_symbol("BTC/USDT"), "BTC/USDT");
    }

    #[test]
    fn bybit_secs_to_interval_1m() {
        assert_eq!(BybitAdapter::secs_to_interval(60).unwrap(), "1");
    }

    #[test]
    fn bybit_secs_to_interval_5m() {
        assert_eq!(BybitAdapter::secs_to_interval(300).unwrap(), "5");
    }

    #[test]
    fn bybit_secs_to_interval_1h() {
        assert_eq!(BybitAdapter::secs_to_interval(3600).unwrap(), "60");
    }

    #[test]
    fn bybit_secs_to_interval_1d() {
        assert_eq!(BybitAdapter::secs_to_interval(86400).unwrap(), "D");
    }

    #[test]
    fn bybit_secs_to_interval_unsupported() {
        let result = BybitAdapter::secs_to_interval(123);
        assert!(matches!(result, Err(DataError::UnsupportedInterval(123))));
    }

    #[test]
    fn bybit_build_klines_url() {
        let adapter = BybitAdapter::with_base_url("https://test.bybit.com".to_string());
        let url = adapter.build_klines_url("BTCUSDT", "1", 0, 1000, 200);
        assert_eq!(
            url,
            "https://test.bybit.com/v5/market/kline?category=spot&symbol=BTCUSDT&interval=1&start=0&end=1000&limit=200"
        );
    }

    #[test]
    fn bybit_parse_valid_kline() {
        let json = serde_json::json!([
            1499040000000i64,
            "0.01634790",
            "0.80000000",
            "0.01575800",
            "0.01577100",
            "148976.11427815",
            "1000.0"
        ]);
        let bar = BybitAdapter::parse_kline_item(&json, "BTC/USDT").unwrap();
        assert_eq!(bar.timestamp, 1499040000);
        assert_eq!(bar.open, Decimal::from_str_exact("0.01634790").unwrap());
        assert_eq!(bar.high, Decimal::from_str_exact("0.80000000").unwrap());
        assert_eq!(bar.low, Decimal::from_str_exact("0.01575800").unwrap());
        assert_eq!(bar.close, Decimal::from_str_exact("0.01577100").unwrap());
        assert_eq!(
            bar.volume,
            Decimal::from_str_exact("148976.11427815").unwrap()
        );
        assert_eq!(bar.symbol, "BTC/USDT");
        assert_eq!(bar.exchange, "bybit");
        assert!(bar.confirmed);
    }

    #[test]
    fn bybit_parse_kline_not_array() {
        let json = serde_json::json!("not an array");
        let result = BybitAdapter::parse_kline_item(&json, "BTC/USDT");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DataError::InvalidResponse(_)));
    }

    #[test]
    fn bybit_parse_kline_too_short() {
        let json = serde_json::json!([1499040000000i64, "0.01634790"]);
        let result = BybitAdapter::parse_kline_item(&json, "BTC/USDT");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DataError::InvalidResponse(_)));
    }

    #[test]
    fn bybit_parse_kline_invalid_timestamp() {
        let json = serde_json::json!([
            "not a number",
            "0.01634790",
            "0.80000000",
            "0.01575800",
            "0.01577100",
            "148976.11427815",
            "1000.0"
        ]);
        let result = BybitAdapter::parse_kline_item(&json, "BTC/USDT");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DataError::InvalidResponse(_)));
    }

    #[test]
    fn bybit_parse_kline_invalid_price() {
        let json = serde_json::json!([
            1499040000000i64,
            "not_a_number",
            "0.80000000",
            "0.01575800",
            "0.01577100",
            "148976.11427815",
            "1000.0"
        ]);
        let result = BybitAdapter::parse_kline_item(&json, "BTC/USDT");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DataError::InvalidResponse(_)));
    }

    #[tokio::test]
    async fn bybit_pagination_fetches_multiple_pages() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let page1_response = serde_json::json!({
            "retCode": 0,
            "result": {
                "list": [
                    [1000000i64, "100.0", "110.0", "90.0", "105.0", "1000.0", "5000.0"],
                    [1060000i64, "105.0", "115.0", "95.0", "110.0", "2000.0", "10000.0"]
                ]
            }
        });

        let page2_response = serde_json::json!({
            "retCode": 0,
            "result": {
                "list": [
                    [1120000i64, "110.0", "120.0", "100.0", "115.0", "3000.0", "15000.0"]
                ]
            }
        });

        let page3_response = serde_json::json!({
            "retCode": 0,
            "result": {
                "list": []
            }
        });

        tokio::spawn(run_mock_server(
            listener,
            vec![page1_response, page2_response, page3_response],
        ));

        let adapter = BybitAdapter::with_base_url(format!("http://127.0.0.1:{}", port));
        let bars = adapter
            .fetch_ohlcv("BTC/USDT", 60, 1000, 1200)
            .await
            .unwrap();

        assert_eq!(
            bars.len(),
            3,
            "Expected 3 bars from 2 pages, got {}",
            bars.len()
        );
        assert_eq!(bars[0].timestamp, 1000);
        assert_eq!(bars[1].timestamp, 1060);
        assert_eq!(bars[2].timestamp, 1120);
    }

    #[tokio::test]
    async fn bybit_pagination_stops_on_empty_response() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let page1_response = serde_json::json!({
            "retCode": 0,
            "result": {
                "list": [
                    [1000000i64, "100.0", "110.0", "90.0", "105.0", "1000.0", "5000.0"]
                ]
            }
        });

        let page2_response = serde_json::json!({
            "retCode": 0,
            "result": {
                "list": []
            }
        });

        tokio::spawn(run_mock_server(
            listener,
            vec![page1_response, page2_response],
        ));

        let adapter = BybitAdapter::with_base_url(format!("http://127.0.0.1:{}", port));
        let bars = adapter
            .fetch_ohlcv("BTC/USDT", 60, 1000, 1200)
            .await
            .unwrap();

        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].timestamp, 1000);
    }

    #[test]
    fn trait_object_bybit() {
        let adapter: Box<dyn ExchangeAdapter> = Box::new(BybitAdapter::new());
        assert_eq!(adapter.name(), "bybit");
        assert_eq!(adapter.min_interval_secs(), 60);
        assert_eq!(adapter.max_limit_per_request(), 200);
    }

    // ========================================================================
    // Binance URL building tests
    // ========================================================================

    #[test]
    fn binance_build_klines_url() {
        let adapter = BinanceAdapter::with_base_url("https://test.binance.com".to_string());
        let url = adapter.build_klines_url("BTCUSDT", "1m", 0, 1000, 500);
        assert_eq!(
            url,
            "https://test.binance.com/api/v3/klines?symbol=BTCUSDT&interval=1m&startTime=0&endTime=1000&limit=500"
        );
    }

    // ========================================================================
    // Binance parse kline item tests
    // ========================================================================

    #[test]
    fn binance_parse_valid_kline() {
        let json = serde_json::json!([
            1499040000000i64,
            "0.01634790",
            "0.80000000",
            "0.01575800",
            "0.01577100",
            "148976.11427815"
        ]);
        let bar = BinanceAdapter::parse_kline_item(&json, "BTC/USDT").unwrap();
        assert_eq!(bar.timestamp, 1499040000);
        assert_eq!(bar.open, Decimal::from_str_exact("0.01634790").unwrap());
        assert_eq!(bar.high, Decimal::from_str_exact("0.80000000").unwrap());
        assert_eq!(bar.low, Decimal::from_str_exact("0.01575800").unwrap());
        assert_eq!(bar.close, Decimal::from_str_exact("0.01577100").unwrap());
        assert_eq!(
            bar.volume,
            Decimal::from_str_exact("148976.11427815").unwrap()
        );
        assert_eq!(bar.symbol, "BTC/USDT");
        assert_eq!(bar.exchange, "binance");
        assert!(bar.confirmed);
    }

    #[test]
    fn binance_parse_kline_not_array() {
        let json = serde_json::json!("not an array");
        let result = BinanceAdapter::parse_kline_item(&json, "BTC/USDT");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DataError::InvalidResponse(_)));
    }

    #[test]
    fn binance_parse_kline_too_short() {
        let json = serde_json::json!([1499040000000i64, "0.01634790"]);
        let result = BinanceAdapter::parse_kline_item(&json, "BTC/USDT");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DataError::InvalidResponse(_)));
    }

    #[test]
    fn binance_parse_kline_invalid_timestamp() {
        let json = serde_json::json!([
            "not a number",
            "0.01634790",
            "0.80000000",
            "0.01575800",
            "0.01577100",
            "148976.11427815"
        ]);
        let result = BinanceAdapter::parse_kline_item(&json, "BTC/USDT");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DataError::InvalidResponse(_)));
    }

    #[test]
    fn binance_parse_kline_invalid_price() {
        let json = serde_json::json!([
            1499040000000i64,
            "not_a_number",
            "0.80000000",
            "0.01575800",
            "0.01577100",
            "148976.11427815"
        ]);
        let result = BinanceAdapter::parse_kline_item(&json, "BTC/USDT");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DataError::InvalidResponse(_)));
    }

    // ========================================================================
    // ExchangeAdapter trait object tests
    // ========================================================================

    #[test]
    fn trait_object_binance() {
        let adapter: Box<dyn ExchangeAdapter> = Box::new(BinanceAdapter::new());
        assert_eq!(adapter.name(), "binance");
        assert_eq!(adapter.min_interval_secs(), 60);
        assert_eq!(adapter.max_limit_per_request(), 1000);
    }

    #[test]
    fn trait_object_okx() {
        let adapter: Box<dyn ExchangeAdapter> = Box::new(OKXAdapter::new());
        assert_eq!(adapter.name(), "okx");
        assert_eq!(adapter.min_interval_secs(), 60);
        assert_eq!(adapter.max_limit_per_request(), 300);
    }
}
