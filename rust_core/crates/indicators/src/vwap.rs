use crate::{IndicatorError, IndicatorResult};
use rust_decimal::Decimal;

pub fn vwap(
    prices: &[Decimal],
    volumes: &[Decimal],
) -> Result<Vec<IndicatorResult>, IndicatorError> {
    if prices.len() != volumes.len() {
        return Err(IndicatorError::InvalidParameter(
            "prices and volumes must have same length".to_string(),
        ));
    }
    if prices.is_empty() {
        return Err(IndicatorError::InsufficientData {
            required: 1,
            got: 0,
        });
    }

    let mut cum_pv = Decimal::ZERO;
    let mut cum_vol = Decimal::ZERO;
    let mut result = Vec::new();

    for i in 0..prices.len() {
        cum_pv += prices[i] * volumes[i];
        cum_vol += volumes[i];

        if cum_vol == Decimal::ZERO {
            return Err(IndicatorError::CalculationError(
                "cumulative volume is zero".to_string(),
            ));
        }

        let vwap_val = cum_pv / cum_vol;
        result.push(IndicatorResult {
            value: vwap_val,
            timestamp: i as i64,
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vwap_basic() {
        let prices = vec![Decimal::from(10), Decimal::from(12), Decimal::from(11)];
        let volumes = vec![Decimal::from(100), Decimal::from(200), Decimal::from(150)];

        let result = vwap(&prices, &volumes).unwrap();
        assert_eq!(result.len(), 3);

        // Index 0: (10*100)/100 = 10
        assert_eq!(result[0].timestamp, 0);
        assert_eq!(result[0].value, Decimal::from(10));

        // Index 1: (10*100 + 12*200)/300 = 3400/300 = 11.33...
        assert_eq!(result[1].timestamp, 1);
        let expected_1 = Decimal::from(3400) / Decimal::from(300);
        assert_eq!(result[1].value, expected_1);

        // Index 2: (10*100 + 12*200 + 11*150)/450 = 5050/450 = 11.22...
        assert_eq!(result[2].timestamp, 2);
        let expected_2 = Decimal::from(5050) / Decimal::from(450);
        assert_eq!(result[2].value, expected_2);
    }

    #[test]
    fn test_vwap_mismatched_arrays() {
        let prices = vec![Decimal::from(10)];
        let volumes = vec![Decimal::from(100), Decimal::from(200)];

        let result = vwap(&prices, &volumes);
        assert!(result.is_err());
    }

    #[test]
    fn test_vwap_empty() {
        let prices: Vec<Decimal> = vec![];
        let volumes: Vec<Decimal> = vec![];

        let result = vwap(&prices, &volumes);
        assert!(result.is_err());
    }

    #[test]
    fn test_vwap_zero_volume() {
        let prices = vec![Decimal::from(10), Decimal::from(12)];
        let volumes = vec![Decimal::from(0), Decimal::from(200)];

        let result = vwap(&prices, &volumes);
        // First bar has zero volume, but second bar should work
        // Actually, cumulative volume after first bar is 0, which causes error
        assert!(result.is_err());
    }

    #[test]
    fn test_vwap_single_bar() {
        let prices = vec![Decimal::from(10)];
        let volumes = vec![Decimal::from(100)];

        let result = vwap(&prices, &volumes).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, Decimal::from(10));
    }

    #[test]
    fn test_vwap_equal_prices() {
        let prices = vec![Decimal::from(10), Decimal::from(10), Decimal::from(10)];
        let volumes = vec![Decimal::from(100), Decimal::from(200), Decimal::from(300)];

        let result = vwap(&prices, &volumes).unwrap();
        for r in result {
            assert_eq!(r.value, Decimal::from(10));
        }
    }
}
