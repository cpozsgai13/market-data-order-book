/// Single-symbol limit order book with price-time-priority matching.
///
/// Mirrors `OrderBook.h` / `OrderBook.cpp`.
///
/// Data-structure mapping (C++ → Rust):
///   `std::map<Price, CustomOrderQueue, std::greater<Price>>` (bids, descending)
///     → `BTreeMap<Price, OrderQueue>`, iterated in *reverse* for best bid.
///   `std::map<Price, CustomOrderQueue, std::less<Price>>` (asks, ascending)
///     → `BTreeMap<Price, OrderQueue>`, iterated *forward* for best ask.
///   `std::unordered_map<OrderID, OrderPtr>` → `HashMap<OrderId, OrderPtr>`
///   `std::unordered_map<Price, Volume>` → `HashMap<Price, Volume>`
use std::collections::{BTreeMap, HashMap};
use std::fmt;

use crate::price_trait::FixedPrecisionPriceLike;

use crate::messages::ModifyOrderMsg;
use crate::order::{new_order_ptr, OrderPtr};
use crate::order_queue::OrderQueue;
// Price type is now generic; import the desired type in your module.
use crate::trade::{Trade, TradeSide};
use crate::types::{OrderId, OrderType, Side, Volume};

// ── OrderBook ─────────────────────────────────────────────────────────────────

pub struct OrderBook<P>
where
    P: crate::price_trait::FixedPrecisionPriceLike,
{
    symbol: String,
    /// All live orders indexed by id.
    order_map: HashMap<OrderId, crate::order::OrderPtr<P>>,
    /// Bid price levels (ascending key; iterate in reverse for best bid).
    bid_queue_map: BTreeMap<P, OrderQueue<P>>,
    /// Ask price levels (ascending key; first entry is best ask).
    ask_queue_map: BTreeMap<P, OrderQueue<P>>,
    /// Aggregated visible volume per bid price level.
    bid_volume_map: HashMap<P, Volume>,
    /// Aggregated visible volume per ask price level.
    ask_volume_map: HashMap<P, Volume>,
    /// All trades executed by this book (newest last).
    pub trades: Vec<Trade<P>>,
}

