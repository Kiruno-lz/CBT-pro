use data_pipeline::exchange::{BinanceAdapter, ExchangeAdapter, OkxAdapter};
use data_pipeline::{StandardBar, TimeFrame};
use rust_decimal::Decimal;

// ===========================================================================
// 复制 server.rs 中 generate_synthetic_bars 的当前（有 bug）实现
// ===========================================================================

/// 复制自 crates/api/src/server.rs —— 使用 `i % 5` 生成 close 和 volume，
/// 导致每 5 根 K 线重复一次模式。
fn generate_synthetic_bars_buggy(symbol: &str, count: usize) -> Vec<StandardBar> {
    let mut bars = Vec::with_capacity(count);
    let base = Decimal::from(40000);
    for i in 0..count {
        let open = base + Decimal::from(i as i64 * 10);
        let close = open + Decimal::from((i % 5) as i64 * 5 - 10);
        let high = if close > open {
            close + Decimal::from(20)
        } else {
            open + Decimal::from(20)
        };
        let low = if close < open {
            close - Decimal::from(20)
        } else {
            open - Decimal::from(20)
        };
        bars.push(StandardBar {
            timestamp: 1704067200000 + i as i64 * 60000,
            open,
            high,
            low,
            close,
            volume: Decimal::from(10 + (i % 5) as i64),
            symbol: symbol.to_string(),
            exchange: "binance".to_string(),
            confirmed: true,
        });
    }
    bars
}

// ===========================================================================
// 循环检测辅助函数
// ===========================================================================

/// 检测价格序列中是否存在短周期循环。
///
/// 算法：先计算相邻 close 的**一阶差分**序列，然后检查该差分序列中
/// 是否存在长度为 `cycle_len` 的子序列连续重复至少 `min_repetitions` 次。
/// 差分能消除线性趋势，更容易发现周期性模式。
fn detect_price_cycle(values: &[Decimal], cycle_len: usize, min_repetitions: usize) -> bool {
    if values.len() < cycle_len * min_repetitions + 1 {
        return false;
    }

    // 计算一阶差分
    let diffs: Vec<Decimal> = values.windows(2).map(|w| w[1] - w[0]).collect();

    if diffs.len() < cycle_len * min_repetitions {
        return false;
    }

    for start in 0..=diffs.len().saturating_sub(cycle_len * min_repetitions) {
        let pattern = &diffs[start..start + cycle_len];
        let mut count = 1;

        for rep in 1..min_repetitions {
            let next_start = start + cycle_len * rep;
            let next_end = next_start + cycle_len;
            if &diffs[next_start..next_end] == pattern {
                count += 1;
            } else {
                break;
            }
        }

        if count >= min_repetitions {
            return true;
        }
    }

    false
}

/// 检测 volume 序列中是否存在短周期循环。
///
/// 与价格不同，volume 本身就是绝对值，因此直接检查原始值是否存在
/// 长度为 `cycle_len` 的子序列连续重复至少 `min_repetitions` 次。
fn detect_volume_cycle(volumes: &[Decimal], cycle_len: usize, min_repetitions: usize) -> bool {
    if volumes.len() < cycle_len * min_repetitions {
        return false;
    }

    for start in 0..=volumes.len().saturating_sub(cycle_len * min_repetitions) {
        let pattern = &volumes[start..start + cycle_len];
        let mut count = 1;

        for rep in 1..min_repetitions {
            let next_start = start + cycle_len * rep;
            let next_end = next_start + cycle_len;
            if &volumes[next_start..next_end] == pattern {
                count += 1;
            } else {
                break;
            }
        }

        if count >= min_repetitions {
            return true;
        }
    }

    false
}

/// 验证 OHLCV 关系合理性，返回所有错误信息。
///
/// 要求：
/// - high >= open
/// - high >= close
/// - low <= open
/// - low <= close
/// - high >= low
fn validate_ohlcv(bars: &[StandardBar]) -> Vec<String> {
    let mut errors = Vec::new();

    for (i, bar) in bars.iter().enumerate() {
        if bar.high < bar.open {
            errors.push(format!(
                "Bar {}: high ({}) < open ({})",
                i, bar.high, bar.open
            ));
        }
        if bar.high < bar.close {
            errors.push(format!(
                "Bar {}: high ({}) < close ({})",
                i, bar.high, bar.close
            ));
        }
        if bar.low > bar.open {
            errors.push(format!(
                "Bar {}: low ({}) > open ({})",
                i, bar.low, bar.open
            ));
        }
        if bar.low > bar.close {
            errors.push(format!(
                "Bar {}: low ({}) > close ({})",
                i, bar.low, bar.close
            ));
        }
        if bar.high < bar.low {
            errors.push(format!(
                "Bar {}: high ({}) < low ({})",
                i, bar.high, bar.low
            ));
        }
    }

    errors
}

// ===========================================================================
// 回归测试：验证循环检测器能够发现旧的 buggy 逻辑（i % 5）
// ===========================================================================

