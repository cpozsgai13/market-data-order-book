/// Priority queue of orders sorted by `OrderId` (min-heap → earliest order first).
///
/// Mirrors `CustomOrderQueue` from `OrderBook.h`.  The C++ version uses a
/// `std::priority_queue` with lazy deletion backed by an `unordered_map`.
/// This Rust version does the same:
///   - `BinaryHeap<Reverse<OrderId>>` provides O(log n) insert / peek / pop.
///   - `HashMap<OrderId, OrderPtr>` tracks which ids are still live.
///   - Cancelled / filled orders are removed from the map; the heap cleans up
///     stale entries lazily the next time `front()` / `pop_front()` is called.
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use crate::order::OrderPtr;
use crate::types::OrderId;

// ── OrderQueue ────────────────────────────────────────────────────────────────

pub struct OrderQueue {
    heap: BinaryHeap<Reverse<OrderId>>,
    order_map: HashMap<OrderId, OrderPtr>,
}

impl Default for OrderQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderQueue {
    pub fn new() -> Self {
        OrderQueue {
            heap: BinaryHeap::new(),
            order_map: HashMap::new(),
        }
    }

    /// Insert an order.  O(log n).
    pub fn push_back(&mut self, order: OrderPtr) {
        let id = order.lock().unwrap().order_id();
        self.heap.push(Reverse(id));
        self.order_map.insert(id, order);
    }

    /// Lazily remove an order by id.  O(1) (the heap slot is cleaned up later).
    pub fn erase(&mut self, order_id: OrderId) {
        self.order_map.remove(&order_id);
    }

    /// `true` when no live orders remain.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.order_map.is_empty()
    }

    /// Peek at the order with the smallest `OrderId` (FIFO front).
    ///
    /// Pops and discards any stale heap entries (those already removed from
    /// the map) before returning.  Returns `None` on an empty queue.
    pub fn front(&mut self) -> Option<OrderPtr> {
        while let Some(&Reverse(id)) = self.heap.peek() {
            if self.order_map.contains_key(&id) {
                return self.order_map.get(&id).cloned();
            }
            self.heap.pop(); // lazy-delete stale entry
        }
        None
    }

    /// Remove the front order (smallest `OrderId`) from both the heap and the
    /// map.  No-op on an empty queue.
    pub fn pop_front(&mut self) {
        while let Some(Reverse(id)) = self.heap.pop() {
            if self.order_map.remove(&id).is_some() {
                return; // successfully popped a live entry
            }
            // id was already lazy-deleted; keep popping
        }
    }
}
