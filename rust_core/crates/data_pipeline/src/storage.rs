use crate::{error::DataError, StandardBar};
use arrow::array::{BooleanArray, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use chrono::{Datelike, TimeZone, Utc};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, info};

/// PostgreSQL-backed storage for raw 1-minute OHLCV bars.
///
/// Uses the unified `ohlcv_1m` table (created by migrations).
/// All aggregated data is computed on-the-fly from raw 1m data.
///
/// Expected table schema:
/// ```sql
/// CREATE TABLE ohlcv_1m (
///     id         BIGSERIAL PRIMARY KEY,
///     symbol     VARCHAR(32) NOT NULL,
///     timestamp  BIGINT NOT NULL,
///     open       NUMERIC(24, 8) NOT NULL,
///     high       NUMERIC(24, 8) NOT NULL,
///     low        NUMERIC(24, 8) NOT NULL,
///     close      NUMERIC(24, 8) NOT NULL,
///     volume     NUMERIC(24, 8) NOT NULL,
///     exchange   VARCHAR(32) NOT NULL,
///     confirmed  BOOLEAN NOT NULL DEFAULT true,
///     UNIQUE (symbol, timestamp, exchange)
/// );
/// CREATE INDEX idx_ohlcv_1m_brin ON ohlcv_1m USING BRIN (symbol, timestamp);
/// CREATE INDEX idx_ohlcv_1m_symbol ON ohlcv_1m (symbol);
/// ```
#[derive(Debug)]
pub struct PostgresStorage {
    pool: PgPool,
}

impl PostgresStorage {
    /// Connect to the database and ensure schema exists.
    pub async fn connect(database_url: &str) -> Result<Self, DataError> {
        let pool = sqlx::postgres::PgPool::connect(database_url).await?;
        let storage = Self { pool };
        storage.init_schema().await?;
        Ok(storage)
    }

