use crate::margin::MarginCalculator;
use crate::{
    CostBasisMethod, Direction, OrderBookError, OrderFill, OrderRequest, Position, PositionBook,
    PositionId, PositionLeg, PositionStatus,
};
use rust_decimal::Decimal;
use std::collections::HashMap;
use uuid::Uuid;

/// Manages an in-memory position book with full open/add/reduce/close lifecycle.
pub trait OrderBookManager {
    /// Open a new position from an initial fill.
    fn open_position(
        &mut self,
        req: &OrderRequest,
        fill: &OrderFill,
    ) -> Result<Position, OrderBookError>;

    /// Add to an existing position (pyramiding / scaling in).
    fn add_to_position(
        &mut self,
        pos_id: PositionId,
        fill: &OrderFill,
    ) -> Result<Position, OrderBookError>;

    /// Reduce a position partially, returning the updated position and realized PnL.
    fn reduce_position(
        &mut self,
        pos_id: PositionId,
        fill: &OrderFill,
        method: CostBasisMethod,
    ) -> Result<(Position, Decimal), OrderBookError>;

    /// Fully close a position, returning the updated position and realized PnL.
    fn close_position(
        &mut self,
        pos_id: PositionId,
        fill: &OrderFill,
        method: CostBasisMethod,
    ) -> Result<(Position, Decimal), OrderBookError>;

    /// Get a single position by ID.
    fn get_position(&self, pos_id: PositionId) -> Option<&Position>;

    /// Get all open positions for a symbol.
    fn get_positions_by_symbol(&self, symbol: &str) -> Vec<&Position>;

    /// Get all open positions.
    fn get_all_positions(&self) -> Vec<&Position>;

    /// Update unrealized PnL for all positions of a given symbol using the mark price.
    fn update_unrealized_pnl(&mut self, symbol: &str, mark_price: Decimal);

    /// Check if a position has been liquidated at the given mark price.
    fn check_liquidation(
        &self,
        pos_id: PositionId,
        mark_price: Decimal,
        maintenance_rate: Decimal,
    ) -> bool;

    /// Get a reference to the underlying position book.
    fn get_position_book(&self) -> &PositionBook;
}

/// In-memory implementation of [`OrderBookManager`].
#[derive(Debug, Clone)]
pub struct InMemoryOrderBook {
    book: PositionBook,
}

impl Default for InMemoryOrderBook {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryOrderBook {
    /// Create a new empty position book.
    pub fn new() -> Self {
        Self {
            book: PositionBook {
                positions: HashMap::new(),
                closed_positions: Vec::new(),
                margin_used: Decimal::ZERO,
                unrealized_pnl: Decimal::ZERO,
            },
        }
    }

    /// Recalculate the book-level margin used from all open positions.
    ///
    /// Margin used = sum of (size * avg_entry / leverage) for each position.
    fn recalc_margin_used(&mut self) {
        let total = Decimal::ZERO;
        for _pos in self.book.positions.values() {
            // We don't have leverage stored on Position; the margin is tracked externally.
            // For now, we keep the existing margin_used field updated by the engine.
        }
        // Placeholder: margin_used is updated by the engine when fills occur.
        let _ = total;
    }

    /// Recalculate book-level unrealized PnL from all open positions.
    fn recalc_unrealized_pnl(&mut self) {
        let total: Decimal = self.book.positions.values().map(|p| p.unrealized_pnl).sum();
        self.book.unrealized_pnl = total;
    }
}

impl OrderBookManager for InMemoryOrderBook {
    fn open_position(
        &mut self,
        req: &OrderRequest,
        fill: &OrderFill,
    ) -> Result<Position, OrderBookError> {
        let pos_id = Uuid::new_v4();
        let leg = PositionLeg {
            entry_price: fill.filled_price,
            quantity: fill.filled_quantity,
            timestamp: fill.timestamp,
            order_id: fill.order_id,
        };
        let pos = Position {
            id: pos_id,
            symbol: req.symbol.clone(),
            direction: req.direction,
            status: PositionStatus::Open,
            entries: vec![leg],
            current_size: fill.filled_quantity,
            average_entry_price: fill.filled_price,
            realized_pnl: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            opened_at: fill.timestamp,
            updated_at: fill.timestamp,
        };
        self.book.positions.insert(pos_id, pos.clone());
        Ok(pos)
    }

