/// Order and its shared-ownership pointer type.
///
/// Mirrors `Order` from `Order.h` / `Order.cpp`.
use std::sync::{Arc, Mutex};

use crate::price::Price;
use crate::types::{OrderId, OrderType, Quantity, Side, Timestamp};

// ── Order ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Order {
    order_type: OrderType,
    side: Side,
    order_id: OrderId,
    price: Price,
    initial_quantity: Quantity,
    remaining_quantity: Quantity,
    creation_time_ns: Timestamp,
}

impl Order {
    pub fn new(
        order_type: OrderType,
        side: Side,
        order_id: OrderId,
        price: Price,
        quantity: Quantity,
        creation_time_ns: Timestamp,
    ) -> Self {
        Order {
            order_type,
            side,
            order_id,
            price,
            initial_quantity: quantity,
            remaining_quantity: quantity,
            creation_time_ns,
        }
    }

    // ── Accessors ──────────────────────────────────────────────────────────────

    #[inline] pub fn order_type(&self)          -> OrderType { self.order_type }
    #[inline] pub fn side(&self)                -> Side      { self.side }
    #[inline] pub fn order_id(&self)            -> OrderId   { self.order_id }
    #[inline] pub fn price(&self)               -> Price     { self.price }
    #[inline] pub fn initial_quantity(&self)    -> Quantity  { self.initial_quantity }
    #[inline] pub fn remaining_quantity(&self)  -> Quantity  { self.remaining_quantity }
    #[inline] pub fn creation_time(&self)       -> Timestamp { self.creation_time_ns }

    #[inline]
    pub fn filled_quantity(&self) -> Quantity {
        self.initial_quantity - self.remaining_quantity
    }

    // ── Mutation ───────────────────────────────────────────────────────────────

    /// Reduce remaining quantity by `q`.  Returns `false` if `q` exceeds what
    /// is left (the fill is rejected and no state is changed).
    pub fn fill(&mut self, q: Quantity) -> bool {
        if q > self.remaining_quantity {
            return false;
        }
        self.remaining_quantity -= q;
        true
    }

    /// Returns `true` when there is no remaining quantity.
    #[inline]
    pub fn is_filled(&self) -> bool {
        self.remaining_quantity == 0
    }
}

// ── Shared pointer alias ──────────────────────────────────────────────────────

/// `Arc<Mutex<Order>>` — thread-safe shared ownership with interior mutability.
/// This differs from the original C++ `shared_ptr<Order>` only in Rust's
/// synchronization primitive choice so that the order book can move across
/// worker threads safely.
pub type OrderPtr = Arc<Mutex<Order>>;

/// Convenience constructor that wraps a new `Order` in `Rc<RefCell<>>`.
pub fn new_order_ptr(
    order_type: OrderType,
    side: Side,
    order_id: OrderId,
    price: Price,
    quantity: Quantity,
    creation_time_ns: Timestamp,
) -> OrderPtr {
    Arc::new(Mutex::new(Order::new(
        order_type,
        side,
        order_id,
        price,
        quantity,
        creation_time_ns,
    )))
}