impl<P> OrderBook<P>
where
    P: crate::price_trait::FixedPrecisionPriceLike,
{
    pub fn new(symbol: impl Into<String>) -> Self {
        OrderBook {
            symbol: symbol.into(),
            order_map: HashMap::new(),
            bid_queue_map: BTreeMap::new(),
            ask_queue_map: BTreeMap::new(),
            bid_volume_map: HashMap::new(),
            ask_volume_map: HashMap::new(),
            trades: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.order_map.is_empty()
    }

    // ── Public API ────────────────────────────────────────────────────────────

    /// Add a new order.  IOC orders are matched immediately and never rest in
    /// the book; GFD orders rest and trigger a sweep of matchable levels.
    pub fn add_order(&mut self, order: OrderPtr<P>) -> bool {
        let order_id = order.lock().unwrap().order_id();

        // Duplicate check.
        if self.order_map.contains_key(&order_id) {
            return false;
        }

        let guard = order.lock().unwrap();
        let price = guard.price();
        let qty   = guard.initial_quantity();
        let otype = guard.order_type();
        let side  = guard.side();
        drop(guard);

        // IOC: only add if there is a match right now.
        if otype == OrderType::Ioc {
            if !self.can_match(side, price) {
                return false;
            }
            self.match_ioc_order(order);
            return true;
        }

        // GFD / Market: rest the order in the book, then sweep.
        match side {
            Side::Bid => {
                let queue = self.bid_queue_map.entry(price).or_insert_with(OrderQueue::new);
                queue.push_back(order.clone());
                *self.bid_volume_map.entry(price).or_insert(0) += qty;
            }
            Side::Ask => {
                let queue = self.ask_queue_map.entry(price).or_insert_with(OrderQueue::new);
                queue.push_back(order.clone());
                *self.ask_volume_map.entry(price).or_insert(0) += qty;
            }
            Side::Invalid => return false,
        }

        self.order_map.insert(order_id, order);
        self.match_orders();
        // Debug: print the book after each new order
        println!("[OrderBook] After add_order:");
        println!("{}", self);
        true
    }

    /// Cancel a resting order by id.
    pub fn cancel_order(&mut self, order_id: OrderId) -> bool {
        let order = match self.order_map.get(&order_id) {
            Some(o) => o.clone(),
            None    => return false,
        };

        let guard = order.lock().unwrap();
        let side  = guard.side();
        let price = guard.price();
        let qty   = guard.remaining_quantity();
        drop(guard);

        match side {
            Side::Bid => {
                // Reduce volume; remove level if drained.
                if let Some(vol) = self.bid_volume_map.get_mut(&price) {
                    *vol = vol.saturating_sub(qty);
                    if *vol == 0 { self.bid_volume_map.remove(&price); }
                }
                if let Some(queue) = self.bid_queue_map.get_mut(&price) {
                    queue.erase(order_id);
                    if queue.is_empty() { self.bid_queue_map.remove(&price); }
                }
            }
            Side::Ask => {
                if let Some(vol) = self.ask_volume_map.get_mut(&price) {
                    *vol = vol.saturating_sub(qty);
                    if *vol == 0 { self.ask_volume_map.remove(&price); }
                }
                if let Some(queue) = self.ask_queue_map.get_mut(&price) {
                    queue.erase(order_id);
                    if queue.is_empty() { self.ask_queue_map.remove(&price); }
                }
            }
            Side::Invalid => {}
        }

        self.order_map.remove(&order_id);
        true
    }

    /// Modify a resting order: cancel it and re-add with the new attributes
    /// while preserving its original `OrderType`.
    pub fn update_order(&mut self, modify: &ModifyOrderMsg<P>) -> bool {
        let orig_type = match self.order_map.get(&modify.order_id) {
            Some(o) => o.lock().unwrap().order_type(),
            None    => return false,
        };
        self.cancel_order(modify.order_id);

        let replacement = new_order_ptr(
            orig_type,
            modify.side,
            modify.order_id,
            modify.price,
            modify.quantity,
            modify.update_time_ns,
        );
        self.add_order(replacement)
    }

    /// Return the aggregated visible volume at a price level on the given side.
    pub fn volume_at_price(&self, price: P, side: Side) -> Volume {
        match side {
            Side::Bid => self.bid_volume_map.get(&price).copied().unwrap_or(0),
            Side::Ask => self.ask_volume_map.get(&price).copied().unwrap_or(0),
            Side::Invalid => 0,
        }
    }

    /// Return the best bid price (highest), if any resting bid exists.
    pub fn best_bid(&self) -> Option<P> {
        self.bid_queue_map.keys().next_back().copied()
    }

    /// Return the best ask price (lowest), if any resting ask exists.
    pub fn best_ask(&self) -> Option<P> {
        self.ask_queue_map.keys().next().copied()
    }

    /// Return the best bid price and its volume.
    pub fn best_bid_with_volume(&self) -> Option<(P, Volume)> {
        self.bid_queue_map.keys().next_back().map(|p| {
            let vol = self.bid_volume_map.get(p).copied().unwrap_or(0);
            (*p, vol)
        })
    }

    /// Return the best ask price and its volume.
    pub fn best_ask_with_volume(&self) -> Option<(P, Volume)> {
        self.ask_queue_map.keys().next().map(|p| {
            let vol = self.ask_volume_map.get(p).copied().unwrap_or(0);
            (*p, vol)
        })
    }

    pub fn print(&self) {
        println!("{}", self);
    }

    // ── Private matching ──────────────────────────────────────────────────────

    /// `true` if an order on `side` at `price` can be matched immediately.
    fn can_match(&self, side: Side, price: P) -> bool {
        match side {
            Side::Bid => {
                // Can match if best ask ≤ order price.
                self.ask_queue_map
                    .keys()
                    .next()
                    .map_or(false, |&best_ask| price >= best_ask)
            }
            Side::Ask => {
                // Can match if best bid ≥ order price.
                self.bid_queue_map
                    .keys()
                    .next_back()
                    .map_or(false, |&best_bid| price <= best_bid)
            }
            Side::Invalid => false,
        }
    }

    /// Sweep all matchable bid/ask price levels (GFD matching sweep).
    fn match_orders(&mut self) {
        loop {
            // Identify best prices using immutable peeks first.
            let best_bid = self.bid_queue_map.keys().next_back().copied();
            let best_ask = self.ask_queue_map.keys().next().copied();

            let (bid_price, ask_price) = match (best_bid, best_ask) {
                (Some(b), Some(a)) if b >= a => (b, a),
                _ => break,
            };

            // Inner loop: drain the two queues at these price levels.
            loop {
                let bid_ptr = self.bid_queue_map.get_mut(&bid_price).and_then(|q| q.front());
                let ask_ptr = self.ask_queue_map.get_mut(&ask_price).and_then(|q| q.front());

                match (bid_ptr, ask_ptr) {
                    (Some(bid), Some(ask)) => {
                        let bid_qty = bid.lock().unwrap().remaining_quantity();
                        let ask_qty = ask.lock().unwrap().remaining_quantity();
                        let q = bid_qty.min(ask_qty);

                        if q == 0 {
                            panic!("match_orders: trade quantity is zero");
                        }

                        let bid_id    = bid.lock().unwrap().order_id();
                        let ask_id    = ask.lock().unwrap().order_id();
                        let bid_price_t = bid.lock().unwrap().price();
                        let ask_price_t = ask.lock().unwrap().price();

                        self.trades.push(Trade {
                            bid_side: TradeSide { order_id: bid_id, price: bid_price_t, quantity: q },
                            ask_side: TradeSide { order_id: ask_id, price: ask_price_t, quantity: q },
                        });

                        bid.lock().unwrap().fill(q);
                        ask.lock().unwrap().fill(q);

                        let bid_filled = bid.lock().unwrap().is_filled();
                        let ask_filled = ask.lock().unwrap().is_filled();

                        // Update ask volume and remove if filled.
                        if let Some(v) = self.ask_volume_map.get_mut(&ask_price) {
                            *v = v.saturating_sub(q);
                        }
                        if ask_filled {
                            if let Some(queue) = self.ask_queue_map.get_mut(&ask_price) {
                                queue.pop_front();
                            }
                            self.order_map.remove(&ask_id);
                            if self.ask_volume_map.get(&ask_price) == Some(&0) {
                                self.ask_volume_map.remove(&ask_price);
                            }
                        }

                        // Update bid volume and remove if filled.
                        if let Some(v) = self.bid_volume_map.get_mut(&bid_price) {
                            *v = v.saturating_sub(q);
                        }
                        if bid_filled {
                            if let Some(queue) = self.bid_queue_map.get_mut(&bid_price) {
                                queue.pop_front();
                            }
                            self.order_map.remove(&bid_id);
                            if self.bid_volume_map.get(&bid_price) == Some(&0) {
                                self.bid_volume_map.remove(&bid_price);
                            }
                        }
                    }
                    _ => break, // one side has no more orders at this level
                }

                // Stop inner loop if either price level is now empty.
                let bid_empty = self.bid_queue_map.get(&bid_price).map_or(true, |q| q.is_empty());
                let ask_empty = self.ask_queue_map.get(&ask_price).map_or(true, |q| q.is_empty());
                if bid_empty || ask_empty {
                    break;
                }
            }

            // Clean up empty price levels.
            if self.bid_queue_map.get(&bid_price).map_or(true, |q| q.is_empty()) {
                self.bid_queue_map.remove(&bid_price);
            }
            if self.ask_queue_map.get(&ask_price).map_or(true, |q| q.is_empty()) {
                self.ask_queue_map.remove(&ask_price);
            }
        }
    }

    /// Immediate-or-Cancel matching: fill as much of `order` as possible right
    /// now; any unfilled remainder is silently discarded.
    fn match_ioc_order(&mut self, order: OrderPtr<P>) {
        let otype = {
            let order_guard = order.lock().unwrap();
            order_guard.order_type()
        };
        if otype != OrderType::Ioc {
            return;
        }

        let order_price = {
            let order_guard = order.lock().unwrap();
            order_guard.price()
        };

        loop {
            let is_filled = {
                let order_guard = order.lock().unwrap();
                order_guard.is_filled()
            };
            if is_filled { break; }

            let side = {
                let order_guard = order.lock().unwrap();
                order_guard.side()
            };

            match side {
                Side::Bid => {
                    // Match against the best ask.
                    let best_ask = self.ask_queue_map.keys().next().copied();
                    let best_ask = match best_ask {
                        Some(p) if order_price >= p => p,
                        _ => break,
                    };

                    loop {
                        let is_filled = {
                            let order_guard = order.lock().unwrap();
                            order_guard.is_filled()
                        };
                        if is_filled { break; }

                        let ask_ptr = self.ask_queue_map.get_mut(&best_ask).and_then(|q| q.front());
                        let ask = match ask_ptr {
                            Some(a) => a,
                            None    => break,
                        };

                        // Lock both ask and order once per loop
                        let (ask_qty, _ask_filled, ask_id, ask_price_val) = {
                            let ask_guard = ask.lock().unwrap();
                            (ask_guard.remaining_quantity(), ask_guard.is_filled(), ask_guard.order_id(), ask_guard.price())
                        };
                        let (order_qty, order_id_val) = {
                            let order_guard = order.lock().unwrap();
                            (order_guard.remaining_quantity(), order_guard.order_id())
                        };
                        let qty = ask_qty.min(order_qty);

                        {
                            let mut ask_guard = ask.lock().unwrap();
                            ask_guard.fill(qty);
                        }
                        {
                            let mut order_guard = order.lock().unwrap();
                            order_guard.fill(qty);
                        }

                        self.trades.push(Trade {
                            bid_side: TradeSide {
                                order_id: order_id_val,
                                price: order_price,
                                quantity: qty,
                            },
                            ask_side: TradeSide {
                                order_id: ask_id,
                                price: ask_price_val,
                                quantity: qty,
                            },
                        });

                        let ask_is_filled = {
                            let ask_guard = ask.lock().unwrap();
                            ask_guard.is_filled()
                        };
                        if ask_is_filled {
                            let ask_id = {
                                let ask_guard = ask.lock().unwrap();
                                ask_guard.order_id()
                            };
                            if let Some(queue) = self.ask_queue_map.get_mut(&best_ask) {
                                queue.pop_front();
                            }
                            self.order_map.remove(&ask_id);
                        }
                    }
                    if self.ask_queue_map.get(&best_ask).map_or(true, |q| q.is_empty()) {
                        self.ask_queue_map.remove(&best_ask);
                    }
                }

                Side::Ask => {
                    // Match against the best bid.
                    let best_bid = self.bid_queue_map.keys().next_back().copied();
                    let best_bid = match best_bid {
                        Some(p) if order_price <= p => p,
                        _ => break,
                    };

                    loop {
                        if order.lock().unwrap().is_filled() { break; }
                        let bid_ptr = self.bid_queue_map.get_mut(&best_bid).and_then(|q| q.front());
                        let bid = match bid_ptr {
                            Some(b) => b,
                            None    => break,
                        };

                        let bid_qty   = bid.lock().unwrap().remaining_quantity();
                        let order_qty = order.lock().unwrap().remaining_quantity();
                        let qty       = bid_qty.min(order_qty);

                        bid.lock().unwrap().fill(qty);
                        order.lock().unwrap().fill(qty);

                        self.trades.push(Trade {
                            bid_side: TradeSide {
                                order_id: bid.lock().unwrap().order_id(),
                                price: bid.lock().unwrap().price(),
                                quantity: qty,
                            },
                            ask_side: TradeSide {
                                order_id: order.lock().unwrap().order_id(),
                                price: order_price,
                                quantity: qty,
                            },
                        });

                        if bid.lock().unwrap().is_filled() {
                            let bid_id = bid.lock().unwrap().order_id();
                            if let Some(queue) = self.bid_queue_map.get_mut(&best_bid) {
                                queue.pop_front();
                            }
                            self.order_map.remove(&bid_id);
                        }
                    }
                    if self.bid_queue_map.get(&best_bid).map_or(true, |q| q.is_empty()) {
                        self.bid_queue_map.remove(&best_bid);
                    }
                }

                Side::Invalid => break,
            }
        }
    }
}

// ── Display ───────────────────────────────────────────────────────────────────

impl<P> fmt::Display for OrderBook<P>
where
    P: FixedPrecisionPriceLike,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const VOL_W: usize   = 6;
        const PRICE_W: usize = 9;

        writeln!(f, "{:^20}", self.symbol)?;

        let bid_levels = self.bid_queue_map.len();
        let ask_levels = self.ask_queue_map.len();
        let depth      = bid_levels.max(ask_levels);

        if depth == 0 {
            return writeln!(f, "{:^20}", "Empty book");
        }

        writeln!(
            f,
            "  {:>VOL_W$}  {:>PRICE_W$} | {:<PRICE_W$}  {:<VOL_W$}",
            "BID VOL", "PRICE", "PRICE", "ASK VOL",
            VOL_W = VOL_W, PRICE_W = PRICE_W,
        )?;

        // Bids iterate highest-to-lowest (reverse BTreeMap).
        let bids: Vec<(P, Volume)> = self
            .bid_queue_map
            .iter()
            .rev()
            .map(|(p, _)| (*p, self.bid_volume_map.get(p).copied().unwrap_or(0)))
            .collect();

        // Asks iterate lowest-to-highest (natural BTreeMap order).
        let asks: Vec<(P, Volume)> = self
            .ask_queue_map
            .iter()
            .map(|(p, _)| (*p, self.ask_volume_map.get(p).copied().unwrap_or(0)))
            .collect();

        for i in 0..depth {
            // Bid side
            if let Some((price, vol)) = bids.get(i) {
                write!(f, "  {:>VOL_W$}  {:>PRICE_W$}", vol, price.to_string(),
                       VOL_W = VOL_W, PRICE_W = PRICE_W)?;
            } else {
                write!(f, "  {:>VOL_W$}  {:>PRICE_W$}", "", "",
                       VOL_W = VOL_W, PRICE_W = PRICE_W)?;
            }
            write!(f, " | ")?;
            // Ask side
            if let Some((price, vol)) = asks.get(i) {
                writeln!(f, "{:<PRICE_W$}  {:<VOL_W$}", price.to_string(), vol,
                         VOL_W = VOL_W, PRICE_W = PRICE_W)?;
            } else {
                writeln!(f, "{:<PRICE_W$}  {:<VOL_W$}", "", "",
                         VOL_W = VOL_W, PRICE_W = PRICE_W)?;
            }
        }

        Ok(())
    }
}
