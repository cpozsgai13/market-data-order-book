/// Trade representation — mirrors `Trade.h` / `Trade.cpp`.
use std::fmt;

use crate::types::{OrderId, Quantity};

// ── TradeSide ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TradeSide<P> {
    pub order_id: OrderId,
    pub price: P,
    pub quantity: Quantity,
}

// ── Trade ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Trade<P> {
    pub bid_side: TradeSide<P>,
    pub ask_side: TradeSide<P>,
}

impl<P: fmt::Display> fmt::Display for Trade<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TRADE: Bid({} {} {}), Ask({} {} {})",
            self.bid_side.order_id,
            self.bid_side.price,
            self.bid_side.quantity,
            self.ask_side.order_id,
            self.ask_side.price,
            self.ask_side.quantity,
        )
    }
}
