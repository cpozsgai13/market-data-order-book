/// Latency counter for add / update / cancel operations.
///
/// Mirrors `PerformanceCounter.h` / `PerformanceCounter.cpp`.
///
/// Timing uses `std::time::Instant` (nanosecond resolution) instead of the
/// C++ RDTSC-based counter.
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

// ── PerfCounter ───────────────────────────────────────────────────────────────

pub struct PerfCounter {
    add_stats:    Vec<u64>,
    update_stats: Vec<u64>,
    cancel_stats: Vec<u64>,
    output_file:  String,
}

impl PerfCounter {
    pub fn new(output_file: impl Into<String>) -> Self {
        PerfCounter {
            add_stats:    Vec::new(),
            update_stats: Vec::new(),
            cancel_stats: Vec::new(),
            output_file:  output_file.into(),
        }
    }

    // ── Timed call wrappers ───────────────────────────────────────────────────

    /// Time a closure and record the duration in the add-stat bucket.
    pub fn time_add<F: FnOnce() -> R, R>(&mut self, f: F) -> R {
        let (r, ns) = timed(f);
        self.add_stats.push(ns);
        r
    }

    /// Time a closure and record the duration in the update-stat bucket.
    pub fn time_update<F: FnOnce() -> R, R>(&mut self, f: F) -> R {
        let (r, ns) = timed(f);
        self.update_stats.push(ns);
        r
    }

    /// Time a closure and record the duration in the cancel-stat bucket.
    pub fn time_cancel<F: FnOnce() -> R, R>(&mut self, f: F) -> R {
        let (r, ns) = timed(f);
        self.cancel_stats.push(ns);
        r
    }

    // ── Reporting ─────────────────────────────────────────────────────────────

    pub fn print_stats(&self) {
        let sep = "-".repeat(50);
        print_bucket("ADD",    &self.add_stats);
        println!("{sep}");
        print_bucket("UPDATE", &self.update_stats);
        println!("{sep}");
        print_bucket("CANCEL", &self.cancel_stats);
        println!("{sep}");
    }

    pub fn write_to_file(&self) {
        if self.output_file.is_empty() {
            return;
        }
        let file = match File::create(&self.output_file) {
            Ok(f)  => f,
            Err(e) => { eprintln!("PerfCounter: cannot open {}: {}", self.output_file, e); return; }
        };
        let mut w = BufWriter::new(file);
        let _ = writeln!(w, "EventType,I,Nanoseconds");
        for (i, &ns) in self.add_stats.iter().enumerate() {
            let _ = writeln!(w, "0,{},{}", i + 1, ns);
        }
        for (i, &ns) in self.update_stats.iter().enumerate() {
            let _ = writeln!(w, "1,{},{}", i + 1, ns);
        }
        for (i, &ns) in self.cancel_stats.iter().enumerate() {
            let _ = writeln!(w, "2,{},{}", i + 1, ns);
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Call `f` and return `(result, elapsed_nanoseconds)`.
#[inline]
fn timed<F: FnOnce() -> R, R>(f: F) -> (R, u64) {
    let start = Instant::now();
    let r = f();
    let ns = start.elapsed().as_nanos() as u64;
    (r, ns)
}

/// Compute (min, max, mean, median) from a slice.
fn compute_stats(data: &[u64]) -> (u64, u64, u64, u64) {
    if data.is_empty() {
        return (0, 0, 0, 0);
    }
    let min  = *data.iter().min().unwrap();
    let max  = *data.iter().max().unwrap();
    let mean = data.iter().sum::<u64>() / data.len() as u64;
    let mut sorted = data.to_vec();
    sorted.sort_unstable();
    let n   = sorted.len();
    let med = if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2
    };
    (min, max, mean, med)
}

fn print_bucket(label: &str, data: &[u64]) {
    println!("{} STATS:", label);
    println!("\tCOUNT: {}", data.len());
    let (min, max, mean, med) = compute_stats(data);
    println!("\tMIN:  {} ns", min);
    println!("\tMAX:  {} ns", max);
    println!("\tMEAN: {} ns", mean);
    println!("\tMDN:  {} ns", med);
}

// ── PerformanceMeta (config carrier) ─────────────────────────────────────────

/// Mirrors the C++ `PerformanceMeta` struct.
#[derive(Debug, Default, Clone)]
pub struct PerfMeta {
    pub enabled:         bool,
    pub output_file:     String,
    pub processor_speed: f64, // GHz (informational; timing now uses Instant)
}
