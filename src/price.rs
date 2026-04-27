/// Fixed-precision price with 6 decimal places, stored as a raw u64.
///
/// Mirrors `FixedPrecisionPrice<uint64_t, 6>` from the C++ code-base.
/// A value of `1_000_000` represents the price `1.000000`.
use std::fmt;
use std::hash::{Hash, Hasher};

/// 10^6 — the scaling factor used throughout.
pub const PRICE_DIVISOR: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, Default)]
pub struct Price {
    raw: u64,
}

impl Price {
    /// Construct from a raw (already-scaled) integer.
    #[inline]
    pub fn from_raw(raw: u64) -> Self {
        Price { raw }
    }

    /// Return the raw (scaled) integer value.
    #[inline]
    pub fn raw_value(self) -> u64 {
        self.raw
    }

    /// Convert to f64 for display purposes.
    #[inline]
    pub fn to_f64(self) -> f64 {
        self.raw as f64 / PRICE_DIVISOR as f64
    }
}

// ── Conversions ──────────────────────────────────────────────────────────────

impl From<f64> for Price {
    fn from(d: f64) -> Self {
        Price {
            raw: (d * PRICE_DIVISOR as f64).round() as u64,
        }
    }
}

impl From<u64> for Price {
    fn from(raw: u64) -> Self {
        Price { raw }
    }
}

// ── Comparison / ordering ─────────────────────────────────────────────────────

impl PartialEq for Price {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Eq for Price {}

impl PartialOrd for Price {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Price {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.raw.cmp(&other.raw)
    }
}

// ── Hashing ───────────────────────────────────────────────────────────────────

impl Hash for Price {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

// ── Display ───────────────────────────────────────────────────────────────────

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.3}", self.to_f64())
    }
}