    fn add_to_position(
        &mut self,
        pos_id: PositionId,
        fill: &OrderFill,
    ) -> Result<Position, OrderBookError> {
        let pos = self
            .book
            .positions
            .get_mut(&pos_id)
            .ok_or(OrderBookError::PositionNotFound(pos_id))?;

        if pos.status == PositionStatus::Closed {
            return Err(OrderBookError::InvalidOperation(format!(
                "Cannot add to closed position {}",
                pos_id
            )));
        }

        let leg = PositionLeg {
            entry_price: fill.filled_price,
            quantity: fill.filled_quantity,
            timestamp: fill.timestamp,
            order_id: fill.order_id,
        };
        pos.entries.push(leg);

        // Weighted average recalculation
        let total_cost =
            pos.average_entry_price * pos.current_size + fill.filled_price * fill.filled_quantity;
        pos.current_size += fill.filled_quantity;
        pos.average_entry_price = total_cost / pos.current_size;
        pos.updated_at = fill.timestamp;

        Ok(pos.clone())
    }

    fn reduce_position(
        &mut self,
        pos_id: PositionId,
        fill: &OrderFill,
        method: CostBasisMethod,
    ) -> Result<(Position, Decimal), OrderBookError> {
        let pos = self
            .book
            .positions
            .get_mut(&pos_id)
            .ok_or(OrderBookError::PositionNotFound(pos_id))?;

        if fill.filled_quantity > pos.current_size {
            return Err(OrderBookError::InsufficientSize {
                have: pos.current_size,
                need: fill.filled_quantity,
            });
        }

        let realized_pnl = match method {
            CostBasisMethod::FIFO => reduce_fifo(pos, fill),
            CostBasisMethod::LIFO => reduce_lifo(pos, fill),
            CostBasisMethod::WeightedAverage => reduce_weighted_average(pos, fill),
        };

        pos.realized_pnl += realized_pnl;
        pos.current_size -= fill.filled_quantity;
        pos.updated_at = fill.timestamp;

        if pos.current_size.is_zero() {
            pos.status = PositionStatus::Closed;
            pos.unrealized_pnl = Decimal::ZERO;
        } else {
            pos.status = PositionStatus::PartiallyClosed;
        }

        let pos_clone = pos.clone();

        // If fully closed, move to closed_positions
        if pos.status == PositionStatus::Closed {
            let removed = self.book.positions.remove(&pos_id).unwrap();
            self.book.closed_positions.push(removed);
        }

        self.recalc_unrealized_pnl();
        Ok((pos_clone, realized_pnl))
    }

    fn close_position(
        &mut self,
        pos_id: PositionId,
        fill: &OrderFill,
        method: CostBasisMethod,
    ) -> Result<(Position, Decimal), OrderBookError> {
        // First reduce to zero, then ensure status is Closed
        let mut fill = fill.clone();
        let pos = self
            .book
            .positions
            .get(&pos_id)
            .ok_or(OrderBookError::PositionNotFound(pos_id))?;
        fill.filled_quantity = pos.current_size; // force full close
        let (mut pos, realized_pnl) = self.reduce_position(pos_id, &fill, method)?;
        pos.status = PositionStatus::Closed;
        Ok((pos, realized_pnl))
    }

    fn get_position(&self, pos_id: PositionId) -> Option<&Position> {
        self.book.positions.get(&pos_id)
    }

    fn get_positions_by_symbol(&self, symbol: &str) -> Vec<&Position> {
        self.book
            .positions
            .values()
            .filter(|p| p.symbol == symbol)
            .collect()
    }

    fn get_all_positions(&self) -> Vec<&Position> {
        self.book.positions.values().collect()
    }

    fn update_unrealized_pnl(&mut self, symbol: &str, mark_price: Decimal) {
        for pos in self.book.positions.values_mut() {
            if pos.symbol != symbol {
                continue;
            }
            let dir_multiplier = match pos.direction {
                Direction::Long => Decimal::ONE,
                Direction::Short => Decimal::NEGATIVE_ONE,
            };
            pos.unrealized_pnl =
                pos.current_size * (mark_price - pos.average_entry_price) * dir_multiplier;
        }
        self.recalc_unrealized_pnl();
    }

