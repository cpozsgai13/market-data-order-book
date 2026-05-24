/// TCP sender thread.
///
/// Mirrors `TCPSenderThread.h` / `TCPSenderThread.cpp`.
///
/// Lifecycle:
///   1. Caller pre-loads packets via `enqueue()`.
///   2. `spawn()` binds a TCP listen socket, then starts a background thread
///      that accepts **one** client connection and sends all queued packets.
///   3. When the internal queue is drained the thread closes the socket and exits.
use std::io::BufWriter;
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::codec;
use crate::messages::{Packet, Price};

/// Default internal queue depth.  Mirrors `RING_BUFFER_SIZE` (2^20).
const QUEUE_DEPTH: usize = 1 << 20;

/// Nanoseconds per second.
const NANOS_PER_SEC: u64 = 1_000_000_000;

// ── TcpSender ─────────────────────────────────────────────────────────────────

pub struct TcpSender {
    tx:      SyncSender<Packet<Price>>,
    running: Arc<AtomicBool>,
}

impl TcpSender {
    /// Push a packet into the sender's internal queue.
    /// Returns `false` if the queue is full.
    pub fn enqueue(&self, packet: Packet<Price>) -> bool {
        self.tx.try_send(packet).is_ok()
    }

    /// Signal the sender thread to stop.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
    }
}

/// Spawn the TCP sender thread.
///
/// Returns a `TcpSender` handle for enqueue / stop and a `JoinHandle`.
///
/// `port`        — TCP port to listen on.
/// `rate_pps`    — send rate in packets/second; ≤ 0 means unlimited.
pub fn spawn(
    port:     u16,
    rate_pps: i32,
) -> (TcpSender, thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::sync_channel::<Packet<Price>>(QUEUE_DEPTH);
    let running  = Arc::new(AtomicBool::new(true));
    let running2 = Arc::clone(&running);

    let handle = thread::spawn(move || {
        sender_run(rx, port, rate_pps, running2);
    });

    (TcpSender { tx, running }, handle)
}

// ── Thread body ───────────────────────────────────────────────────────────────

fn sender_run(
    rx:       Receiver<Packet<Price>>,
    port:     u16,
    rate_pps: i32,
    running:  Arc<AtomicBool>,
) {
    // Bind the listener.
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", port)) {
        Ok(l)  => l,
        Err(e) => { eprintln!("[TcpSender] bind failed on port {}: {}", port, e); return; }
    };
    println!("[TcpSender] listening on port {}", port);

    // Accept exactly one connection.
    let stream = match listener.accept() {
        Ok((s, addr)) => { println!("[TcpSender] client connected from {}", addr); s }
        Err(e) => { eprintln!("[TcpSender] accept error: {}", e); return; }
    };

    let delay = if rate_pps > 0 {
        Some(Duration::from_nanos(NANOS_PER_SEC / rate_pps as u64))
    } else {
        None
    };

    let start_time  = std::time::Instant::now();
    let mut count   = 0usize;
    let bad       = 0usize;
    let mut writer  = BufWriter::new(stream);

    while running.load(Ordering::Acquire) {
        match rx.try_recv() {
            Ok(packet) => {
                if !packet.is_empty() {
                    if let Some(d) = delay {
                        thread::sleep(d);
                    }
                    if let Err(e) = codec::write_packet(&mut writer, &packet) {
                        eprintln!("[TcpSender] send error: {}", e);
                        break;
                    }
                    count += 1;
                    println!("[TcpSender] sent a packet");
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                // Queue drained — all pre-loaded packets sent.
                let elapsed = start_time.elapsed();
                println!(
                    "[TcpSender] queue empty after {:.2} ms, sent {} packets, {} errors",
                    elapsed.as_secs_f64() * 1000.0,
                    count,
                    bad
                );
                // Drop writer to close socket and signal receiver.
                break;
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
        }
    }
    // Drop writer → flushes BufWriter → closes TCP socket.
    println!("[TcpSender] connection closed (sender thread exiting)");
    // Do not set running here. Main controls shutdown.
}
