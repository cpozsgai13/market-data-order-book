impl<T> Clone for SpscConsumer<T> {
    fn clone(&self) -> Self {
        SpscConsumer { inner: Arc::clone(&self.inner) }
    }
}
/// Lock-free single-producer / single-consumer ring buffer.
///
/// Mirrors `ring_buffer_spsc.hpp` from the C++ code-base.
///
/// The implementation uses two `AtomicUsize` indices (head = read cursor,
/// tail = write cursor) and a `Vec<UnsafeCell<Option<T>>>` data array.
/// Only one thread may call `push` and only one thread may call `pop` — the
/// "SPSC contract".  The split `SpscProducer` / `SpscConsumer` handles enforce
/// this at the type level.
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// ── Inner buffer ──────────────────────────────────────────────────────────────

struct Inner<T> {
    head:     AtomicUsize, // consumer cursor (read position)
    tail:     AtomicUsize, // producer cursor (write position)
    data:     Vec<UnsafeCell<Option<T>>>,
    capacity: usize,
}

// Safety: only one thread writes to data[tail] and only one reads data[head].
// Because head != tail (full/empty checks ensure no overlap), the two threads
// never access the same slot concurrently.
unsafe impl<T: Send> Send for Inner<T> {}
unsafe impl<T: Send> Sync for Inner<T> {}

impl<T> Inner<T> {
    fn new(capacity: usize) -> Self {
        assert!(capacity >= 2, "SPSC capacity must be >= 2");
        let mut data = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            data.push(UnsafeCell::new(None));
        }
        Inner {
            head:     AtomicUsize::new(0),
            tail:     AtomicUsize::new(0),
            data,
            capacity,
        }
    }

    fn push(&self, value: T) -> bool {
        let tail      = self.tail.load(Ordering::Relaxed);
        let next_tail = (tail + 1) % self.capacity;

        // Full check: if the next write position == read position, buffer is full.
        if next_tail == self.head.load(Ordering::Acquire) {
            return false;
        }

        // Safety: only the producer accesses data[tail].
        unsafe { *self.data[tail].get() = Some(value); }

        self.tail.store(next_tail, Ordering::Release);
        true
    }

    fn pop(&self) -> Option<T> {
        let head = self.head.load(Ordering::Relaxed);

        // Empty check.
        if head == self.tail.load(Ordering::Acquire) {
            return None;
        }

        // Safety: only the consumer accesses data[head].
        let value = unsafe { (*self.data[head].get()).take() };

        self.head.store((head + 1) % self.capacity, Ordering::Release);
        value
    }

    fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }
}

// ── Public split handles ──────────────────────────────────────────────────────

/// The producer (write) end of a SPSC ring buffer.
pub struct SpscProducer<T> {
    inner: Arc<Inner<T>>,
}

/// The consumer (read) end of a SPSC ring buffer.
pub struct SpscConsumer<T> {
    inner: Arc<Inner<T>>,
}

// SpscProducer is Send because it has exclusive write access to the Arc.
unsafe impl<T: Send> Send for SpscProducer<T> {}
// SpscConsumer is Send because it has exclusive read access to the Arc.
unsafe impl<T: Send> Send for SpscConsumer<T> {}

impl<T> SpscProducer<T> {
    /// Push a value.  Returns `false` if the ring buffer is full (non-blocking).
    #[inline]
    pub fn push(&self, value: T) -> bool {
        self.inner.push(value)
    }
}

impl<T> SpscConsumer<T> {
    /// Pop a value.  Returns `None` if the ring buffer is empty (non-blocking).
    #[inline]
    pub fn pop(&self) -> Option<T> {
        self.inner.pop()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

// ── Constructor ───────────────────────────────────────────────────────────────

/// Create a new SPSC ring buffer with the given capacity.
///
/// Returns a `(SpscProducer, SpscConsumer)` pair.  The producer must be used
/// from exactly one thread and the consumer from exactly one (different) thread.
pub fn spsc_channel<T: Send>(capacity: usize) -> (SpscProducer<T>, SpscConsumer<T>) {
    let inner = Arc::new(Inner::new(capacity));
    (
        SpscProducer { inner: Arc::clone(&inner) },
        SpscConsumer { inner },
    )
}
