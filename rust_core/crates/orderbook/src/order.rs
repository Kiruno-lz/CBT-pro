use crate::{OrderFill, OrderRequest, OrderSide, OrderType, StandardBar};
use rust_decimal::Decimal;

/// Simulates order execution against a [`BarSnapshot`].
///
/// All fills are deterministic and use conservative pricing:
/// - Market Buy fills at `bar.close` (or `bar.high` if more conservative)
/// - Market Sell fills at `bar.close` (or `bar.low` if more conservative)
///
/// For this implementation we fill at `bar.close` for market orders, which is
/// the standard assumption in backtesting. In a more conservative mode, Buy
/// would fill at `bar.high` and Sell at `bar.low` (worst-case execution).
pub struct OrderSimulator;

impl OrderSimulator {
    /// Simulate a market order fill.
    ///
    /// - Buy: filled at `bar.close`
    /// - Sell: filled at `bar.close`
    /// - Fee: `filled_quantity * filled_price * fee_rate`
    pub fn simulate_market_order(
        req: &OrderRequest,
        bar: &StandardBar,
        fee_rate: Decimal,
    ) -> OrderFill {
        let filled_price = bar.close;
        let filled_quantity = req.quantity;
        let fee = filled_quantity * filled_price * fee_rate;
        OrderFill {
            order_id: req.order_id,
            position_id: None,
            symbol: req.symbol.clone(),
            side: req.side,
            direction: req.direction,
            filled_price,
            filled_quantity,
            fee,
            fee_asset: "USDT".to_string(),
            timestamp: bar.timestamp,
            realized_pnl: None,
        }
    }

    /// Simulate a limit order fill.
    ///
    /// - Buy Limit: fills if `bar.low <= limit_price`, at `min(limit_price, bar.close)`
    /// - Sell Limit: fills if `bar.high >= limit_price`, at `max(limit_price, bar.close)`
    /// - Returns `None` if the limit is not touched during the bar.
    pub fn simulate_limit_order(
        req: &OrderRequest,
        bar: &StandardBar,
        fee_rate: Decimal,
    ) -> Option<OrderFill> {
        let limit_price = match req.order_type {
            OrderType::Limit(p) => p,
            _ => return None,
        };

        let filled_price = match req.side {
            OrderSide::Buy => {
                if bar.low > limit_price {
                    return None;
                }
                limit_price.min(bar.close)
            }
            OrderSide::Sell => {
                if bar.high < limit_price {
                    return None;
                }
                limit_price.max(bar.close)
            }
        };

        let filled_quantity = req.quantity;
        let fee = filled_quantity * filled_price * fee_rate;
        Some(OrderFill {
            order_id: req.order_id,
            position_id: None,
            symbol: req.symbol.clone(),
            side: req.side,
            direction: req.direction,
            filled_price,
            filled_quantity,
            fee,
            fee_asset: "USDT".to_string(),
            timestamp: bar.timestamp,
            realized_pnl: None,
        })
    }

