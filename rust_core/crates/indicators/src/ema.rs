use rust_decimal::Decimal;
use crate::{IndicatorResult, IndicatorError};

pub fn ema(period: usize, prices: &[Decimal]) -> Result<Vec<IndicatorResult>, IndicatorError> {
    if period == 0 {
        return Err(IndicatorError::InvalidParameter("period must be > 0".to_string()));
    }
    if prices.len() < period {
        return Err(IndicatorError::InsufficientData {
            required: period,
            got: prices.len(),
        });
    }

    let k = Decimal::from(2) / Decimal::from(period as i64 + 1);
    let one_minus_k = Decimal::from(1) - k;
    let mut result = Vec::new();

    let mut ema_val: Decimal = prices[..period].iter().copied().sum::<Decimal>() 
        / Decimal::from(period as i64);
    
    result.push(IndicatorResult {
        value: ema_val,
        timestamp: (period - 1) as i64,
    });

    for (i, &price) in prices.iter().enumerate().skip(period) {
        ema_val = price * k + ema_val * one_minus_k;
        result.push(IndicatorResult {
            value: ema_val,
            timestamp: i as i64,
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ema_basic() {
        let prices = vec![
            Decimal::from(10),
            Decimal::from(11),
            Decimal::from(12),
            Decimal::from(11),
            Decimal::from(13),
            Decimal::from(15),
        ];
        let result = ema(3, &prices).unwrap();
        assert_eq!(result.len(), 4);
        
        // First EMA is SMA of first 3: (10+11+12)/3 = 11
        assert_eq!(result[0].timestamp, 2);
        assert_eq!(result[0].value, Decimal::from(11));
        
        // EMA at index 3: 11*0.5 + 11*0.5 = 11
        assert_eq!(result[1].timestamp, 3);
        assert_eq!(result[1].value, Decimal::from(11));
        
        // EMA at index 4: 13*0.5 + 11*0.5 = 12
        assert_eq!(result[2].timestamp, 4);
        assert_eq!(result[2].value, Decimal::from(12));
        
        // EMA at index 5: 15*0.5 + 12*0.5 = 13.5
        assert_eq!(result[3].timestamp, 5);
        assert_eq!(result[3].value, Decimal::new(135, 1));
    }

    #[test]
    fn test_ema_insufficient_data() {
        let prices = vec![Decimal::from(10), Decimal::from(11)];
        let result = ema(3, &prices);
        assert!(result.is_err());
    }

    #[test]
    fn test_ema_zero_period() {
        let prices = vec![Decimal::from(10)];
        let result = ema(0, &prices);
        assert!(result.is_err());
    }

    #[test]
    fn test_ema_single_period() {
        let prices = vec![Decimal::from(10), Decimal::from(12), Decimal::from(11)];
        let result = ema(1, &prices).unwrap();
        assert_eq!(result.len(), 3);
        // With period=1, k=1, so EMA = price
        assert_eq!(result[0].value, Decimal::from(10));
        assert_eq!(result[1].value, Decimal::from(12));
        assert_eq!(result[2].value, Decimal::from(11));
    }
}
