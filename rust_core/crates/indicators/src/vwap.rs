use rust_decimal::Decimal;
use crate::IndicatorResult;
use crate::IndicatorError;

pub fn vwap(_prices: &[Decimal], _volumes: &[Decimal]) -> Result<Vec<IndicatorResult>, IndicatorError> {
    Ok(Vec::new())
}
