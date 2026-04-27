/// Message types that flow through the system.
///
/// Mirrors the structs in `CoreMessages.h` (Symbol, AddOrder, ModifyOrder,
/// CancelOrder) and the `DataType` discriminant.
use crate::price::Price;
use crate::types::{InstrumentId, OrderId, OrderType, Quantity, Side, Timestamp};

// ── Symbol ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SymbolMsg {
    pub symbol: String,
    pub instrument_id: InstrumentId,
    pub last_price: Price,
}

// ── AddOrder ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AddOrderMsg {
    pub instrument_id: InstrumentId,
    pub order_id: OrderId,
    pub price: Price,
    pub quantity: Quantity,
    pub side: Side,
    pub order_type: OrderType,
    pub update_time_ns: Timestamp,
}

// ── ModifyOrder ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ModifyOrderMsg {
    pub order_id: OrderId,
    pub instrument_id: InstrumentId,
    pub price: Price,
    pub side: Side,
    pub quantity: Quantity,
    pub update_time_ns: Timestamp,
}

// ── CancelOrder ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CancelOrderMsg {
    pub order_id: OrderId,
    pub instrument_id: InstrumentId,
}

// ── CoreMessage ───────────────────────────────────────────────────────────────

/// Tagged union of all message variants — mirrors the `CoreMessage` struct with
/// a `DataType` discriminant in the C++ code-base.
#[derive(Debug, Clone)]
pub enum CoreMessage {
    Symbol(SymbolMsg),
    AddOrder(AddOrderMsg),
    ModifyOrder(ModifyOrderMsg),
    CancelOrder(CancelOrderMsg),
}

// ── Packet ────────────────────────────────────────────────────────────────────

/// A group of `CoreMessage`s transported together over TCP.
///
/// Mirrors the C++ `Packet` struct (Header + message array) but uses an owned
/// `Vec` rather than a fixed-size array so it can be moved across thread
/// boundaries without copying.
#[derive(Debug, Clone, Default)]
pub struct Packet {
    pub messages: Vec<CoreMessage>,
}

impl Packet {
    pub fn new() -> Self {
        Packet { messages: Vec::new() }
    }

    pub fn push(&mut self, msg: CoreMessage) {
        self.messages.push(msg);
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }
}
