use crate::{IndicatorError, IndicatorResult};
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq)]
pub struct BollingerBands {
    pub upper: Decimal,
    pub middle: Decimal,
    pub lower: Decimal,
}

fn decimal_sqrt(value: Decimal) -> Result<Decimal, IndicatorError> {
    let f = value.to_f64().ok_or_else(|| {
        IndicatorError::CalculationError("cannot convert Decimal to f64".to_string())
    })?;
    let sqrt_f = f.sqrt();
    if sqrt_f.is_nan() || sqrt_f.is_infinite() {
        return Err(IndicatorError::CalculationError(
            "sqrt result is invalid".to_string(),
        ));
    }
    Decimal::from_f64(sqrt_f).ok_or_else(|| {
        IndicatorError::CalculationError("cannot convert f64 to Decimal".to_string())
    })
}

pub fn bollinger(
    period: usize,
    std_dev: Decimal,
    prices: &[Decimal],
) -> Result<Vec<(IndicatorResult, BollingerBands)>, IndicatorError> {
    if period == 0 {
        return Err(IndicatorError::InvalidParameter(
            "period must be > 0".to_string(),
        ));
    }
    if prices.len() < period {
        return Err(IndicatorError::InsufficientData {
            required: period,
            got: prices.len(),
        });
    }

    let mut result = Vec::new();
    let period_dec = Decimal::from(period as i64);

    for i in period..=prices.len() {
        let window = &prices[i - period..i];
        let sma = window.iter().copied().sum::<Decimal>() / period_dec;

        let variance = window
            .iter()
            .map(|&price| {
                let diff = price - sma;
                diff * diff
            })
            .sum::<Decimal>()
            / period_dec;

        let std = decimal_sqrt(variance)?;
        let upper = sma + std_dev * std;
        let lower = sma - std_dev * std;

        result.push((
            IndicatorResult {
                value: sma,
                timestamp: (i - 1) as i64,
            },
            BollingerBands {
                upper,
                middle: sma,
                lower,
            },
        ));
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bollinger_basic() {
        let prices = vec![
            Decimal::from(10),
            Decimal::from(12),
            Decimal::from(11),
            Decimal::from(13),
        ];
        let result = bollinger(2, Decimal::from(2), &prices).unwrap();
        assert_eq!(result.len(), 3);

        // Window [10, 12]: SMA=11, variance=1, std=1
        // upper = 11 + 2*1 = 13, lower = 11 - 2*1 = 9
        assert_eq!(result[0].0.timestamp, 1);
        assert_eq!(result[0].0.value, Decimal::from(11));
        assert_eq!(result[0].1.upper, Decimal::from(13));
        assert_eq!(result[0].1.middle, Decimal::from(11));
        assert_eq!(result[0].1.lower, Decimal::from(9));

        // Window [12, 11]: SMA=11.5, variance=0.25, std=0.5
        // upper = 11.5 + 2*0.5 = 12.5, lower = 11.5 - 2*0.5 = 10.5
        assert_eq!(result[1].0.timestamp, 2);
        assert_eq!(result[1].0.value, Decimal::new(115, 1));
        assert_eq!(result[1].1.upper, Decimal::new(125, 1));
        assert_eq!(result[1].1.lower, Decimal::new(105, 1));

        // Window [11, 13]: SMA=12, variance=1, std=1
        // upper = 12 + 2*1 = 14, lower = 12 - 2*1 = 10
        assert_eq!(result[2].0.timestamp, 3);
        assert_eq!(result[2].0.value, Decimal::from(12));
        assert_eq!(result[2].1.upper, Decimal::from(14));
        assert_eq!(result[2].1.lower, Decimal::from(10));
    }

    #[test]
    fn test_bollinger_insufficient_data() {
        let prices = vec![Decimal::from(10)];
        let result = bollinger(2, Decimal::from(2), &prices);
        assert!(result.is_err());
    }

    #[test]
    fn test_bollinger_zero_period() {
        let prices = vec![Decimal::from(10), Decimal::from(12)];
        let result = bollinger(0, Decimal::from(2), &prices);
        assert!(result.is_err());
    }

    #[test]
    fn test_bollinger_constant_prices() {
        let prices = vec![Decimal::from(10), Decimal::from(10), Decimal::from(10)];
        let result = bollinger(2, Decimal::from(2), &prices).unwrap();
        // Variance = 0, so std = 0, upper = lower = middle
        for (_, bands) in result {
            assert_eq!(bands.upper, bands.middle);
            assert_eq!(bands.lower, bands.middle);
        }
    }
}
