/// Simple INI-file config parser.
///
/// Mirrors the `boost::property_tree::ini_parser` usage in `main.cpp`.
///
/// Supported syntax:
/// ```ini
/// key=value          ; global (no section)
/// [section]
/// key=value          ; section-scoped key
/// # comment
/// ; comment
/// ```
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

// ── Raw INI map ───────────────────────────────────────────────────────────────

/// Flat key→value map.  Section-scoped keys are stored as `"section.key"`.
pub type IniMap = HashMap<String, String>;

/// Parse an INI file into a flat `IniMap`.
pub fn parse_ini(path: &Path) -> Result<IniMap, String> {
    let file = File::open(path)
        .map_err(|e| format!("Cannot open config file {:?}: {}", path, e))?;

    let mut map     = IniMap::new();
    let mut section = String::new();

    for (lineno, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|e| format!("line {}: {}", lineno + 1, e))?;
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_lowercase();
            continue;
        }

        if let Some(eq) = line.find('=') {
            let key   = line[..eq].trim().to_lowercase();
            let value = line[eq + 1..].trim().to_string();
            let full_key = if section.is_empty() {
                key
            } else {
                format!("{}.{}", section, key)
            };
            // Later duplicates win (same as boost property_tree behaviour for
            // the same-section repeated `[data]` block in config.ini).
            map.insert(full_key, value);
        }
    }
    Ok(map)
}

// ── Typed helpers ─────────────────────────────────────────────────────────────

pub trait IniGet {
    fn get_str(&self, key: &str) -> Option<&str>;
    fn get_or(&self, key: &str, default: &str) -> String;
    fn get_bool(&self, key: &str, default: bool) -> bool;
    fn get_u16(&self, key: &str, default: u16) -> u16;
    fn get_i32(&self, key: &str, default: i32) -> i32;
    fn get_u32(&self, key: &str, default: u32) -> u32;
    fn get_f64(&self, key: &str, default: f64) -> f64;
}

impl IniGet for IniMap {
    fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).map(|s| s.as_str())
    }

    fn get_or(&self, key: &str, default: &str) -> String {
        self.get(key).cloned().unwrap_or_else(|| default.to_string())
    }

    fn get_bool(&self, key: &str, default: bool) -> bool {
        self.get(key).map_or(default, |v| {
            matches!(v.to_lowercase().as_str(), "true" | "1" | "yes")
        })
    }

    fn get_u16(&self, key: &str, default: u16) -> u16 {
        self.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
    }

    fn get_i32(&self, key: &str, default: i32) -> i32 {
        self.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
    }

    fn get_u32(&self, key: &str, default: u32) -> u32 {
        self.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
    }

    fn get_f64(&self, key: &str, default: f64) -> f64 {
        self.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
    }
}

// ── Config struct ─────────────────────────────────────────────────────────────

/// Strongly-typed configuration — mirrors every field read in `main.cpp`.
#[derive(Debug, Clone)]
pub struct Config {
    pub exchange_name:      String,

    // [data]
    pub data_type:          String,   // "file" | "random"
    pub symbol_file:        String,
    pub data_files:         Vec<String>,
    pub num_random_orders:  u32,

    // [consumer]
    pub consumer_ip:        String,
    pub consumer_port:      u16,
    pub consumer_core:      i32,

    // [producer]
    pub producer_port:      u16,
    pub producer_core:      i32,
    pub producer_rate:      i32,      // packets/sec; -1 = unlimited

    // [perf]
    pub perf_enabled:       bool,
    pub perf_output_file:   String,
    pub perf_proc_speed:    f64,      // GHz
}

impl Config {
    pub fn from_ini(path: &Path) -> Result<Self, String> {
        let m = parse_ini(path)?;

        let data_type = m.get_or("data.type", "file");

        // market_data_files is a comma-separated list.
        let data_files: Vec<String> = m
            .get_or("data.market_data_files", "")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        Ok(Config {
            exchange_name:     m.get_or("exchange_name", "Exchange"),

            data_type,
            symbol_file:       m.get_or("data.symbol_file", "Symbols.txt"),
            data_files,
            num_random_orders: m.get_u32("data.n", 1000),

            consumer_ip:   m.get_or("consumer.ip", "127.0.0.1"),
            consumer_port: m.get_u16("consumer.port", 1234),
            consumer_core: m.get_i32("consumer.core", -1),

            producer_port: m.get_u16("producer.port", 1234),
            producer_core: m.get_i32("producer.core", -1),
            producer_rate: m.get_i32("producer.rate", -1),

            perf_enabled:     m.get_bool("perf.enabled",    false),
            perf_output_file: m.get_or("perf.outfile",      "perf.csv"),
            perf_proc_speed:  m.get_f64("perf.procspeed",   0.0),
        })
    }
}
