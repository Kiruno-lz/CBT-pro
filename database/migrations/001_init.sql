-- CBT-Pro Initial Schema
-- OHLCV 1-minute base table
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
);

-- Aggregated OHLCV table (M5, M15, M30, H1, H4, D1, W1)
CREATE TABLE IF NOT EXISTS ohlcv_aggregated (
    id BIGSERIAL PRIMARY KEY,
    symbol VARCHAR(32) NOT NULL,
    timestamp BIGINT NOT NULL,
    timeframe VARCHAR(8) NOT NULL,
    open NUMERIC(24, 8) NOT NULL,
    high NUMERIC(24, 8) NOT NULL,
    low NUMERIC(24, 8) NOT NULL,
    close NUMERIC(24, 8) NOT NULL,
    volume NUMERIC(24, 8) NOT NULL,
    UNIQUE (symbol, timestamp, timeframe)
);

-- BRIN index for efficient time-range scans on ohlcv_1m
CREATE INDEX IF NOT EXISTS idx_ohlcv_1m_brin
    ON ohlcv_1m USING BRIN (symbol, timestamp)
    WITH (pages_per_range = 128);

-- B-tree index on symbol for ohlcv_1m
CREATE INDEX IF NOT EXISTS idx_ohlcv_1m_symbol
    ON ohlcv_1m (symbol);

-- B-tree index on symbol for ohlcv_aggregated
CREATE INDEX IF NOT EXISTS idx_ohlcv_aggregated_symbol
    ON ohlcv_aggregated (symbol);

-- Composite index for aggregated lookups
CREATE INDEX IF NOT EXISTS idx_ohlcv_aggregated_symbol_tf
    ON ohlcv_aggregated (symbol, timeframe, timestamp);
