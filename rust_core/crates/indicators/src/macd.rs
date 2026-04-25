use rust_decimal::Decimal;
use crate::IndicatorResult;
use crate::IndicatorError;

pub struct MacdResult {
    pub macd: Decimal,
    pub signal: Decimal,
    pub histogram: Decimal,
}

pub fn macd(
    _fast: usize,
    _slow: usize,
    _signal: usize,
    _prices: &[Decimal],
) -> Result<Vec<(IndicatorResult, MacdResult)>, IndicatorError> {
    Ok(Vec::new())
}
