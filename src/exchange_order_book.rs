/// Exchange-level container that maps instrument ids → individual order books.
///
/// Mirrors `ExchangeOrderBook.h` / `ExchangeOrderBook.cpp`.
use std::collections::HashMap;

use crate::messages::{AddOrderMsg, CancelOrderMsg, ModifyOrderMsg, SymbolMsg};
use crate::order::new_order_ptr;
use crate::order_book::OrderBook;
use crate::types::{InstrumentId, Timestamp};

// ── ExchangeOrderBook ─────────────────────────────────────────────────────────

pub struct ExchangeOrderBook {
    exchange_name: String,
    /// instrument id → order book
    instrument_map: HashMap<InstrumentId, OrderBook>,
    /// symbol name → instrument id
    symbol_map: HashMap<String, InstrumentId>,
}

impl ExchangeOrderBook {
    pub fn new(name: impl Into<String>) -> Self {
        ExchangeOrderBook {
            exchange_name: name.into(),
            instrument_map: HashMap::new(),
            symbol_map: HashMap::new(),
        }
    }

    /// Register (or re-register) a symbol/instrument.
    pub fn add_update_symbol(&mut self, msg: &SymbolMsg) {
        let book = OrderBook::new(msg.symbol.clone());
        self.instrument_map.insert(msg.instrument_id, book);
        self.symbol_map.insert(msg.symbol.clone(), msg.instrument_id);
    }

    /// Submit a new order.  Uses `now()` as the creation timestamp to mirror
    /// the C++ `ExchangeOrderBook::AddNewOrder` behaviour.
    pub fn add_new_order(&mut self, msg: &AddOrderMsg) -> bool {
        match self.instrument_map.get_mut(&msg.instrument_id) {
            None => false,
            Some(book) => {
                let now = Self::now_ns();
                let order = new_order_ptr(
                    msg.order_type,
                    msg.side,
                    msg.order_id,
                    msg.price,
                    msg.quantity,
                    now,
                );
                book.add_order(order)
            }
        }
    }

    /// Modify a resting order.
    pub fn update_order(&mut self, msg: &ModifyOrderMsg) -> bool {
        match self.instrument_map.get_mut(&msg.instrument_id) {
            None       => false,
            Some(book) => book.update_order(msg),
        }
    }

    /// Cancel a resting order.
    pub fn cancel_order(&mut self, msg: &CancelOrderMsg) -> bool {
        match self.instrument_map.get_mut(&msg.instrument_id) {
            None       => false,
            Some(book) => book.cancel_order(msg.order_id),
        }
    }

    /// Print the order book for a specific symbol.
    pub fn print_book(&self, symbol: &str) {
        match self.symbol_map.get(symbol) {
            None => println!("Symbol not found: {}", symbol),
            Some(&id) => match self.instrument_map.get(&id) {
                None       => println!("No book for {}", symbol),
                Some(book) => book.print(),
            },
        }
    }

    /// Print all order books.
    pub fn print_all_books(&self) {
        // Sort by instrument id for deterministic output.
        let mut syms: Vec<(&String, &InstrumentId)> = self.symbol_map.iter().collect();
        syms.sort_by_key(|(_, &id)| id);

        for (symbol, &id) in syms {
            println!("=== Order Book: {} (id={}) ===", symbol, id);
            if let Some(book) = self.instrument_map.get(&id) {
                book.print();
            }
            println!();
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Nanoseconds since the Unix epoch, used as an order creation timestamp.
    fn now_ns() -> Timestamp {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as Timestamp)
            .unwrap_or(0)
    }
}
