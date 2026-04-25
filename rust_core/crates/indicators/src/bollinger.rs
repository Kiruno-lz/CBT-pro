use rust_decimal::Decimal;
use crate::IndicatorResult;
use crate::IndicatorError;

pub struct BollingerBands {
    pub upper: Decimal,
    pub middle: Decimal,
    pub lower: Decimal,
}

pub fn bollinger(
    _period: usize,
    _std_dev: Decimal,
    _prices: &[Decimal],
) -> Result<Vec<(IndicatorResult, BollingerBands)>, IndicatorError> {
    Ok(Vec::new())
}
