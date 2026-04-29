use crate::ema::ema;
use crate::{IndicatorError, IndicatorResult};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq)]
pub struct MacdResult {
    pub macd: Decimal,
    pub signal: Decimal,
    pub histogram: Decimal,
}

pub fn macd(
    fast: usize,
    slow: usize,
    signal: usize,
    prices: &[Decimal],
) -> Result<Vec<(IndicatorResult, MacdResult)>, IndicatorError> {
    if fast >= slow {
        return Err(IndicatorError::InvalidParameter(
            "fast must be < slow".to_string(),
        ));
    }
    if fast == 0 || slow == 0 || signal == 0 {
        return Err(IndicatorError::InvalidParameter(
            "periods must be > 0".to_string(),
        ));
    }

    let fast_ema = ema(fast, prices)?;
    let slow_ema = ema(slow, prices)?;

    // MACD Line = EMA(fast) - EMA(slow)
    // fast_ema starts at index fast-1, slow_ema at index slow-1
    // We align them starting from index slow-1
    let mut macd_line = Vec::new();
    for i in (slow - 1)..prices.len() {
        let fast_val = fast_ema[i - (fast - 1)].value;
        let slow_val = slow_ema[i - (slow - 1)].value;
        macd_line.push(IndicatorResult {
            value: fast_val - slow_val,
            timestamp: i as i64,
        });
    }

    // Signal Line = EMA(signal) of MACD Line values
    let macd_values: Vec<Decimal> = macd_line.iter().map(|r| r.value).collect();
    let signal_ema = ema(signal, &macd_values)?;

    // Align signal with macd_line
    let mut result = Vec::new();
    for i in (signal - 1)..macd_line.len() {
        let macd_val = macd_line[i].value;
        let signal_val = signal_ema[i - (signal - 1)].value;
        let histogram = macd_val - signal_val;

        result.push((
            IndicatorResult {
                value: macd_val,
                timestamp: macd_line[i].timestamp,
            },
            MacdResult {
                macd: macd_val,
                signal: signal_val,
                histogram,
            },
        ));
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macd_basic() {
        let prices = vec![
            Decimal::from(10),
            Decimal::from(11),
            Decimal::from(12),
            Decimal::from(11),
            Decimal::from(13),
            Decimal::from(15),
            Decimal::from(14),
            Decimal::from(16),
            Decimal::from(18),
            Decimal::from(17),
        ];

        // fast=3, slow=5, signal=2
        let result = macd(3, 5, 2, &prices).unwrap();

        // Should have results starting from index slow-1 + signal-1 = 4 + 1 = 5
        // prices.len() = 10, slow=5, so macd_line has 10-4 = 6 elements (indices 4..9)
        // signal=2, so signal_ema has 6-1 = 5 elements starting from macd_line index 1
        // result has 5 elements
        assert!(!result.is_empty());

        // Check that all MACD values are present
        for (indicator, macd_result) in &result {
            assert_eq!(indicator.value, macd_result.macd);
        }
    }

    #[test]
    fn test_macd_invalid_params() {
        let prices = vec![Decimal::from(10); 10];

        // fast >= slow
        let result = macd(5, 3, 2, &prices);
        assert!(result.is_err());

        // zero period
        let result = macd(0, 3, 2, &prices);
        assert!(result.is_err());

        let result = macd(3, 0, 2, &prices);
        assert!(result.is_err());

        let result = macd(3, 5, 0, &prices);
        assert!(result.is_err());
    }

    #[test]
    fn test_macd_insufficient_data() {
        let prices = vec![Decimal::from(10); 5];
        let result = macd(12, 26, 9, &prices);
        assert!(result.is_err());
    }

    #[test]
    fn test_macd_values() {
        let prices = vec![
            Decimal::from(10),
            Decimal::from(11),
            Decimal::from(12),
            Decimal::from(11),
            Decimal::from(13),
            Decimal::from(15),
            Decimal::from(14),
            Decimal::from(16),
            Decimal::from(18),
            Decimal::from(17),
        ];

        let result = macd(3, 5, 2, &prices).unwrap();

        // Verify histogram = macd - signal
        for (_, macd_result) in &result {
            let expected_histogram = macd_result.macd - macd_result.signal;
            assert_eq!(macd_result.histogram, expected_histogram);
        }
    }
}
