/// Canonical generic message types for the market data order book.
///
/// All structs are generic over the price type `P`, allowing the same pipeline
/// to be instantiated at Price4, Price6, or Price9 without any code duplication.
///
/// For network / wire-format work import the precision-specific codec from
/// `crate::precision_codec::price6` (or price4 / price9) directly.
use crate::types::{InstrumentId, OrderId, OrderType, Quantity, Side, Timestamp};

// ── Generic message structs ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SymbolMsg<P> {
    pub symbol:        String,
    pub instrument_id: InstrumentId,
    pub last_price:    P,
}

#[derive(Debug, Clone)]
pub struct AddOrderMsg<P> {
    pub instrument_id:  InstrumentId,
    pub order_id:       OrderId,
    pub price:          P,
    pub quantity:       Quantity,
    pub side:           Side,
    pub order_type:     OrderType,
    pub update_time_ns: Timestamp,
}

#[derive(Debug, Clone)]
pub struct ModifyOrderMsg<P> {
    pub order_id:       OrderId,
    pub instrument_id:  InstrumentId,
    pub price:          P,
    pub side:           Side,
    pub quantity:       Quantity,
    pub update_time_ns: Timestamp,
}

#[derive(Debug, Clone)]
pub struct CancelOrderMsg {
    pub order_id:      OrderId,
    pub instrument_id: InstrumentId,
}

#[derive(Debug, Clone)]
pub enum CoreMessage<P> {
    Symbol(SymbolMsg<P>),
    AddOrder(AddOrderMsg<P>),
    ModifyOrder(ModifyOrderMsg<P>),
    CancelOrder(CancelOrderMsg),
}

// ── Generic packet ────────────────────────────────────────────────────────────

/// An ordered collection of `CoreMessage<P>`.
/// In network mode P is fixed to the wire precision; in file mode P is chosen
/// per run (Price4 / Price6 / Price9).
#[derive(Debug, Clone, Default)]
pub struct Packet<P> {
    pub messages: Vec<CoreMessage<P>>,
}

impl<P> Packet<P> {
    pub fn new() -> Self { Packet { messages: Vec::new() } }
    pub fn push(&mut self, msg: CoreMessage<P>) { self.messages.push(msg); }
    pub fn is_empty(&self) -> bool { self.messages.is_empty() }
    pub fn len(&self) -> usize { self.messages.len() }
}

// ── Price type re-exports ─────────────────────────────────────────────────────

#[allow(unused_imports)]
pub use crate::price::{Price4, Price6, Price9};

/// Default price precision (6 decimal places — equities / crypto standard).
/// Used by network mode and wherever a single precision is needed.
pub type Price = Price6;
