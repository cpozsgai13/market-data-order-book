/// Exchange data processor thread.
///
/// Mirrors `ExchangeDataProcessorThread.h` / `ExchangeDataProcessorThread.cpp`.
///
/// Drains the SPSC ring buffer, routes each `CoreMessage` to the
/// `ExchangeOrderBook`, and optionally records per-operation latency via
/// `PerfCounter`.  The processor thread **owns** the `ExchangeOrderBook` and
/// returns it when the thread exits so the caller can print the final state.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use crate::exchange_order_book::ExchangeOrderBook;
use crate::messages::{CoreMessage, Packet};
use crate::perf_counter::{PerfCounter, PerfMeta};
use crate::price_trait::FixedPrecisionPriceLike;
use crate::ring_buffer::SpscConsumer;

// ── Spawn ─────────────────────────────────────────────────────────────────────

/// Spawn the processor thread.
///
/// Returns a `JoinHandle<ExchangeOrderBook<P>>` — call `.join().unwrap()` after
/// stopping all threads to retrieve the final order-book state.
pub fn spawn<P>(
    exchange: ExchangeOrderBook<P>,
    consumer: SpscConsumer<Packet<P>>,
    perf_meta: PerfMeta,
    running:  Arc<AtomicBool>,
) -> thread::JoinHandle<ExchangeOrderBook<P>>
where
    P: FixedPrecisionPriceLike + Send + 'static,
{
    thread::spawn(move || {
        processor_run(exchange, consumer, perf_meta, running)
    })
}

// ── Thread body ───────────────────────────────────────────────────────────────

fn processor_run<P>(
    mut exchange: ExchangeOrderBook<P>,
    consumer:     SpscConsumer<Packet<P>>,
    perf_meta:    PerfMeta,
    running:      Arc<AtomicBool>,
) -> ExchangeOrderBook<P>
where
    P: FixedPrecisionPriceLike + Send + 'static,
{
    let mut perf = if perf_meta.enabled {
        Some(PerfCounter::new(perf_meta.output_file.clone()))
    } else {
        None
    };

    let mut loop_count = 0u64;
    while running.load(Ordering::Acquire) {
        loop_count += 1;
        if let Some(packet) = consumer.pop() {
            println!("[Processor] popped a packet from ring buffer");
            for (i, msg) in packet.messages.iter().enumerate() {
                match msg {
                    CoreMessage::Symbol(s) => println!("[Processor] msg {}: Symbol {} (id={})", i, s.symbol, s.instrument_id),
                    CoreMessage::AddOrder(a) => println!("[Processor] msg {}: AddOrder instrument_id={}", i, a.instrument_id),
                    CoreMessage::ModifyOrder(m) => println!("[Processor] msg {}: ModifyOrder instrument_id={}", i, m.instrument_id),
                    CoreMessage::CancelOrder(c) => println!("[Processor] msg {}: CancelOrder instrument_id={}", i, c.instrument_id),
                }
            }
            process_packet(packet, &mut exchange, &mut perf);
        }
        if loop_count % 1_000_000 == 0 {
            println!("[Processor] still running, loop_count={}", loop_count);
        }
    }
    println!("[Processor] running flag is now false, draining remaining packets...");
    while let Some(packet) = consumer.pop() {
        println!("[Processor] draining packet after shutdown");
        for (i, msg) in packet.messages.iter().enumerate() {
            match msg {
                CoreMessage::Symbol(s) => println!("[Processor] msg {}: Symbol {} (id={})", i, s.symbol, s.instrument_id),
                CoreMessage::AddOrder(a) => println!("[Processor] msg {}: AddOrder instrument_id={}", i, a.instrument_id),
                CoreMessage::ModifyOrder(m) => println!("[Processor] msg {}: ModifyOrder instrument_id={}", i, m.instrument_id),
                CoreMessage::CancelOrder(c) => println!("[Processor] msg {}: CancelOrder instrument_id={}", i, c.instrument_id),
            }
        }
        process_packet(packet, &mut exchange, &mut perf);
    }
    println!("[Processor] exiting thread, returning ExchangeOrderBook");

    if let Some(ref p) = perf {
        p.print_stats();
        p.write_to_file();
    }

    exchange
}

fn process_packet<P>(
    packet:   Packet<P>,
    exchange: &mut ExchangeOrderBook<P>,
    perf:     &mut Option<PerfCounter>,
)
where
    P: FixedPrecisionPriceLike,
{
    for msg in packet.messages {
        process_message(msg, exchange, perf);
    }
}

fn process_message<P>(
    msg:      CoreMessage<P>,
    exchange: &mut ExchangeOrderBook<P>,
    perf:     &mut Option<PerfCounter>,
)
where
    P: FixedPrecisionPriceLike,
{
    match msg {
        CoreMessage::Symbol(s) => {
            println!("[Processor] processing Symbol message: {} (id={})", s.symbol, s.instrument_id);
            exchange.add_update_symbol(&s);
        }
        CoreMessage::AddOrder(a) => {
            println!("[Processor] processing AddOrder message: instrument_id={}", a.instrument_id);
            if let Some(p) = perf {
                p.time_add(|| exchange.add_new_order(&a));
            } else {
                exchange.add_new_order(&a);
            }
        }
        CoreMessage::ModifyOrder(m) => {
            println!("[Processor] processing ModifyOrder message: instrument_id={}", m.instrument_id);
            if let Some(p) = perf {
                p.time_update(|| exchange.update_order(&m));
            } else {
                exchange.update_order(&m);
            }
        }
        CoreMessage::CancelOrder(c) => {
            println!("[Processor] processing CancelOrder message: instrument_id={}", c.instrument_id);
            if let Some(p) = perf {
                p.time_cancel(|| exchange.cancel_order(&c));
            } else {
                exchange.cancel_order(&c);
            }
        }
    }
}