    /// Initialize database schema if not present.
    async fn init_schema(&self) -> Result<(), DataError> {
        let table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1 FROM information_schema.tables
                WHERE table_schema = 'public' AND table_name = 'ohlcv_1m'
            )",
        )
        .fetch_one(&self.pool)
        .await?;

        if !table_exists {
            info!("ohlcv_1m table not found, creating schema...");

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS ohlcv_1m (
                    id BIGSERIAL PRIMARY KEY,
                    symbol VARCHAR(32) NOT NULL,
                    timestamp BIGINT NOT NULL,
                    open NUMERIC(24, 8) NOT NULL,
                    high NUMERIC(24, 8) NOT NULL,
                    low NUMERIC(24, 8) NOT NULL,
                    close NUMERIC(24, 8) NOT NULL,
                    volume NUMERIC(24, 8) NOT NULL,
                    exchange VARCHAR(32) NOT NULL,
                    confirmed BOOLEAN NOT NULL DEFAULT TRUE,
                    UNIQUE (symbol, timestamp, exchange)
                )
                "#,
            )
            .execute(&self.pool)
            .await?;

            sqlx::query(
                r#"
                CREATE INDEX IF NOT EXISTS idx_ohlcv_1m_brin
                    ON ohlcv_1m USING BRIN (symbol, timestamp)
                    WITH (pages_per_range = 128)
                "#,
            )
            .execute(&self.pool)
            .await?;

            sqlx::query(
                r#"
                CREATE INDEX IF NOT EXISTS idx_ohlcv_1m_symbol
                    ON ohlcv_1m (symbol)
                "#,
            )
            .execute(&self.pool)
            .await?;

            info!("ohlcv_1m schema created successfully");
        } else {
            debug!("ohlcv_1m table already exists, skipping schema init");
        }

        Ok(())
    }

    /// Wrap an existing pool (used by `AggregationEngine`).
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Return a reference to the underlying pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Insert raw 1-minute bars.  Returns the number of rows inserted.
    pub async fn insert_bars(&self, bars: &[StandardBar]) -> Result<u64, DataError> {
        if bars.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut inserted: u64 = 0;

        // Batch insert using a simple loop; for production, consider UNNEST.
        for bar in bars {
            let result = sqlx::query(
                r#"
                INSERT INTO ohlcv_1m (symbol, timestamp, open, high, low, close, volume, exchange, confirmed)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                ON CONFLICT (symbol, timestamp, exchange) DO NOTHING
                "#,
            )
            .bind(&bar.symbol)
            .bind(bar.timestamp)
            .bind(bar.open)
            .bind(bar.high)
            .bind(bar.low)
            .bind(bar.close)
            .bind(bar.volume)
            .bind(&bar.exchange)
            .bind(bar.confirmed)
            .execute(&mut *tx)
            .await?;

            inserted += result.rows_affected();
        }

        tx.commit().await?;
        info!(inserted, "bars inserted into ohlcv_1m");
        Ok(inserted)
    }

    /// Query raw 1-minute bars for a symbol between `start` and `end` (inclusive).
    pub async fn query_bars(
        &self,
        symbol: &str,
        start: i64,
        end: i64,
    ) -> Result<Vec<StandardBar>, DataError> {
        debug!(symbol, start, end, "query_bars");

        let rows = sqlx::query_as::<_, BarRow>(
            r#"
            SELECT symbol, timestamp, open, high, low, close, volume, exchange, confirmed
            FROM ohlcv_1m
            WHERE symbol = $1 AND timestamp >= $2 AND timestamp <= $3
            ORDER BY timestamp ASC
            "#,
        )
        .bind(symbol)
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Return the single latest raw bar for a symbol.
    pub async fn query_latest(&self, symbol: &str) -> Result<Option<StandardBar>, DataError> {
        let row = sqlx::query_as::<_, BarRow>(
            r#"
            SELECT symbol, timestamp, open, high, low, close, volume, exchange, confirmed
            FROM ohlcv_1m
            WHERE symbol = $1
            ORDER BY timestamp DESC
            LIMIT 1
            "#,
        )
        .bind(symbol)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    /// Query continuous data ranges for a symbol from the raw 1-minute table.
    ///
    /// Returns a list of `(start, end)` tuples where each range represents
    /// a continuous block of 1-minute bars (interval = 60 seconds).
    pub async fn query_data_ranges(&self, symbol: &str) -> Result<Vec<(i64, i64)>, DataError> {
        let rows = sqlx::query_as::<_, (i64,)>(
            r#"
            SELECT timestamp
            FROM ohlcv_1m
            WHERE symbol = $1
            ORDER BY timestamp ASC
            "#,
        )
        .bind(symbol)
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let mut ranges = Vec::new();
        let mut range_start = rows[0].0;
        let mut range_end = rows[0].0 + 60;

        for row in rows.iter().skip(1) {
            let ts = row.0;
            if ts == range_end {
                range_end = ts + 60;
            } else {
                ranges.push((range_start, range_end));
                range_start = ts;
                range_end = ts + 60;
            }
        }

        ranges.push((range_start, range_end));
        Ok(ranges)
    }
}

/// Internal row type for `sqlx` mapping.
#[derive(Debug, sqlx::FromRow)]
struct BarRow {
    timestamp: i64,
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
    volume: Decimal,
    symbol: String,
    exchange: String,
    confirmed: bool,
}

impl From<BarRow> for StandardBar {
    fn from(r: BarRow) -> Self {
        Self {
            timestamp: r.timestamp,
            open: r.open,
            high: r.high,
            low: r.low,
            close: r.close,
            volume: r.volume,
            symbol: r.symbol,
            exchange: r.exchange,
            confirmed: r.confirmed,
        }
    }
}

// ---------------------------------------------------------------------------
// DataStorage trait implementation for PostgresStorage
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl crate::fetcher::DataStorage for PostgresStorage {
    async fn query_data_ranges(&self, symbol: &str) -> Result<Vec<(i64, i64)>, DataError> {
        self.query_data_ranges(symbol).await
    }

    async fn insert_bars(&self, bars: &[StandardBar]) -> Result<u64, DataError> {
        self.insert_bars(bars).await
    }

    async fn query_bars(
        &self,
        symbol: &str,
        start: i64,
        end: i64,
    ) -> Result<Vec<StandardBar>, DataError> {
        self.query_bars(symbol, start, end).await
    }
}

// ---------------------------------------------------------------------------
// Parquet Storage
// ---------------------------------------------------------------------------

/// Columnar file storage using the Apache Parquet format.
///
/// Partition path: `data/parquet/{exchange}/{symbol}/{YYYY}/{MM}/{symbol}_{YYYYMMDD}.parquet`
pub struct ParquetStorage {
    base_path: std::path::PathBuf,
}

impl ParquetStorage {
    /// Create a new ParquetStorage rooted at `base_path`.
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: std::path::PathBuf::from(base_path),
        }
    }

    /// Compute the full partition path for a given symbol, exchange, year and month.
    pub fn get_partition_path(
        &self,
        symbol: &str,
        exchange: &str,
        year: i32,
        month: u32,
    ) -> std::path::PathBuf {
        let day = 1; // partition files cover the whole month, filename uses first day
        let date_str = format!("{:04}{:02}{:02}", year, month, day);
        self.base_path
            .join("data")
            .join("parquet")
            .join(exchange)
            .join(symbol)
            .join(year.to_string())
            .join(format!("{:02}", month))
            .join(format!("{}_{}.parquet", symbol, date_str))
    }

    /// Write a slice of bars to the appropriate Parquet partition.
    ///
    /// Bars are grouped by month and each month is written to a separate file.
    pub fn write_bars(&self, bars: &[StandardBar]) -> Result<(), DataError> {
        if bars.is_empty() {
            return Ok(());
        }

        // Group by (symbol, exchange, year, month)
        let mut groups: std::collections::HashMap<(String, String, i32, u32), Vec<&StandardBar>> =
            std::collections::HashMap::new();

        for bar in bars {
            let dt = Utc
                .timestamp_opt(bar.timestamp, 0)
                .single()
                .ok_or_else(|| {
                    DataError::Storage(format!("invalid timestamp {} for bar", bar.timestamp))
                })?;
            let key = (
                bar.symbol.clone(),
                bar.exchange.clone(),
                dt.year(),
                dt.month(),
            );
            groups.entry(key).or_default().push(bar);
        }

        for ((symbol, exchange, year, month), group_bars) in groups {
            let path = self.get_partition_path(&symbol, &exchange, year, month);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Build Arrow arrays
            let timestamps =
                Int64Array::from(group_bars.iter().map(|b| b.timestamp).collect::<Vec<_>>());
            let opens = StringArray::from(
                group_bars
                    .iter()
                    .map(|b| b.open.to_string())
                    .collect::<Vec<_>>(),
            );
            let highs = StringArray::from(
                group_bars
                    .iter()
                    .map(|b| b.high.to_string())
                    .collect::<Vec<_>>(),
            );
            let lows = StringArray::from(
                group_bars
                    .iter()
                    .map(|b| b.low.to_string())
                    .collect::<Vec<_>>(),
            );
            let closes = StringArray::from(
                group_bars
                    .iter()
                    .map(|b| b.close.to_string())
                    .collect::<Vec<_>>(),
            );
            let volumes = StringArray::from(
                group_bars
                    .iter()
                    .map(|b| b.volume.to_string())
                    .collect::<Vec<_>>(),
            );
            let symbols = StringArray::from(
                group_bars
                    .iter()
                    .map(|b| b.symbol.as_str())
                    .collect::<Vec<_>>(),
            );
            let exchanges = StringArray::from(
                group_bars
                    .iter()
                    .map(|b| b.exchange.as_str())
                    .collect::<Vec<_>>(),
            );
            let confirmeds =
                BooleanArray::from(group_bars.iter().map(|b| b.confirmed).collect::<Vec<_>>());

            let schema = Arc::new(Schema::new(vec![
                Field::new("timestamp", DataType::Int64, false),
                Field::new("open", DataType::Utf8, false),
                Field::new("high", DataType::Utf8, false),
                Field::new("low", DataType::Utf8, false),
                Field::new("close", DataType::Utf8, false),
                Field::new("volume", DataType::Utf8, false),
                Field::new("symbol", DataType::Utf8, false),
                Field::new("exchange", DataType::Utf8, false),
                Field::new("confirmed", DataType::Boolean, false),
            ]));

            let batch = RecordBatch::try_new(
                schema,
                vec![
                    Arc::new(timestamps),
                    Arc::new(opens),
                    Arc::new(highs),
                    Arc::new(lows),
                    Arc::new(closes),
                    Arc::new(volumes),
                    Arc::new(symbols),
                    Arc::new(exchanges),
                    Arc::new(confirmeds),
                ],
            )
            .map_err(|e| DataError::Parquet(e.to_string()))?;

            let file = std::fs::File::create(&path)?;
            let mut writer = ArrowWriter::try_new(file, batch.schema(), None)
                .map_err(|e| DataError::Parquet(e.to_string()))?;
            writer
                .write(&batch)
                .map_err(|e| DataError::Parquet(e.to_string()))?;
            writer
                .close()
                .map_err(|e| DataError::Parquet(e.to_string()))?;

            info!(path = %path.display(), rows = group_bars.len(), "parquet partition written");
        }

        Ok(())
    }

    /// Read bars from a Parquet partition.
    pub fn read_bars(
        &self,
        symbol: &str,
        exchange: &str,
        year: i32,
        month: u32,
    ) -> Result<Vec<StandardBar>, DataError> {
        let path = self.get_partition_path(symbol, exchange, year, month);
        if !path.exists() {
            return Err(DataError::NotFound(format!(
                "parquet partition not found: {}",
                path.display()
            )));
        }

        let file = std::fs::File::open(&path)?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)
            .map_err(|e| DataError::Parquet(e.to_string()))?;
        let reader = builder
            .build()
            .map_err(|e| DataError::Parquet(e.to_string()))?;

        let mut bars: Vec<StandardBar> = Vec::new();

        for maybe_batch in reader {
            let batch = maybe_batch.map_err(|e| DataError::Parquet(e.to_string()))?;
            let timestamp_col = batch
                .column(0)
                .as_any()
                .downcast_ref::<Int64Array>()
                .ok_or_else(|| DataError::Parquet("timestamp column type mismatch".into()))?;
            let open_col = batch
                .column(1)
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| DataError::Parquet("open column type mismatch".into()))?;
            let high_col = batch
                .column(2)
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| DataError::Parquet("high column type mismatch".into()))?;
            let low_col = batch
                .column(3)
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| DataError::Parquet("low column type mismatch".into()))?;
            let close_col = batch
                .column(4)
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| DataError::Parquet("close column type mismatch".into()))?;
            let volume_col = batch
                .column(5)
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| DataError::Parquet("volume column type mismatch".into()))?;
            let symbol_col = batch
                .column(6)
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| DataError::Parquet("symbol column type mismatch".into()))?;
            let exchange_col = batch
                .column(7)
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| DataError::Parquet("exchange column type mismatch".into()))?;
            let confirmed_col = batch
                .column(8)
                .as_any()
                .downcast_ref::<BooleanArray>()
                .ok_or_else(|| DataError::Parquet("confirmed column type mismatch".into()))?;

            for i in 0..batch.num_rows() {
                bars.push(StandardBar {
                    timestamp: timestamp_col.value(i),
                    open: open_col
                        .value(i)
                        .parse()
                        .map_err(|e| DataError::Parquet(format!("parse open: {}", e)))?,
                    high: high_col
                        .value(i)
                        .parse()
                        .map_err(|e| DataError::Parquet(format!("parse high: {}", e)))?,
                    low: low_col
                        .value(i)
                        .parse()
                        .map_err(|e| DataError::Parquet(format!("parse low: {}", e)))?,
                    close: close_col
                        .value(i)
                        .parse()
                        .map_err(|e| DataError::Parquet(format!("parse close: {}", e)))?,
                    volume: volume_col
                        .value(i)
                        .parse()
                        .map_err(|e| DataError::Parquet(format!("parse volume: {}", e)))?,
                    symbol: symbol_col.value(i).to_string(),
                    exchange: exchange_col.value(i).to_string(),
                    confirmed: confirmed_col.value(i),
                });
            }
        }

        info!(rows = bars.len(), path = %path.display(), "parquet partition read");
        Ok(bars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
    fn test_parquet_roundtrip() {
        let dir = tempdir().unwrap();
        let storage = ParquetStorage::new(dir.path().to_str().unwrap());

        let bars = vec![
            make_bar(
                1704067200,
                "42000.50",
                "42100.00",
                "41900.00",
                "42050.00",
                "123.45678901",
            ),
            make_bar(
                1704067260,
                "42050.00",
                "42200.00",
                "42000.00",
                "42150.00",
                "200.00000000",
            ),
            make_bar(
                1704067320,
                "42150.00",
                "42300.00",
                "42100.00",
                "42250.00",
                "50.12345678",
            ),
        ];

        storage.write_bars(&bars).unwrap();

        // All three bars fall in January 2024
        let read = storage.read_bars("BTC-USDT", "binance", 2024, 1).unwrap();
        assert_eq!(read.len(), 3);
        for (expected, actual) in bars.iter().zip(read.iter()) {
            assert_eq!(expected.timestamp, actual.timestamp);
            assert_eq!(expected.open, actual.open);
            assert_eq!(expected.high, actual.high);
            assert_eq!(expected.low, actual.low);
            assert_eq!(expected.close, actual.close);
            assert_eq!(expected.volume, actual.volume);
            assert_eq!(expected.symbol, actual.symbol);
            assert_eq!(expected.exchange, actual.exchange);
            assert_eq!(expected.confirmed, actual.confirmed);
        }
    }

    #[test]
    fn test_parquet_partition_path() {
        let storage = ParquetStorage::new("/tmp/cbt");
        let path = storage.get_partition_path("BTC-USDT", "binance", 2024, 1);
        assert_eq!(
            path,
            std::path::PathBuf::from(
                "/tmp/cbt/data/parquet/binance/BTC-USDT/2024/01/BTC-USDT_20240101.parquet"
            )
        );
    }

    #[test]
    fn test_parquet_empty_write() {
        let dir = tempdir().unwrap();
        let storage = ParquetStorage::new(dir.path().to_str().unwrap());
        storage.write_bars(&[]).unwrap();
        // No file should be created
        let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parquet_multiple_months() {
        let dir = tempdir().unwrap();
        let storage = ParquetStorage::new(dir.path().to_str().unwrap());

        // January 2024
        let jan_bars = vec![
            make_bar(
                1704067200, "42000.00", "42100.00", "41900.00", "42050.00", "100.0",
            ),
            make_bar(
                1704067260, "42050.00", "42200.00", "42000.00", "42150.00", "200.0",
            ),
        ];
        // February 2024
        let feb_bars = vec![make_bar(
            1706749200, "43000.00", "43100.00", "42900.00", "43050.00", "300.0",
        )];

        let mut all_bars = jan_bars.clone();
        all_bars.extend(feb_bars.clone());
        storage.write_bars(&all_bars).unwrap();

        let read_jan = storage.read_bars("BTC-USDT", "binance", 2024, 1).unwrap();
        assert_eq!(read_jan.len(), 2);

        let read_feb = storage.read_bars("BTC-USDT", "binance", 2024, 2).unwrap();
        assert_eq!(read_feb.len(), 1);
        assert_eq!(read_feb[0].timestamp, 1706749200);
    }

    #[test]
    fn test_parquet_read_nonexistent_file() {
        let dir = tempdir().unwrap();
        let storage = ParquetStorage::new(dir.path().to_str().unwrap());

        let result = storage.read_bars("BTC-USDT", "binance", 2024, 1);
        assert!(matches!(result, Err(DataError::NotFound(_))));
    }

    #[test]
    fn test_parquet_invalid_timestamp() {
        let dir = tempdir().unwrap();
        let storage = ParquetStorage::new(dir.path().to_str().unwrap());

        let bars = vec![StandardBar {
            timestamp: i64::MAX,
            open: Decimal::from(1),
            high: Decimal::from(2),
            low: Decimal::from(1),
            close: Decimal::from(2),
            volume: Decimal::from(100),
            symbol: "BTC-USDT".to_string(),
            exchange: "binance".to_string(),
            confirmed: true,
        }];

        let result = storage.write_bars(&bars);
        assert!(matches!(result, Err(DataError::Storage(_))));
    }

    // ---------------------------------------------------------------------------
    // PostgresStorage integration tests
    // ---------------------------------------------------------------------------

    const TEST_DATABASE_URL: &str = "postgresql://cbtpro:cbtpro@172.18.0.2:5432/cbtpro";

    async fn setup_test_db() -> Option<PostgresStorage> {
        match PostgresStorage::connect(TEST_DATABASE_URL).await {
            Ok(storage) => {
                // Clean up test data
                let _ = sqlx::query("DELETE FROM ohlcv_1m WHERE symbol LIKE 'TEST-%'")
                    .execute(storage.pool())
                    .await;
                Some(storage)
            }
            Err(_) => None,
        }
    }

    fn make_test_bar(ts: i64, symbol: &str, exchange: &str) -> StandardBar {
        StandardBar {
            timestamp: ts,
            open: Decimal::from(100),
            high: Decimal::from(110),
            low: Decimal::from(90),
            close: Decimal::from(105),
            volume: Decimal::from(1000),
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            confirmed: true,
        }
    }

    #[tokio::test]
    async fn test_postgres_insert_and_query() {
        let Some(storage) = setup_test_db().await else {
            eprintln!("Skipping postgres test: database not available");
            return;
        };

        let symbol = "TEST-BTC/USDT";
        let bars = vec![
            make_test_bar(1704067200, symbol, "binance"),
            make_test_bar(1704067260, symbol, "binance"),
            make_test_bar(1704067320, symbol, "binance"),
        ];

        let inserted = storage.insert_bars(&bars).await.unwrap();
        assert_eq!(inserted, 3);

        let queried = storage
            .query_bars(symbol, 1704067200, 1704067320)
            .await
            .unwrap();
        assert_eq!(queried.len(), 3);
        assert_eq!(queried[0].timestamp, 1704067200);
        assert_eq!(queried[2].timestamp, 1704067320);
    }

    #[tokio::test]
    async fn test_postgres_insert_duplicate_ignored() {
        let Some(storage) = setup_test_db().await else {
            eprintln!("Skipping postgres test: database not available");
            return;
        };

        let symbol = "TEST-ETH/USDT";
        let bars = vec![make_test_bar(1704067200, symbol, "okx")];

        let inserted1 = storage.insert_bars(&bars).await.unwrap();
        assert_eq!(inserted1, 1);

        let inserted2 = storage.insert_bars(&bars).await.unwrap();
        assert_eq!(inserted2, 0);

        let queried = storage
            .query_bars(symbol, 1704067200, 1704067200)
            .await
            .unwrap();
        assert_eq!(queried.len(), 1);
    }

    #[tokio::test]
    async fn test_postgres_query_empty_range() {
        let Some(storage) = setup_test_db().await else {
            eprintln!("Skipping postgres test: database not available");
            return;
        };

        let symbol = "TEST-EMPTY";
        let bars = vec![make_test_bar(1704067200, symbol, "binance")];
        storage.insert_bars(&bars).await.unwrap();

        let queried = storage
            .query_bars(symbol, 1704067400, 1704067500)
            .await
            .unwrap();
        assert!(queried.is_empty());
    }

    #[tokio::test]
    async fn test_postgres_query_latest() {
        let Some(storage) = setup_test_db().await else {
            eprintln!("Skipping postgres test: database not available");
            return;
        };

        let symbol = "TEST-LATEST";
        let bars = vec![
            make_test_bar(1704067200, symbol, "binance"),
            make_test_bar(1704067260, symbol, "binance"),
            make_test_bar(1704067320, symbol, "binance"),
        ];

        storage.insert_bars(&bars).await.unwrap();

        let latest = storage.query_latest(symbol).await.unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().timestamp, 1704067320);
    }

    #[tokio::test]
    async fn test_postgres_query_latest_nonexistent() {
        let Some(storage) = setup_test_db().await else {
            eprintln!("Skipping postgres test: database not available");
            return;
        };

        let latest = storage.query_latest("TEST-NONEXISTENT").await.unwrap();
        assert!(latest.is_none());
    }

    #[tokio::test]
    async fn test_postgres_query_data_ranges() {
        let Some(storage) = setup_test_db().await else {
            eprintln!("Skipping postgres test: database not available");
            return;
        };

        let symbol = "TEST-RANGES";
        let bars = vec![
            make_test_bar(1704067200, symbol, "binance"),
            make_test_bar(1704067260, symbol, "binance"),
            make_test_bar(1704067320, symbol, "binance"),
            make_test_bar(1704067500, symbol, "binance"),
            make_test_bar(1704067560, symbol, "binance"),
        ];

        storage.insert_bars(&bars).await.unwrap();

        let ranges = storage.query_data_ranges(symbol).await.unwrap();
        assert_eq!(ranges.len(), 2);
        // First range: 1704067200 to 1704067380 (3 bars)
        assert_eq!(ranges[0], (1704067200, 1704067380));
        // Second range: 1704067500 to 1704067620 (2 bars)
        assert_eq!(ranges[1], (1704067500, 1704067620));
    }

    #[tokio::test]
    async fn test_postgres_query_data_ranges_empty() {
        let Some(storage) = setup_test_db().await else {
            eprintln!("Skipping postgres test: database not available");
            return;
        };

        let ranges = storage.query_data_ranges("TEST-NO-RANGES").await.unwrap();
        assert!(ranges.is_empty());
    }

    #[tokio::test]
    async fn test_postgres_insert_empty() {
        let Some(storage) = setup_test_db().await else {
            eprintln!("Skipping postgres test: database not available");
            return;
        };

        let inserted = storage.insert_bars(&[]).await.unwrap();
        assert_eq!(inserted, 0);
    }

    #[tokio::test]
    async fn test_postgres_query_nonexistent_symbol() {
        let Some(storage) = setup_test_db().await else {
            eprintln!("Skipping postgres test: database not available");
            return;
        };

        let queried = storage.query_bars("TEST-NO-SYMBOL", 0, 1000).await.unwrap();
        assert!(queried.is_empty());
    }

    #[tokio::test]
    async fn test_postgres_data_storage_trait() {
        let Some(storage) = setup_test_db().await else {
            eprintln!("Skipping postgres test: database not available");
            return;
        };

        let symbol = "TEST-TRAIT";
        let bars = vec![
            make_test_bar(1704067200, symbol, "binance"),
            make_test_bar(1704067260, symbol, "binance"),
        ];

        let inserted = storage.insert_bars(&bars).await.unwrap();
        assert_eq!(inserted, 2);

        let queried = storage
            .query_bars(symbol, 1704067200, 1704067260)
            .await
            .unwrap();
        assert_eq!(queried.len(), 2);

        let ranges = storage.query_data_ranges(symbol).await.unwrap();
        assert_eq!(ranges.len(), 1);
    }

    #[tokio::test]
    async fn test_postgres_multiple_exchanges_same_symbol() {
        let Some(storage) = setup_test_db().await else {
            eprintln!("Skipping postgres test: database not available");
            return;
        };

        let symbol = "TEST-MULTI-EX";
        let bars = vec![
            make_test_bar(1704067200, symbol, "binance"),
            make_test_bar(1704067200, symbol, "okx"),
        ];

        let inserted = storage.insert_bars(&bars).await.unwrap();
        assert_eq!(inserted, 2);

        let queried = storage
            .query_bars(symbol, 1704067200, 1704067200)
            .await
            .unwrap();
        assert_eq!(queried.len(), 2);
    }

    #[tokio::test]
    async fn test_postgres_large_batch_insert() {
        let Some(storage) = setup_test_db().await else {
            eprintln!("Skipping postgres test: database not available");
            return;
        };

        let symbol = "TEST-LARGE";
        let bars: Vec<StandardBar> = (0..100)
            .map(|i| make_test_bar(1704067200 + i * 60, symbol, "binance"))
            .collect();

        let inserted = storage.insert_bars(&bars).await.unwrap();
        assert_eq!(inserted, 100);

        let queried = storage
            .query_bars(symbol, 1704067200, 1704067200 + 99 * 60)
            .await
            .unwrap();
        assert_eq!(queried.len(), 100);
    }
}
