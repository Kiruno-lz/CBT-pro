use crate::{error::DataError, StandardBar, TimeFrame};
use arrow::array::{
    BooleanArray, Int64Array, StringArray,
};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use chrono::{Datelike, TimeZone, Utc};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, info};

/// PostgreSQL-backed storage for raw and aggregated bars.
///
/// Expected raw table (created by migrations):
/// ```sql
/// CREATE TABLE bars_1m (
///     timestamp  BIGINT NOT NULL,
///     open       NUMERIC NOT NULL,
///     high       NUMERIC NOT NULL,
///     low        NUMERIC NOT NULL,
///     close      NUMERIC NOT NULL,
///     volume     NUMERIC NOT NULL,
///     symbol     TEXT NOT NULL,
///     exchange   TEXT NOT NULL,
///     confirmed  BOOLEAN NOT NULL DEFAULT true,
///     PRIMARY KEY (symbol, timestamp)
/// );
/// CREATE INDEX idx_bars_1m_symbol_ts ON bars_1m(symbol, timestamp);
/// ```
pub struct PostgresStorage {
    pool: PgPool,
}

impl PostgresStorage {
    /// Connect to the database.
    pub async fn connect(database_url: &str) -> Result<Self, DataError> {
        let pool = sqlx::postgres::PgPool::connect(database_url).await?;
        Ok(Self { pool })
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
                INSERT INTO bars_1m (timestamp, open, high, low, close, volume, symbol, exchange, confirmed)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                ON CONFLICT (symbol, timestamp) DO NOTHING
                "#,
            )
            .bind(bar.timestamp)
            .bind(bar.open)
            .bind(bar.high)
            .bind(bar.low)
            .bind(bar.close)
            .bind(bar.volume)
            .bind(&bar.symbol)
            .bind(&bar.exchange)
            .bind(bar.confirmed)
            .execute(&mut *tx)
            .await?;

            inserted += result.rows_affected();
        }

        tx.commit().await?;
        info!(inserted, "bars inserted into bars_1m");
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
            SELECT timestamp, open, high, low, close, volume, symbol, exchange, confirmed
            FROM bars_1m
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
            SELECT timestamp, open, high, low, close, volume, symbol, exchange, confirmed
            FROM bars_1m
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

    /// Insert aggregated bars into the per-timeframe cache table.
    pub async fn insert_aggregated(
        &self,
        _symbol: &str,
        timeframe: TimeFrame,
        bars: &[StandardBar],
    ) -> Result<u64, DataError> {
        if bars.is_empty() {
            return Ok(0);
        }

        let table = format!("bars_{:?}", timeframe).to_lowercase();
        let mut tx = self.pool.begin().await?;
        let mut inserted: u64 = 0;

        for bar in bars {
            let result = sqlx::query(&format!(
                r#"
                INSERT INTO {} (timestamp, open, high, low, close, volume, symbol, exchange, confirmed)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                ON CONFLICT (symbol, timestamp) DO NOTHING
                "#,
                table
            ))
            .bind(bar.timestamp)
            .bind(bar.open)
            .bind(bar.high)
            .bind(bar.low)
            .bind(bar.close)
            .bind(bar.volume)
            .bind(&bar.symbol)
            .bind(&bar.exchange)
            .bind(bar.confirmed)
            .execute(&mut *tx)
            .await?;

            inserted += result.rows_affected();
        }

        tx.commit().await?;
        info!(inserted, table, "aggregated bars inserted");
        Ok(inserted)
    }

    /// Query the aggregated cache table for a given timeframe.
    pub async fn query_aggregated(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
        start: i64,
        end: i64,
    ) -> Result<Vec<StandardBar>, DataError> {
        let table = format!("bars_{:?}", timeframe).to_lowercase();
        debug!(symbol, ?timeframe, start, end, table, "query_aggregated");

        let rows = sqlx::query_as::<_, BarRow>(&format!(
            r#"
            SELECT timestamp, open, high, low, close, volume, symbol, exchange, confirmed
            FROM {}
            WHERE symbol = $1 AND timestamp >= $2 AND timestamp <= $3
            ORDER BY timestamp ASC
            "#,
            table
        ))
        .bind(symbol)
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Return the latest aggregated bar for a symbol / timeframe.
    pub async fn query_latest_aggregated(
        &self,
        symbol: &str,
        timeframe: TimeFrame,
    ) -> Result<Option<StandardBar>, DataError> {
        let table = format!("bars_{:?}", timeframe).to_lowercase();

        let row = sqlx::query_as::<_, BarRow>(&format!(
            r#"
            SELECT timestamp, open, high, low, close, volume, symbol, exchange, confirmed
            FROM {}
            WHERE symbol = $1
            ORDER BY timestamp DESC
            LIMIT 1
            "#,
            table
        ))
        .bind(symbol)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
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
            let dt = Utc.timestamp_opt(bar.timestamp, 0).single().ok_or_else(|| {
                DataError::Storage(format!(
                    "invalid timestamp {} for bar",
                    bar.timestamp
                ))
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
            let timestamps = Int64Array::from(
                group_bars.iter().map(|b| b.timestamp).collect::<Vec<_>>(),
            );
            let opens = StringArray::from(
                group_bars.iter().map(|b| b.open.to_string()).collect::<Vec<_>>(),
            );
            let highs = StringArray::from(
                group_bars.iter().map(|b| b.high.to_string()).collect::<Vec<_>>(),
            );
            let lows = StringArray::from(
                group_bars.iter().map(|b| b.low.to_string()).collect::<Vec<_>>(),
            );
            let closes = StringArray::from(
                group_bars.iter().map(|b| b.close.to_string()).collect::<Vec<_>>(),
            );
            let volumes = StringArray::from(
                group_bars.iter().map(|b| b.volume.to_string()).collect::<Vec<_>>(),
            );
            let symbols = StringArray::from(
                group_bars.iter().map(|b| b.symbol.as_str()).collect::<Vec<_>>(),
            );
            let exchanges = StringArray::from(
                group_bars.iter().map(|b| b.exchange.as_str()).collect::<Vec<_>>(),
            );
            let confirmeds = BooleanArray::from(
                group_bars.iter().map(|b| b.confirmed).collect::<Vec<_>>(),
            );

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
            make_bar(1704067200, "42000.50", "42100.00", "41900.00", "42050.00", "123.45678901"),
            make_bar(1704067260, "42050.00", "42200.00", "42000.00", "42150.00", "200.00000000"),
            make_bar(1704067320, "42150.00", "42300.00", "42100.00", "42250.00", "50.12345678"),
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
            std::path::PathBuf::from("/tmp/cbt/data/parquet/binance/BTC-USDT/2024/01/BTC-USDT_20240101.parquet")
        );
    }
}