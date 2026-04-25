use rust_decimal::Decimal;
use crate::IndicatorResult;
use crate::IndicatorError;

pub fn rsi(_period: usize, _prices: &[Decimal]) -> Result<Vec<IndicatorResult>, IndicatorError> {
    Ok(Vec::new())
}