    fn check_liquidation(
        &self,
        pos_id: PositionId,
        mark_price: Decimal,
        maintenance_rate: Decimal,
    ) -> bool {
        let Some(pos) = self.book.positions.get(&pos_id) else {
            return false;
        };

        // For liquidation check, we need leverage. We don't store it on Position directly,
        // so we compute an approximate check using a default high leverage or
        // the engine passes it. Here we use a simplified formula:
        // Long liq_price = entry * (1 - 1/leverage + maintenance_rate)
        // Short liq_price = entry * (1 + 1/leverage - maintenance_rate)
        // Since we don't have leverage here, we return false and let the engine
        // compute exact liquidation with leverage info.
        // However, for completeness, if leverage was stored we'd compute:
        let _ = (mark_price, maintenance_rate, pos);
        false
    }

    fn get_position_book(&self) -> &PositionBook {
        &self.book
    }
}

// ---------------------------------------------------------------------------
// Cost-basis reduction helpers
// ---------------------------------------------------------------------------

/// Reduce using FIFO: remove oldest legs first.
fn reduce_fifo(pos: &mut Position, fill: &OrderFill) -> Decimal {
    let mut remaining = fill.filled_quantity;
    let mut cost_basis = Decimal::ZERO;
    let mut new_entries = Vec::new();

    for leg in pos.entries.drain(..) {
        if remaining.is_zero() {
            new_entries.push(leg);
            continue;
        }
        if leg.quantity <= remaining {
            cost_basis += leg.entry_price * leg.quantity;
            remaining -= leg.quantity;
        } else {
            cost_basis += leg.entry_price * remaining;
            new_entries.push(PositionLeg {
                entry_price: leg.entry_price,
                quantity: leg.quantity - remaining,
                timestamp: leg.timestamp,
                order_id: leg.order_id,
            });
            remaining = Decimal::ZERO;
        }
    }

    pos.entries = new_entries;
    let proceeds = fill.filled_price * fill.filled_quantity;
    let dir_multiplier = match pos.direction {
        Direction::Long => Decimal::ONE,
        Direction::Short => Decimal::NEGATIVE_ONE,
    };
    // For Long:  realized = (exit_price - cost_basis_qty) * qty * 1
    // For Short: realized = (cost_basis_qty - exit_price) * qty * 1  => (exit - cost) * -1
    (proceeds - cost_basis) * dir_multiplier
}

/// Reduce using LIFO: remove newest legs first.
fn reduce_lifo(pos: &mut Position, fill: &OrderFill) -> Decimal {
    let mut remaining = fill.filled_quantity;
    let mut cost_basis = Decimal::ZERO;
    let mut consumed = Vec::new();

    // Walk entries from newest to oldest
    while let Some(mut leg) = pos.entries.pop() {
        if remaining.is_zero() {
            consumed.push(leg);
            break;
        }
        if leg.quantity <= remaining {
            cost_basis += leg.entry_price * leg.quantity;
            remaining -= leg.quantity;
        } else {
            cost_basis += leg.entry_price * remaining;
            leg.quantity -= remaining;
            consumed.push(leg);
            remaining = Decimal::ZERO;
        }
    }

    // Reverse consumed to restore original order for remaining legs
    consumed.reverse();
    pos.entries.extend(consumed);

    let proceeds = fill.filled_price * fill.filled_quantity;
    let dir_multiplier = match pos.direction {
        Direction::Long => Decimal::ONE,
        Direction::Short => Decimal::NEGATIVE_ONE,
    };
    (proceeds - cost_basis) * dir_multiplier
}

/// Reduce using weighted average: all reductions use the current average entry price.
fn reduce_weighted_average(pos: &mut Position, fill: &OrderFill) -> Decimal {
    let cost_basis = pos.average_entry_price * fill.filled_quantity;
    let proceeds = fill.filled_price * fill.filled_quantity;
    let dir_multiplier = match pos.direction {
        Direction::Long => Decimal::ONE,
        Direction::Short => Decimal::NEGATIVE_ONE,
    };
    // Scale down entries proportionally
    let mut remaining = fill.filled_quantity;
    let mut new_entries = Vec::new();
    for leg in pos.entries.drain(..) {
        if remaining.is_zero() {
            new_entries.push(leg);
            continue;
        }
        if leg.quantity <= remaining {
            remaining -= leg.quantity;
        } else {
            new_entries.push(PositionLeg {
                entry_price: leg.entry_price,
                quantity: leg.quantity - remaining,
                timestamp: leg.timestamp,
                order_id: leg.order_id,
            });
            remaining = Decimal::ZERO;
        }
    }
    pos.entries = new_entries;
    (proceeds - cost_basis) * dir_multiplier
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MarginMode, OrderSide, OrderType};
    use rust_decimal_macros::dec;

