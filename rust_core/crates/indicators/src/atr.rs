use rust_decimal::Decimal;
use crate::{IndicatorResult, IndicatorError};

pub fn atr(
    period: usize,
    highs: &[Decimal],
    lows: &[Decimal],
    closes: &[Decimal],
) -> Result<Vec<IndicatorResult>, IndicatorError> {
    if period == 0 {
        return Err(IndicatorError::InvalidParameter("period must be > 0".to_string()));
    }
    if highs.len() != lows.len() || highs.len() != closes.len() {
        return Err(IndicatorError::InvalidParameter("input arrays must have same length".to_string()));
    }
    if highs.len() <= period {
        return Err(IndicatorError::InsufficientData {
            required: period + 1,
            got: highs.len(),
        });
    }

    let mut tr_values = Vec::new();
    
    for i in 1..highs.len() {
        let tr1 = highs[i] - lows[i];
        let tr2 = (highs[i] - closes[i - 1]).abs();
        let tr3 = (lows[i] - closes[i - 1]).abs();
        
        let tr = tr1.max(tr2).max(tr3);
        tr_values.push(tr);
    }

    let mut result = Vec::new();
    let period_dec = Decimal::from(period as i64);
    
    for i in period..=tr_values.len() {
        let atr_val = tr_values[i - period..i].iter().copied().sum::<Decimal>() / period_dec;
        result.push(IndicatorResult {
            value: atr_val,
            timestamp: i as i64,
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atr_basic() {
        let highs = vec![
            Decimal::from(12),
            Decimal::from(13),
            Decimal::from(11),
            Decimal::from(14),
        ];
        let lows = vec![
            Decimal::from(10),
            Decimal::from(11),
            Decimal::from(9),
            Decimal::from(12),
        ];
        let closes = vec![
            Decimal::from(11),
            Decimal::from(12),
            Decimal::from(10),
            Decimal::from(13),
        ];
        
        let result = atr(2, &highs, &lows, &closes).unwrap();
        assert_eq!(result.len(), 2);
        
        // TR at index 1: max(13-11=2, |13-11|=2, |11-11|=0) = 2
        // TR at index 2: max(11-9=2, |11-12|=1, |9-12|=3) = 3
        // TR at index 3: max(14-12=2, |14-10|=4, |12-10|=2) = 4
        
        // ATR at index 2: (2+3)/2 = 2.5
        assert_eq!(result[0].timestamp, 2);
        assert_eq!(result[0].value, Decimal::new(25, 1));
        
        // ATR at index 3: (3+4)/2 = 3.5
        assert_eq!(result[1].timestamp, 3);
        assert_eq!(result[1].value, Decimal::new(35, 1));
    }

    #[test]
    fn test_atr_mismatched_arrays() {
        let highs = vec![Decimal::from(12)];
        let lows = vec![Decimal::from(10), Decimal::from(11)];
        let closes = vec![Decimal::from(11)];
        
        let result = atr(2, &highs, &lows, &closes);
        assert!(result.is_err());
    }

    #[test]
    fn test_atr_insufficient_data() {
        let highs = vec![Decimal::from(12), Decimal::from(13)];
        let lows = vec![Decimal::from(10), Decimal::from(11)];
        let closes = vec![Decimal::from(11), Decimal::from(12)];
        
        let result = atr(2, &highs, &lows, &closes);
        assert!(result.is_err());
    }

    #[test]
    fn test_atr_zero_period() {
        let highs = vec![Decimal::from(12), Decimal::from(13), Decimal::from(11)];
        let lows = vec![Decimal::from(10), Decimal::from(11), Decimal::from(9)];
        let closes = vec![Decimal::from(11), Decimal::from(12), Decimal::from(10)];
        
        let result = atr(0, &highs, &lows, &closes);
        assert!(result.is_err());
    }

    #[test]
    fn test_atr_with_gap() {
        let highs = vec![
            Decimal::from(10),
            Decimal::from(15), // gap up
        ];
        let lows = vec![
            Decimal::from(8),
            Decimal::from(12),
        ];
        let closes = vec![
            Decimal::from(9),
            Decimal::from(14),
        ];
        
        let result = atr(1, &highs, &lows, &closes).unwrap();
        assert_eq!(result.len(), 1);
        
        // TR at index 1: max(15-12=3, |15-9|=6, |12-9|=3) = 6
        assert_eq!(result[0].value, Decimal::from(6));
    }
}
