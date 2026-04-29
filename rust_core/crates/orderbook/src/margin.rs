use crate::Direction;
use rust_decimal::Decimal;

/// Calculates margin and liquidation metrics for leveraged positions.
pub struct MarginCalculator;

impl MarginCalculator {
    /// Compute initial margin required to open a position.
    ///
    /// Formula: `size * price / leverage`
    pub fn initial_margin(size: Decimal, price: Decimal, leverage: Decimal) -> Decimal {
        size * price / leverage
    }

    /// Compute maintenance margin.
    ///
    /// Formula: `size * price * rate`
    pub fn maintenance_margin(size: Decimal, price: Decimal, rate: Decimal) -> Decimal {
        size * price * rate
    }

    /// Compute the liquidation price for a position.
    ///
    /// - Long: `entry * (1 - 1/leverage + maintenance_rate)`
    /// - Short: `entry * (1 + 1/leverage - maintenance_rate)`
    ///
    /// # Panics
    /// Panics if `leverage` is zero.
    pub fn liquidation_price(
        entry: Decimal,
        leverage: Decimal,
        maintenance_rate: Decimal,
        direction: Direction,
    ) -> Decimal {
        let one = Decimal::ONE;
        let inv_leverage = one / leverage;
        match direction {
            Direction::Long => entry * (one - inv_leverage + maintenance_rate),
            Direction::Short => entry * (one + inv_leverage - maintenance_rate),
        }
    }

    /// Compute the margin ratio.
    ///
    /// Formula: `(margin_used + unrealized_pnl) / total_equity`
    ///
    /// Returns zero if `total_equity` is zero to avoid division by zero.
    pub fn margin_ratio(
        margin_used: Decimal,
        unrealized_pnl: Decimal,
        total_equity: Decimal,
    ) -> Decimal {
        if total_equity.is_zero() {
            return Decimal::ZERO;
        }
        (margin_used + unrealized_pnl) / total_equity
    }
}
