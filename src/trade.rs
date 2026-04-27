/// Trade representation — mirrors `Trade.h` / `Trade.cpp`.
use std::fmt;

use crate::price::Price;
use crate::types::{OrderId, Quantity};

// ── TradeSide ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TradeSide {
    pub order_id: OrderId,
    pub price: Price,
    pub quantity: Quantity,
}

// ── Trade ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Trade {
    pub bid_side: TradeSide,
    pub ask_side: TradeSide,
}

impl fmt::Display for Trade {
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
