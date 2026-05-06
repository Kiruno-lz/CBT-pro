#!/usr/bin/env node
/**
 * CBT-Pro Binance 历史K线数据导入脚本
 *
 * 从 Binance REST API 获取指定时间范围的1分钟K线数据，
 * 并导入到 PostgreSQL 的 ohlcv_1m 表中。
 *
 * 使用方法:
 *   DATABASE_URL=postgresql://cbtpro:cbtpro@localhost/cbtpro node scripts/import-binance-data.js
 */

const { Client } = require('pg');

// ============================================================================
// 配置
// ============================================================================

const CONFIG = {
  symbol: 'BTCUSDT',           // Binance 格式
  // 使用项目 SymbolNormalizer 标准格式 BTC/USDT
  dbSymbol: 'BTC/USDT',
  interval: '1m',              // 1分钟K线
  exchange: 'binance',
  startTime: new Date('2025-10-01T00:00:00Z').getTime(),
  endTime: (() => {
    const yesterday = new Date();
    yesterday.setDate(yesterday.getDate() - 1);
    yesterday.setHours(0, 0, 0, 0);
    return yesterday.getTime();
  })(),
  batchSize: 1000,             // Binance 单次请求最大限制
  apiBase: 'https://api.binance.com/api/v3/klines',
};

const DATABASE_URL = process.env.DATABASE_URL || 'postgresql://cbtpro:cbtpro@localhost/cbtpro';

// ============================================================================
// 日志工具
// ============================================================================

function log(level, message) {
  const timestamp = new Date().toISOString();
  const colors = {
    info: '\x1b[36m',    // cyan
    success: '\x1b[32m', // green
    warn: '\x1b[33m',    // yellow
    error: '\x1b[31m',   // red
    reset: '\x1b[0m',
  };
  const color = colors[level] || colors.reset;
  console.log(`${color}[${timestamp}] [${level.toUpperCase()}] ${message}${colors.reset}`);
}

// ============================================================================
// Binance API 调用
// ============================================================================

/**
 * 从 Binance 获取K线数据
 * @param {number} startTime - 开始时间戳（毫秒）
 * @param {number} endTime - 结束时间戳（毫秒）
 * @returns {Promise<Array>} K线数据数组
 */
async function fetchKlines(startTime, endTime) {
  const url = new URL(CONFIG.apiBase);
  url.searchParams.set('symbol', CONFIG.symbol);
  url.searchParams.set('interval', CONFIG.interval);
  url.searchParams.set('startTime', startTime.toString());
  url.searchParams.set('endTime', endTime.toString());
  url.searchParams.set('limit', CONFIG.batchSize.toString());

  log('info', `Fetching: ${url.toString()}`);

  const response = await fetch(url.toString(), {
    headers: {
      'Accept': 'application/json',
    },
  });

  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`Binance API error: ${response.status} ${response.statusText} - ${errorText}`);
  }

  const data = await response.json();

  if (!Array.isArray(data)) {
    throw new Error(`Unexpected response format: ${JSON.stringify(data)}`);
  }

  return data;
}

// ============================================================================
// 数据库操作
// ============================================================================

/**
 * 创建数据库连接
 * @returns {Client}
 */
function createClient() {
  return new Client({
    connectionString: DATABASE_URL,
  });
}

/**
 * 将K线数据批量插入数据库
 * @param {Client} client - PostgreSQL 客户端
 * @param {Array} klines - K线数据数组
 * @returns {Promise<number>} 实际插入的行数
 */
async function insertKlines(client, klines) {
  if (klines.length === 0) {
    return 0;
  }

  let inserted = 0;

  // 使用事务批量插入
  await client.query('BEGIN');

  try {
    for (const kline of klines) {
      // Binance kline 格式:
      // [openTime, open, high, low, close, volume, closeTime, quoteVolume, trades, takerBuyBase, takerBuyQuote, ignore]
      const [
        openTime,
        open,
        high,
        low,
        close,
        volume,
      ] = kline;

      // 时间戳转换为秒（数据库使用 BIGINT 存储秒级时间戳）
      const timestampSec = Math.floor(openTime / 1000);

      const result = await client.query(
        `INSERT INTO ohlcv_1m (symbol, timestamp, open, high, low, close, volume, exchange, confirmed)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (symbol, timestamp, exchange) DO NOTHING`,
        [
          CONFIG.dbSymbol,
          timestampSec,
          open,
          high,
          low,
          close,
          volume,
          CONFIG.exchange,
          true,
        ]
      );

      // rowCount 在 ON CONFLICT DO NOTHING 时：
      // - 插入成功返回 1
      // - 冲突跳过返回 0
      inserted += result.rowCount || 0;
    }

    await client.query('COMMIT');
    return inserted;
  } catch (error) {
    await client.query('ROLLBACK');
    throw error;
  }
}

