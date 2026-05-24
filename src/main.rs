// These allow attrs silence dead_code / unused warnings for fields and methods
// that mirror the C++ API contract and will be used as the port grows.
#![allow(dead_code)]

mod codec;
mod config;
mod precision_codec;
mod exchange_order_book;
mod messages;
mod order;
mod order_book;
mod order_queue;
mod parser;
mod perf_counter;
mod price;
mod price_trait;
mod processor;
mod ring_buffer;
mod tcp_receiver;
mod tcp_sender;
mod trade;
mod types;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use config::Config;
use exchange_order_book::ExchangeOrderBook;
use messages::{CoreMessage, Packet, Price};
use perf_counter::PerfMeta;
use ring_buffer::spsc_channel;

/// Ring-buffer capacity — mirrors C++ `RING_BUFFER_SIZE` (2^20).
const RING_BUFFER_SIZE: usize = 1 << 20;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1].ends_with(".ini") {
        run_network_mode(&args[1])?;
    } else {
        run_file_mode(&args)?;
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// File-only mode (no networking)
// ─────────────────────────────────────────────────────────────────────────────

fn run_file_mode(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    type Msg = CoreMessage<Price>;
    let default_symbols = "../../C++/order-book/test/Symbols.txt";
    let default_data    = vec!["../../C++/order-book/test/AAPLOrders.txt".to_string()];

    let (symbols_file, data_files): (String, Vec<String>) = if args.len() > 1 {
        (args[1].clone(), args[2..].to_vec())
    } else {
        (default_symbols.to_string(), default_data)
    };

    let mut exchange     = ExchangeOrderBook::new("RustExchange");
    let mut add_count    = 0usize;
    let mut modify_count = 0usize;
    let mut cancel_count = 0usize;

    // Load symbols.
    let sym_msgs = parser::load_messages::<Price>(Path::new(&symbols_file))?;
    if sym_msgs.is_empty() {
        eprintln!("No symbol messages loaded from {}", symbols_file);
        std::process::exit(1);
    }
    let mut sym_count = 0usize;
    for msg in &sym_msgs {
        if let CoreMessage::Symbol(s) = msg {
            exchange.add_update_symbol(s);
            sym_count += 1;
        }
    }
    println!("Registered {} symbol(s).", sym_count);

    // Load order data files.
    for data_file in &data_files {
        let msgs = parser::load_messages::<Price>(Path::new(data_file))?;
        println!("Loaded {} message(s) from {}.", msgs.len(), data_file);
        for msg in msgs {
            match msg {
                CoreMessage::Symbol(s)      => { exchange.add_update_symbol(&s); }
                CoreMessage::AddOrder(a)    => { exchange.add_new_order(&a);     add_count    += 1; }
                CoreMessage::ModifyOrder(m) => { exchange.update_order(&m);      modify_count += 1; }
                CoreMessage::CancelOrder(c) => { exchange.cancel_order(&c);      cancel_count += 1; }
            }
        }
    }
    println!(
        "Processed: {} add, {} modify, {} cancel.",
        add_count, modify_count, cancel_count
    );

    println!("\n══ Final Order Books ══\n");
    exchange.print_all_books();
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Network mode (TCP sender/receiver/processor pipeline)
// ─────────────────────────────────────────────────────────────────────────────

fn run_network_mode(config_path: &str) -> Result<(), std::io::Error> {
    let cfg = match Config::from_ini(Path::new(config_path)) {
        Ok(c)  => c,
        Err(e) => { eprintln!("Config error: {}", e); std::process::exit(1); }
    };

    println!("Exchange: {}", cfg.exchange_name);
    println!("Mode:     network");
    println!("Producer: 0.0.0.0:{}", cfg.producer_port);
    println!("Consumer: {}:{}", cfg.consumer_ip, cfg.consumer_port);

    // 1. Load and pack all messages into Packets.
    let mut packets: Vec<Packet<Price>> = Vec::new();
    let mut cur_pkt: Packet<Price> = Packet::new();

    let messages = parser::load_messages::<Price>(Path::new(&cfg.symbol_file))?;
    
    for msg in messages{
        if cur_pkt.len() >= codec::MESSAGES_PER_PACKET {
            packets.push(cur_pkt.clone());
            cur_pkt = Packet::new();
        }
        cur_pkt.push(msg);
    }
    for data_file in &cfg.data_files {
        println!("[DEBUG] Attempting to load data file: {}", data_file);
        match parser::load_messages::<Price>(Path::new(data_file)) {
            Ok(file_messages) => {
                println!("[DEBUG] Loaded {} messages from {}", file_messages.len(), data_file);
                if file_messages.is_empty() {
                    println!("[WARNING] No messages found in {}", data_file);
                }
                for msg in file_messages {
                    if cur_pkt.len() >= codec::MESSAGES_PER_PACKET {
                        packets.push(cur_pkt.clone());
                        cur_pkt = Packet::new();
                    }
                    cur_pkt.push(msg);
                }
            }
            Err(e) => {
                println!("[ERROR] Failed to load {}: {}", data_file, e);
            }
        }
    }
    if !cur_pkt.is_empty() {
        packets.push(cur_pkt);
    }
    println!("Prepared {} packets for transmission.", packets.len());

    // 2. Shared stop flags.
    let running = Arc::new(AtomicBool::new(true)); // For sender/receiver
    let processor_running = Arc::new(AtomicBool::new(true)); // For processor thread

    // 3. SPSC ring buffer (receiver → processor).
    let (ring_producer, ring_consumer) = spsc_channel::<Packet<Price>>(RING_BUFFER_SIZE);
    // Clone the consumer so main can check is_empty()
    let ring_consumer_for_proc = ring_consumer.clone();

    // 4. Processor thread.
    let exchange: ExchangeOrderBook<Price> = ExchangeOrderBook::new(cfg.exchange_name.clone());
    let perf_meta = PerfMeta {
        enabled:         cfg.perf_enabled,
        output_file:     cfg.perf_output_file.clone(),
        processor_speed: cfg.perf_proc_speed,
    };
    let proc_handle  = processor::spawn(exchange, ring_consumer_for_proc, perf_meta, Arc::clone(&processor_running));

    // 5. TCP sender thread.
    let (sender, sender_handle) = tcp_sender::spawn(cfg.producer_port, cfg.producer_rate);

    // Pre-load packets into the sender queue.
    let total = packets.len();
    for pkt in packets {
        while !sender.enqueue(pkt.clone()) {
            thread::yield_now();
        }
    }
    println!("Enqueued {} packets into sender queue.", total);

    // 6. Brief pause, then start receiver.
    thread::sleep(Duration::from_millis(50));
    let recv_running = Arc::clone(&running);
    let recv_handle  = tcp_receiver::spawn(
        cfg.consumer_ip.clone(),
        cfg.consumer_port,
        ring_producer,
        -1, // retry forever
        1,
        recv_running,
    );

    // 7. Wait for threads to complete.
    println!("Waiting for sender and receiver to complete...");
    let _ = sender_handle.join();
    println!("Sender thread has completed.");
    running.store(false, Ordering::Release);
    let _ = recv_handle.join();
    println!("Receiver thread has completed.");

    // Wait until the ring buffer is empty before shutting down the processor
    let mut waited_ms = 0;
    let max_wait_ms = 2000; // 2 seconds max
    while !ring_consumer.is_empty() && waited_ms < max_wait_ms {
        std::thread::sleep(std::time::Duration::from_millis(10));
        waited_ms += 10;
    }
    if !ring_consumer.is_empty() {
        println!("[WARN] Ring buffer not empty after waiting {} ms!", max_wait_ms);
    } else {
        println!("Ring buffer is empty, safe to shut down processor.");
    }
    processor_running.store(false, Ordering::Release);
    let exchange = proc_handle.join().expect("processor thread panicked");

    // 8. Print final state.
    println!("\n══ Final Order Books ══\n");
    exchange.print_all_books();

    Ok(())
}