#[test]
fn test_generate_synthetic_bars_buggy_detects_cycle() {
    let bars = generate_synthetic_bars_buggy("BTC-USDT", 1000);
    let closes: Vec<Decimal> = bars.iter().map(|b| b.close).collect();
    let volumes: Vec<Decimal> = bars.iter().map(|b| b.volume).collect();

    // 旧的 buggy 逻辑使用 i % 5，因此 close 和 volume 都会呈现 5 周期循环。
    // 本测试验证：当数据中存在循环时，检测器必须能够发现它。
    // 这是一个回归测试——如果未来有人重新引入 i % 5 模式，检测器仍应报警。
    let has_5_cycle = detect_price_cycle(&closes, 5, 3);
    assert!(
        has_5_cycle,
        "回归测试失败：未能检测到旧 buggy 逻辑中的 5 周期价格循环，\
         说明 detect_price_cycle 失效或 buggy 逻辑被意外修改。"
    );

    let has_5_volume_cycle = detect_volume_cycle(&volumes, 5, 3);
    assert!(
        has_5_volume_cycle,
        "回归测试失败：未能检测到旧 buggy 逻辑中的 5 周期 volume 循环，\
         说明 detect_volume_cycle 失效或 buggy 逻辑被意外修改。"
    );
}

#[test]
fn test_generate_synthetic_bars_ohlcv_relationships() {
    let bars = generate_synthetic_bars_buggy("BTC-USDT", 1000);
    let errors = validate_ohlcv(&bars);
    assert!(
        errors.is_empty(),
        "OHLCV 关系验证失败（共 {} 处）:\n{}",
        errors.len(),
        errors.join("\n")
    );
}

// ===========================================================================
// 测试：OkxAdapter（仍使用 i % 100，存在 100 周期循环）
// ===========================================================================

#[tokio::test]
async fn test_okx_adapter_no_short_cycle() {
    let adapter = OkxAdapter;
    let bars = adapter
        .fetch_historical_bars("BTC-USDT", TimeFrame::M1, 0, 60 * 500)
        .await
        .unwrap();

    let closes: Vec<Decimal> = bars.iter().map(|b| b.close).collect();
    let volumes: Vec<Decimal> = bars.iter().map(|b| b.volume).collect();

    // OkxAdapter：检测是否存在 100 周期循环（历史 bug 使用 `i % 100`）。
    // 如果未来重新引入了取模生成价格增量的方式，此测试将失败。
    let has_100_cycle = detect_price_cycle(&closes, 100, 3);
    assert!(
        !has_100_cycle,
        "检测到 100 周期价格循环！OkxAdapter::fetch_historical_bars 可能使用了 `i % 100` \
         计算价格增量，导致每 100 根 K 线重复一次价格模式。"
    );

    // 检测 volume 是否完全重复（历史 bug 中 volume 固定为 100）。
    let has_volume_cycle = detect_volume_cycle(&volumes, 1, 10);
    assert!(
        !has_volume_cycle,
        "检测到 volume 完全重复！OkxAdapter::fetch_historical_bars 的 volume 可能固定不变，\
         属于极端的周期性重复。"
    );
}

#[tokio::test]
async fn test_okx_adapter_ohlcv_relationships() {
    let adapter = OkxAdapter;
    let bars = adapter
        .fetch_historical_bars("BTC-USDT", TimeFrame::M1, 0, 60 * 100)
        .await
        .unwrap();

    let errors = validate_ohlcv(&bars);
    assert!(
        errors.is_empty(),
        "OkxAdapter OHLCV 关系验证失败（共 {} 处）:\n{}",
        errors.len(),
        errors.join("\n")
    );
}

// ===========================================================================
// 测试：BinanceAdapter（已使用 SmallRng，理论上无短周期循环）
// ===========================================================================

#[tokio::test]
async fn test_binance_adapter_no_short_cycle() {
    let adapter = BinanceAdapter;
    let bars = adapter
        .fetch_historical_bars("BTCUSDT", TimeFrame::M1, 0, 60 * 500)
        .await
        .unwrap();

    let closes: Vec<Decimal> = bars.iter().map(|b| b.close).collect();
    let volumes: Vec<Decimal> = bars.iter().map(|b| b.volume).collect();

    // 对 BinanceAdapter 检测多种可能的短周期（5、10、20、50、100）。
    for cycle_len in [5, 10, 20, 50, 100] {
        let has_cycle = detect_price_cycle(&closes, cycle_len, 3);
        assert!(
            !has_cycle,
            "检测到 {} 周期价格循环！BinanceAdapter::fetch_historical_bars 生成的价格数据 \
             存在周期性重复，可能是使用了取模运算生成伪随机数。",
            cycle_len
        );
    }

    for cycle_len in [5, 10, 20, 50, 100] {
        let has_cycle = detect_volume_cycle(&volumes, cycle_len, 3);
        assert!(
            !has_cycle,
            "检测到 {} 周期 volume 循环！BinanceAdapter::fetch_historical_bars 生成的 volume 数据 \
             存在周期性重复。",
            cycle_len
        );
    }
}

#[tokio::test]
async fn test_binance_adapter_ohlcv_relationships() {
    let adapter = BinanceAdapter;
    let bars = adapter
        .fetch_historical_bars("BTCUSDT", TimeFrame::M1, 0, 60 * 100)
        .await
        .unwrap();

    let errors = validate_ohlcv(&bars);
    assert!(
        errors.is_empty(),
        "BinanceAdapter OHLCV 关系验证失败（共 {} 处）:\n{}",
        errors.len(),
        errors.join("\n")
    );
}