    /// Simulate a stop-market order fill.
    ///
    /// - Buy Stop: triggers if `bar.high >= stop_price`, fills at `bar.close`
    /// - Sell Stop: triggers if `bar.low <= stop_price`, fills at `bar.close`
    /// - Returns `None` if the stop is not triggered during the bar.
    pub fn simulate_stop_market(
        req: &OrderRequest,
        bar: &StandardBar,
        fee_rate: Decimal,
    ) -> Option<OrderFill> {
        let stop_price = match req.order_type {
            OrderType::StopMarket(p) => p,
            _ => return None,
        };

        let triggered = match req.side {
            OrderSide::Buy => bar.high >= stop_price,
            OrderSide::Sell => bar.low <= stop_price,
        };

        if !triggered {
            return None;
        }

        let filled_price = bar.close;
        let filled_quantity = req.quantity;
        let fee = filled_quantity * filled_price * fee_rate;
        Some(OrderFill {
            order_id: req.order_id,
            position_id: None,
            symbol: req.symbol.clone(),
            side: req.side,
            direction: req.direction,
            filled_price,
            filled_quantity,
            fee,
            fee_asset: "USDT".to_string(),
            timestamp: bar.timestamp,
            realized_pnl: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Direction, MarginMode};
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn make_bar(low: Decimal, high: Decimal, close: Decimal) -> StandardBar {
        StandardBar {
            timestamp: 1704067200000,
            open: close - dec!(10),
            high,
            low,
            close,
            volume: dec!(100),
            symbol: "BTC-USDT".to_string(),
            exchange: "binance".to_string(),
            confirmed: true,
        }
    }

    fn make_req(side: OrderSide, order_type: OrderType, qty: Decimal) -> OrderRequest {
        OrderRequest {
            order_id: Uuid::new_v4(),
            symbol: "BTC-USDT".to_string(),
            side,
            direction: Direction::Long,
            order_type,
            quantity: qty,
            margin_mode: MarginMode::Isolated,
            leverage: dec!(10),
            timestamp: 1704067200000,
            strategy_id: "test".to_string(),
            signal_strength: 1.0,
            signal_reason: "test".to_string(),
        }
    }

    #[test]
    fn test_market_order_fill() {
        let bar = make_bar(dec!(39900), dec!(40100), dec!(40000));
        let req = make_req(OrderSide::Buy, OrderType::Market, dec!(0.5));
        let fill = OrderSimulator::simulate_market_order(&req, &bar, dec!(0.001));
        assert_eq!(fill.filled_price, dec!(40000));
        assert_eq!(fill.filled_quantity, dec!(0.5));
        assert_eq!(fill.fee, dec!(20)); // 0.5 * 40000 * 0.001
    }

    #[test]
    fn test_limit_buy_fill() {
        let bar = make_bar(dec!(39500), dec!(40500), dec!(40000));
        let req = make_req(OrderSide::Buy, OrderType::Limit(dec!(39800)), dec!(0.5));
        let fill = OrderSimulator::simulate_limit_order(&req, &bar, dec!(0.001));
        assert!(fill.is_some());
        let fill = fill.unwrap();
        assert_eq!(fill.filled_price, dec!(39800)); // min(limit, close)
    }

    #[test]
    fn test_limit_buy_no_fill() {
        let bar = make_bar(dec!(40000), dec!(41000), dec!(40500));
        let req = make_req(OrderSide::Buy, OrderType::Limit(dec!(39500)), dec!(0.5));
        let fill = OrderSimulator::simulate_limit_order(&req, &bar, dec!(0.001));
        assert!(fill.is_none());
    }

    #[test]
    fn test_limit_sell_fill() {
        let bar = make_bar(dec!(39500), dec!(40500), dec!(40000));
        let req = make_req(OrderSide::Sell, OrderType::Limit(dec!(40200)), dec!(0.5));
        let fill = OrderSimulator::simulate_limit_order(&req, &bar, dec!(0.001));
        assert!(fill.is_some());
        let fill = fill.unwrap();
        assert_eq!(fill.filled_price, dec!(40200)); // max(limit, close)
    }

    #[test]
    fn test_stop_market_buy_triggered() {
        let bar = make_bar(dec!(39500), dec!(40500), dec!(40000));
        let req = make_req(
            OrderSide::Buy,
            OrderType::StopMarket(dec!(40200)),
            dec!(0.5),
        );
        let fill = OrderSimulator::simulate_stop_market(&req, &bar, dec!(0.001));
        assert!(fill.is_some());
        let fill = fill.unwrap();
        assert_eq!(fill.filled_price, dec!(40000));
    }

    #[test]
    fn test_stop_market_buy_not_triggered() {
        let bar = make_bar(dec!(39500), dec!(39900), dec!(39800));
        let req = make_req(
            OrderSide::Buy,
            OrderType::StopMarket(dec!(40200)),
            dec!(0.5),
        );
        let fill = OrderSimulator::simulate_stop_market(&req, &bar, dec!(0.001));
        assert!(fill.is_none());
    }
}