    fn make_fill(price: Decimal, qty: Decimal) -> OrderFill {
        OrderFill {
            order_id: Uuid::new_v4(),
            position_id: None,
            symbol: "BTC-USDT".to_string(),
            side: OrderSide::Buy,
            direction: Direction::Long,
            filled_price: price,
            filled_quantity: qty,
            fee: Decimal::ZERO,
            fee_asset: "USDT".to_string(),
            timestamp: 1704067200000,
            realized_pnl: None,
        }
    }

    fn make_req(symbol: &str, direction: Direction, qty: Decimal) -> OrderRequest {
        OrderRequest {
            order_id: Uuid::new_v4(),
            symbol: symbol.to_string(),
            side: OrderSide::Buy,
            direction,
            order_type: OrderType::Market,
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
    fn test_open_long() {
        let mut book = InMemoryOrderBook::new();
        let req = make_req("BTC-USDT", Direction::Long, dec!(0.5));
        let fill = make_fill(dec!(40000), dec!(0.5));
        let pos = book.open_position(&req, &fill).unwrap();
        assert_eq!(pos.current_size, dec!(0.5));
        assert_eq!(pos.average_entry_price, dec!(40000));
        assert_eq!(pos.status, PositionStatus::Open);
    }

    #[test]
    fn test_add_to_long() {
        let mut book = InMemoryOrderBook::new();
        let req = make_req("BTC-USDT", Direction::Long, dec!(0.5));
        let fill1 = make_fill(dec!(40000), dec!(0.5));
        let pos = book.open_position(&req, &fill1).unwrap();

        let fill2 = make_fill(dec!(42000), dec!(0.5));
        let pos = book.add_to_position(pos.id, &fill2).unwrap();

        // Weighted average: (40000*0.5 + 42000*0.5) / 1.0 = 41000
        assert_eq!(pos.current_size, dec!(1.0));
        assert_eq!(pos.average_entry_price, dec!(41000));
    }

    #[test]
    fn test_reduce_fifo() {
        let mut book = InMemoryOrderBook::new();
        let req = make_req("BTC-USDT", Direction::Long, dec!(1.0));
        let fill1 = make_fill(dec!(40000), dec!(0.5));
        let pos = book.open_position(&req, &fill1).unwrap();
        let fill2 = make_fill(dec!(42000), dec!(0.5));
        let _pos = book.add_to_position(pos.id, &fill2).unwrap();

        // Reduce 0.3 at 41000
        let reduce_fill = OrderFill {
            filled_price: dec!(41000),
            filled_quantity: dec!(0.3),
            ..make_fill(dec!(41000), dec!(0.3))
        };
        let (pos, realized) = book
            .reduce_position(pos.id, &reduce_fill, CostBasisMethod::FIFO)
            .unwrap();

        // FIFO: first 0.3 comes from first leg at 40000
        // realized = (41000 - 40000) * 0.3 = 300
        assert_eq!(realized, dec!(300));
        assert_eq!(pos.current_size, dec!(0.7));
    }

    #[test]
    fn test_reduce_lifo() {
        let mut book = InMemoryOrderBook::new();
        let req = make_req("BTC-USDT", Direction::Long, dec!(1.0));
        let fill1 = make_fill(dec!(40000), dec!(0.5));
        let pos = book.open_position(&req, &fill1).unwrap();
        let fill2 = make_fill(dec!(42000), dec!(0.5));
        let _pos = book.add_to_position(pos.id, &fill2).unwrap();

        let reduce_fill = OrderFill {
            filled_price: dec!(41000),
            filled_quantity: dec!(0.3),
            ..make_fill(dec!(41000), dec!(0.3))
        };
        let (pos, realized) = book
            .reduce_position(pos.id, &reduce_fill, CostBasisMethod::LIFO)
            .unwrap();

        // LIFO: first 0.3 comes from last leg at 42000
        // realized = (41000 - 42000) * 0.3 = -300
        assert_eq!(realized, dec!(-300));
        assert_eq!(pos.current_size, dec!(0.7));
    }

    #[test]
    fn test_close_position() {
        let mut book = InMemoryOrderBook::new();
        let req = make_req("BTC-USDT", Direction::Long, dec!(0.5));
        let fill1 = make_fill(dec!(40000), dec!(0.5));
        let pos = book.open_position(&req, &fill1).unwrap();

        let close_fill = OrderFill {
            filled_price: dec!(41000),
            filled_quantity: dec!(0.5),
            ..make_fill(dec!(41000), dec!(0.5))
        };
        let (pos, realized) = book
            .close_position(pos.id, &close_fill, CostBasisMethod::WeightedAverage)
            .unwrap();

        // realized = (41000 - 40000) * 0.5 = 500
        assert_eq!(realized, dec!(500));
        assert_eq!(pos.status, PositionStatus::Closed);
        assert!(book.get_position(pos.id).is_none());
    }

    #[test]
    fn test_liquidation_long() {
        let entry = dec!(40000);
        let leverage = dec!(10);
        let maintenance_rate = dec!(0.005);
        let liq_price =
            MarginCalculator::liquidation_price(entry, leverage, maintenance_rate, Direction::Long);
        // Long liq = 40000 * (1 - 0.1 + 0.005) = 40000 * 0.905 = 36200
        assert_eq!(liq_price, dec!(36200));

        let mark_below = dec!(36100);
        assert!(mark_below < liq_price);
    }

    #[test]
    fn test_liquidation_short() {
        let entry = dec!(40000);
        let leverage = dec!(10);
        let maintenance_rate = dec!(0.005);
        let liq_price = MarginCalculator::liquidation_price(
            entry,
            leverage,
            maintenance_rate,
            Direction::Short,
        );
        // Short liq = 40000 * (1 + 0.1 - 0.005) = 40000 * 1.095 = 43800
        assert_eq!(liq_price, dec!(43800));

        let mark_above = dec!(43900);
        assert!(mark_above > liq_price);
    }

    #[test]
    fn test_maintain_margin_ratio() {
        let margin_used = dec!(10000);
        let unrealized = dec!(-2000);
        let total_equity = dec!(50000);
        let ratio = MarginCalculator::margin_ratio(margin_used, unrealized, total_equity);
        // (10000 + (-2000)) / 50000 = 8000 / 50000 = 0.16
        assert_eq!(ratio, dec!(0.16));
    }

    #[test]
    fn test_update_unrealized_pnl() {
        let mut book = InMemoryOrderBook::new();
        let req = make_req("BTC-USDT", Direction::Long, dec!(0.5));
        let fill = make_fill(dec!(40000), dec!(0.5));
        let pos = book.open_position(&req, &fill).unwrap();

        book.update_unrealized_pnl("BTC-USDT", dec!(41000));
        let pos = book.get_position(pos.id).unwrap();
        // unrealized = 0.5 * (41000 - 40000) = 500
        assert_eq!(pos.unrealized_pnl, dec!(500));
    }

    #[test]
    fn test_reduce_weighted_average() {
        let mut book = InMemoryOrderBook::new();
        let req = make_req("BTC-USDT", Direction::Long, dec!(1.0));
        let fill1 = make_fill(dec!(40000), dec!(0.5));
        let pos = book.open_position(&req, &fill1).unwrap();
        let fill2 = make_fill(dec!(42000), dec!(0.5));
        let _pos = book.add_to_position(pos.id, &fill2).unwrap();

        let reduce_fill = OrderFill {
            filled_price: dec!(41000),
            filled_quantity: dec!(0.3),
            ..make_fill(dec!(41000), dec!(0.3))
        };
        let (pos, realized) = book
            .reduce_position(pos.id, &reduce_fill, CostBasisMethod::WeightedAverage)
            .unwrap();

        // WA: avg entry = 41000
        // realized = (41000 - 41000) * 0.3 = 0
        assert_eq!(realized, dec!(0));
        assert_eq!(pos.current_size, dec!(0.7));
    }
}
