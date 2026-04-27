/// TCP receiver thread.
///
/// Mirrors `TCPReceiverThread.h` / `TCPReceiverThread.cpp`.
///
/// Connects to the sender's TCP endpoint, receives `Packet`s, and pushes them
/// into the `SpscProducer` end of the processor's ring buffer.
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::codec;
use crate::messages::Packet;
use crate::ring_buffer::SpscProducer;

// ── Spawn ─────────────────────────────────────────────────────────────────────

/// Spawn the TCP receiver thread.
///
/// `addr`          — IP address of the sender (e.g. `"127.0.0.1"`).
/// `port`          — TCP port the sender is listening on.
/// `producer`      — write end of the processor's SPSC ring buffer.
/// `retry_count`   — how many connection attempts; `-1` = retry forever.
/// `retry_interval`— seconds between retries.
/// `running`       — shared stop flag.
pub fn spawn(
    addr:           String,
    port:           u16,
    producer:       SpscProducer<Packet>,
    retry_count:    i32,
    retry_interval: u64,
    running:        Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        receiver_run(addr, port, producer, retry_count, retry_interval, running);
    })
}

// ── Thread body ───────────────────────────────────────────────────────────────

fn receiver_run(
    addr:           String,
    port:           u16,
    producer:       SpscProducer<Packet>,
    retry_count:    i32,
    retry_interval: u64,
    running:        Arc<AtomicBool>,
) {
    let endpoint = format!("{}:{}", addr, port);

    // Connect (with optional retry).
    let stream = match connect(&endpoint, retry_count, retry_interval, &running) {
        Some(s) => s,
        None    => { eprintln!("[TcpReceiver] could not connect to {}", endpoint); return; }
    };
    println!("[TcpReceiver] connected to {}", endpoint);

    // Set a 1-second read timeout so we check `running` periodically.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));

    let mut reader   = std::io::BufReader::new(stream);
    let mut pkt_cnt  = 0usize;
    let mut bad_cnt  = 0usize;

    while running.load(Ordering::Acquire) {
        match codec::read_packet(&mut reader) {
            Ok(Some(pkt)) => {
                // Spin-push: if ring buffer is full, spin until space appears.
                let mut pushed = false;
                while !pushed {
                    pushed = producer.push(pkt.clone());
                    if !pushed {
                        thread::yield_now();
                    }
                }
                println!("[TcpReceiver] pushed a packet to ring buffer");
                pkt_cnt += 1;
            }
            Ok(None) => {
                // Sender closed the connection — all data delivered.
                println!("[TcpReceiver] connection closed by sender");
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut
                   || e.kind() == std::io::ErrorKind::WouldBlock => {
                // Timeout — loop and check `running`.
            }
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                println!("[TcpReceiver] connection closed (EOF)");
                break;
            }
            Err(e) => {
                eprintln!("[TcpReceiver] recv error: {}", e);
                bad_cnt += 1;
                if bad_cnt > 100 {
                    break;
                }
            }
        }
    }
    println!("[TcpReceiver] received {} packets, {} errors", pkt_cnt, bad_cnt);
}

fn connect(
    endpoint:       &str,
    retry_count:    i32,
    retry_interval: u64,
    running:        &AtomicBool,
) -> Option<TcpStream> {
    let interval = Duration::from_secs(retry_interval.max(1));

    if retry_count < 0 {
        // Retry forever while running.
        while running.load(Ordering::Acquire) {
            match TcpStream::connect(endpoint) {
                Ok(s)  => return Some(s),
                Err(_) => thread::sleep(interval),
            }
        }
        None
    } else {
        for _ in 0..retry_count {
            match TcpStream::connect(endpoint) {
                Ok(s)  => return Some(s),
                Err(_) => thread::sleep(interval),
            }
        }
        None
    }
}
