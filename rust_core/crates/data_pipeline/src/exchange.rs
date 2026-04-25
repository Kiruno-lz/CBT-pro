use crate::{error::DataError, StandardBar, TimeFrame};
use rust_decimal::Decimal;
use tracing::{debug, info};

/// Abstraction over a cryptocurrency exchange data feed.
#[async_trait::async_trait]
pub trait ExchangeAdapter: Send + Sync {
    /// Human-readable exchange name.
    fn name(&self) -> &str;

    /// Fetch historical bars from the exchange REST API.
    async fn fetch_historical_bars(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        start: i64,
        end: i64,
    ) -> Result<Vec<StandardBar>, DataError>;

    /// Normalise the exchange-specific symbol format to the CBT-Pro standard.
    fn normalize_symbol(&self, symbol: &str) -> String;
}

// ---------------------------------------------------------------------------
// Binance Adapter (stub — returns synthetic data for offline testing)
// ---------------------------------------------------------------------------

/// Stub adapter that mimics Binance REST behaviour without hitting the network.
pub struct BinanceAdapter;

#[async_trait::async_trait]
impl ExchangeAdapter for BinanceAdapter {
    fn name(&self) -> &str {
        "binance"
    }

    async fn fetch_historical_bars(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        start: i64,
        end: i64,
    ) -> Result<Vec<StandardBar>, DataError> {
        debug!(symbol, ?timeframe, start, end, "BinanceAdapter::fetch_historical_bars");

        let tf_secs = timeframe.as_seconds();
        let count = ((end - start) / tf_secs).max(0) as usize;
        if count == 0 {
            return Ok(Vec::new());
        }

        let base_price = Decimal::from(42000);
        let mut bars: Vec<StandardBar> = Vec::with_capacity(count);
        let normalised = self.normalize_symbol(symbol);

        for i in 0..count {
            let ts = start + (i as i64) * tf_secs;
            let delta = Decimal::from(i as i64 % 100);
            let open = base_price + delta;
            let close = open + Decimal::from(1);
            let high = close + Decimal::from(5);
            let low = open - Decimal::from(5);
            let volume = Decimal::from(100);

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
        }

        info!(count = bars.len(), "BinanceAdapter generated synthetic bars");
        Ok(bars)
    }

    /// Convert Binance format `"BTCUSDT"` → `"BTC-USDT"`.
    fn normalize_symbol(&self, symbol: &str) -> String {
        // Simple heuristic: insert a dash before the last 4 characters (usually USDT)
        if symbol.len() > 4 && !symbol.contains('-') {
            let split = symbol.len() - 4;
            format!("{}-{}", &symbol[..split], &symbol[split..])
        } else {
            symbol.to_string()
        }
    }
}

// ---------------------------------------------------------------------------
// OKX Adapter (stub)
// ---------------------------------------------------------------------------

/// Stub adapter that mimics OKX REST behaviour without hitting the network.
pub struct OkxAdapter;

#[async_trait::async_trait]
impl ExchangeAdapter for OkxAdapter {
    fn name(&self) -> &str {
        "okx"
    }

    async fn fetch_historical_bars(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        start: i64,
        end: i64,
    ) -> Result<Vec<StandardBar>, DataError> {
        debug!(symbol, ?timeframe, start, end, "OkxAdapter::fetch_historical_bars");

        let tf_secs = timeframe.as_seconds();
        let count = ((end - start) / tf_secs).max(0) as usize;
        if count == 0 {
            return Ok(Vec::new());
        }

        let base_price = Decimal::from(42000);
        let mut bars: Vec<StandardBar> = Vec::with_capacity(count);
        let normalised = self.normalize_symbol(symbol);

        for i in 0..count {
            let ts = start + (i as i64) * tf_secs;
            let delta = Decimal::from(i as i64 % 100);
            let open = base_price + delta;
            let close = open + Decimal::from(1);
            let high = close + Decimal::from(5);
            let low = open - Decimal::from(5);
            let volume = Decimal::from(100);

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
        }

        info!(count = bars.len(), "OkxAdapter generated synthetic bars");
        Ok(bars)
    }

    /// OKX already uses `"BTC-USDT"`, so pass-through with upper-casing.
    fn normalize_symbol(&self, symbol: &str) -> String {
        symbol.to_ascii_uppercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binance_normalize() {
        let adapter = BinanceAdapter;
        assert_eq!(adapter.normalize_symbol("BTCUSDT"), "BTC-USDT");
        assert_eq!(adapter.normalize_symbol("ETHUSDT"), "ETH-USDT");
        assert_eq!(adapter.normalize_symbol("BTC-USDT"), "BTC-USDT");
    }

    #[test]
    fn test_okx_normalize() {
        let adapter = OkxAdapter;
        assert_eq!(adapter.normalize_symbol("BTC-USDT"), "BTC-USDT");
        assert_eq!(adapter.normalize_symbol("btc-usdt"), "BTC-USDT");
    }

    #[tokio::test]
    async fn test_binance_fetch() {
        let adapter = BinanceAdapter;
        let bars = adapter
            .fetch_historical_bars("BTCUSDT", TimeFrame::H1, 0, 3600 * 3)
            .await
            .unwrap();
        assert_eq!(bars.len(), 3);
        assert_eq!(bars[0].symbol, "BTC-USDT");
        assert_eq!(bars[0].exchange, "binance");
        assert_eq!(bars[0].timestamp, 0);
        assert_eq!(bars[1].timestamp, 3600);
    }

    #[tokio::test]
    async fn test_okx_fetch() {
        let adapter = OkxAdapter;
        let bars = adapter
            .fetch_historical_bars("BTC-USDT", TimeFrame::M5, 0, 300 * 5)
            .await
            .unwrap();
        assert_eq!(bars.len(), 5);
        assert_eq!(bars[0].symbol, "BTC-USDT");
        assert_eq!(bars[0].exchange, "okx");
    }
}