// ============================================================================
// 主流程
// ============================================================================

async function main() {
  log('info', '========================================');
  log('info', 'CBT-Pro Binance 数据导入工具');
  log('info', '========================================');
  log('info', `Symbol: ${CONFIG.symbol} (DB: ${CONFIG.dbSymbol})`);
  log('info', `Time Range: ${new Date(CONFIG.startTime).toISOString()} ~ ${new Date(CONFIG.endTime).toISOString()}`);
  log('info', `Database: ${DATABASE_URL.replace(/\/\/[^:]+:[^@]+@/, '//***:***@')}`);
  log('info', '');

  // 检查 pg 模块
  try {
    require('pg');
  } catch (error) {
    log('error', '缺少 pg 模块，请安装：');
    log('error', '  npm install pg');
    process.exit(1);
  }

  const client = createClient();

  try {
    // 连接数据库
    log('info', 'Connecting to PostgreSQL...');
    await client.connect();
    log('success', 'Database connected');

    // 验证表存在
    const tableCheck = await client.query(
      "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'ohlcv_1m')"
    );
    if (!tableCheck.rows[0].exists) {
      throw new Error('表 ohlcv_1m 不存在，请先运行数据库迁移');
    }
    log('success', 'Table ohlcv_1m exists');

    // 计算需要获取的数据批次
    const totalDuration = CONFIG.endTime - CONFIG.startTime;
    const batchDuration = CONFIG.batchSize * 60 * 1000; // 1000条 * 60秒 * 1000毫秒
    const totalBatches = Math.ceil(totalDuration / batchDuration);

    log('info', `预计需要获取 ${totalBatches} 批次数据`);
    log('info', '');

    let currentStart = CONFIG.startTime;
    let totalFetched = 0;
    let totalInserted = 0;
    let batchCount = 0;

    while (currentStart < CONFIG.endTime) {
      batchCount++;
      const currentEnd = Math.min(currentStart + batchDuration, CONFIG.endTime);

      log('info', `[批次 ${batchCount}/${totalBatches}] 获取数据: ${new Date(currentStart).toISOString()} ~ ${new Date(currentEnd).toISOString()}`);

      // 获取数据（带重试）
      let klines = null;
      let retries = 3;

      while (retries > 0) {
        try {
          klines = await fetchKlines(currentStart, currentEnd);
          break;
        } catch (error) {
          retries--;
          if (retries === 0) {
            throw error;
          }
          log('warn', `请求失败，${retries} 次重试: ${error.message}`);
          await new Promise(resolve => setTimeout(resolve, 2000));
        }
      }

      if (klines.length === 0) {
        log('warn', '  无数据返回，跳过');
        currentStart = currentEnd;
        continue;
      }

      log('info', `  获取到 ${klines.length} 条K线数据`);

      // 插入数据库
      const inserted = await insertKlines(client, klines);
      totalInserted += inserted;
      totalFetched += klines.length;

      log('success', `  已插入 ${inserted} 条数据 (累计: ${totalInserted})`);

      // 更新下一次请求的起始时间
      // 使用最后一条数据的时间 + 1分钟，避免重复
      const lastKline = klines[klines.length - 1];
      const lastOpenTime = lastKline[0];
      currentStart = lastOpenTime + 60 * 1000;

      // 限速：Binance API 有请求频率限制，添加短暂延迟
      if (currentStart < CONFIG.endTime) {
        await new Promise(resolve => setTimeout(resolve, 200));
      }
    }

    log('info', '');
    log('info', '========================================');
    log('success', '数据导入完成！');
    log('info', `总获取: ${totalFetched} 条`);
    log('info', `总插入: ${totalInserted} 条`);
    log('info', `跳过重复: ${totalFetched - totalInserted} 条`);
    log('info', '========================================');

  } catch (error) {
    log('error', `导入失败: ${error.message}`);
    console.error(error);
    process.exit(1);
  } finally {
    await client.end();
  }
}

// 运行主流程
main();
