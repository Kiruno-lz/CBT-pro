use rust_decimal::Decimal;
use crate::{IndicatorResult, IndicatorError};

pub fn rsi(period: usize, prices: &[Decimal]) -> Result<Vec<IndicatorResult>, IndicatorError> {
    if period == 0 {
        return Err(IndicatorError::InvalidParameter("period must be > 0".to_string()));
    }
    if prices.len() <= period {
        return Err(IndicatorError::InsufficientData {
            required: period + 1,
            got: prices.len(),
        });
    }

    let mut gains = Vec::new();
    let mut losses = Vec::new();

    for i in 1..prices.len() {
        let change = prices[i] - prices[i - 1];
        if change >= Decimal::ZERO {
            gains.push(change);
            losses.push(Decimal::ZERO);
        } else {
            gains.push(Decimal::ZERO);
            losses.push(-change);
        }
    }

    let mut avg_gain: Decimal = gains[..period].iter().copied().sum::<Decimal>() 
        / Decimal::from(period as i64);
    let mut avg_loss: Decimal = losses[..period].iter().copied().sum::<Decimal>() 
        / Decimal::from(period as i64);
    
    let mut result = Vec::new();
    
    let rsi_val = calculate_rsi(avg_gain, avg_loss);
    result.push(IndicatorResult {
        value: rsi_val,
        timestamp: period as i64,
    });

    let period_dec = Decimal::from(period as i64);
    let period_minus_one = Decimal::from(period as i64 - 1);

    for i in period..gains.len() {
        avg_gain = (avg_gain * period_minus_one + gains[i]) / period_dec;
        avg_loss = (avg_loss * period_minus_one + losses[i]) / period_dec;
        
        let rsi_val = calculate_rsi(avg_gain, avg_loss);
        result.push(IndicatorResult {
            value: rsi_val,
            timestamp: (i + 1) as i64,
        });
    }

    Ok(result)
}

fn calculate_rsi(avg_gain: Decimal, avg_loss: Decimal) -> Decimal {
    if avg_loss == Decimal::ZERO {
        Decimal::from(100)
    } else {
        let rs = avg_gain / avg_loss;
        Decimal::from(100) - (Decimal::from(100) / (Decimal::from(1) + rs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsi_basic() {
        let prices = vec![
            Decimal::from(10),
            Decimal::from(12),
            Decimal::from(11),
            Decimal::from(13),
            Decimal::from(15),
        ];
        let result = rsi(2, &prices).unwrap();
        assert_eq!(result.len(), 3);
        
        // Changes: [2, -1, 2, 2]
        // Gains:   [2, 0, 2, 2]
        // Losses:  [0, 1, 0, 0]
        // First avg_gain = (2+0)/2 = 1
        // First avg_loss = (0+1)/2 = 0.5
        // RS = 2, RSI = 100 - 100/3 = 66.67
        assert_eq!(result[0].timestamp, 2);
        let expected_rsi_0 = Decimal::from(100) - Decimal::from(100) / Decimal::from(3);
        assert_eq!(result[0].value, expected_rsi_0);
        
        // Next: avg_gain = (1*1 + 2)/2 = 1.5
        //       avg_loss = (0.5*1 + 0)/2 = 0.25
        // RS = 6, RSI = 100 - 100/7 = 85.71
        assert_eq!(result[1].timestamp, 3);
        let expected_rsi_1 = Decimal::from(100) - Decimal::from(100) / Decimal::from(7);
        assert_eq!(result[1].value, expected_rsi_1);
        
        // Next: avg_gain = (1.5*1 + 2)/2 = 1.75
        //       avg_loss = (0.25*1 + 0)/2 = 0.125
        // RS = 14, RSI = 100 - 100/15 = 93.33
        assert_eq!(result[2].timestamp, 4);
        let expected_rsi_2 = Decimal::from(100) - Decimal::from(100) / Decimal::from(15);
        assert_eq!(result[2].value, expected_rsi_2);
    }

    #[test]
    fn test_rsi_all_gains() {
        let prices = vec![
            Decimal::from(10),
            Decimal::from(11),
            Decimal::from(12),
            Decimal::from(13),
        ];
        let result = rsi(2, &prices).unwrap();
        // avg_loss is always 0, so RSI should be 100
        for r in result {
            assert_eq!(r.value, Decimal::from(100));
        }
    }

    #[test]
    fn test_rsi_insufficient_data() {
        let prices = vec![Decimal::from(10), Decimal::from(11)];
        let result = rsi(2, &prices);
        assert!(result.is_err());
    }

    #[test]
    fn test_rsi_zero_period() {
        let prices = vec![Decimal::from(10)];
        let result = rsi(0, &prices);
        assert!(result.is_err());
    }
}
