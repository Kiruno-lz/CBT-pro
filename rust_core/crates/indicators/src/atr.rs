use rust_decimal::Decimal;
use crate::IndicatorResult;
use crate::IndicatorError;

pub fn atr(_period: usize, _highs: &[Decimal], _lows: &[Decimal], _closes: &[Decimal]) -> Result<Vec<IndicatorResult>, IndicatorError> {
    Ok(Vec::new())
}
