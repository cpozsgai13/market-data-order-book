

impl From<Price4> for Price6 {
    fn from(p: Price4) -> Price6 {
        Price6::from_raw(p.raw_value() * 10u64.pow(6 - 4))
    }
}
impl From<Price6> for Price4 {
    fn from(p: Price6) -> Price4 {
        Price4::from_raw(p.raw_value() / 10u64.pow(6 - 4))
    }
}
impl From<Price4> for Price9 {
    fn from(p: Price4) -> Price9 {
        Price9::from_raw(p.raw_value() * 10u64.pow(9 - 4))
    }
}
impl From<Price9> for Price4 {
    fn from(p: Price9) -> Price4 {
        Price4::from_raw(p.raw_value() / 10u64.pow(9 - 4))
    }
}
impl From<Price6> for Price9 {
    fn from(p: Price6) -> Price9 {
        Price9::from_raw(p.raw_value() * 10u64.pow(9 - 6))
    }
}
impl From<Price9> for Price6 {
    fn from(p: Price9) -> Price6 {
        Price6::from_raw(p.raw_value() / 10u64.pow(9 - 6))
    }
}
/// Generic fixed-precision price type.
///
/// Mirrors the C++ template `FixedPrecisionPrice<T, Places>` using Rust
/// const generics.  The two type parameters are:
///
/// * `T`      — the underlying unsigned integer storage type (e.g. `u64`).
/// * `PLACES` — the number of decimal places encoded as a `u32` const.
///
/// The compile-time divisor is computed via a `const fn` helper so there is
/// zero runtime overhead — identical to `Power<T, Exp>::value` in C++.
///
/// ```
/// use price::FixedPrecisionPrice;
///
/// // Price with 4 decimal places stored as u64
/// // 1000243 represents 100.0243
/// let p = FixedPrecisionPrice::<u64, 4>::from_raw(1_000_243_u64);
/// assert_eq!(p.to_f64(), 100.0243_f64);
/// ```
///
/// The project-wide `Price` alias is `FixedPrecisionPrice<u64, 6>`, matching
/// the existing wire-format and parser expectations.
use std::fmt;
use std::hash::{Hash, Hasher};

// ── Compile-time power helper ─────────────────────────────────────────────────

/// Compute `10^PLACES` at compile time for any unsigned primitive.
///
/// Mirrors `Power<T, Exp>::value` from `FixedPrecisionPrice.h`.
const fn pow10(places: u32) -> u64 {
    let mut result = 1u64;
    let mut i = 0u32;
    while i < places {
        result *= 10;
        i += 1;
    }
    result
}

// ── FixedPrecisionPrice ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default)]
pub struct FixedPrecisionPrice<T, const PLACES: u32> {
    raw: T,
}

impl<const PLACES: u32> FixedPrecisionPrice<u64, PLACES> {
    /// Divisor = 10^PLACES, computed at compile time.
    pub const DIVISOR: u64 = pow10(PLACES);

    /// Number of decimal places.
    pub const NUM_PLACES: u32 = PLACES;

    /// Construct from a raw (already-scaled) integer — mirrors `FixedPrecisionPrice(T t)`.
    #[inline]
    pub fn from_raw(raw: u64) -> Self {
        Self { raw }
    }

    /// Return the raw (scaled) integer — mirrors `rawValue()`.
    #[inline]
    pub fn raw_value(self) -> u64 {
        self.raw
    }

    /// Convert to f64 — mirrors `operator double()`.
    #[inline]
    pub fn to_f64(self) -> f64 {
        self.raw as f64 / Self::DIVISOR as f64
    }

    /// Number of decimal places — mirrors `numPlaces()`.
    #[inline]
    pub fn num_places(self) -> u32 {
        PLACES
    }

    /// Number of integer digits — mirrors `numDigits()`.
    pub fn num_digits(self) -> u32 {
        let int_part = self.raw / Self::DIVISOR;
        if int_part > 0 {
            int_part.ilog10() + 1
        } else {
            0
        }
    }
}

// ── Conversions ───────────────────────────────────────────────────────────────

impl<const PLACES: u32> From<f64> for FixedPrecisionPrice<u64, PLACES> {
    /// Mirrors `FixedPrecisionPrice(double d)`.
    fn from(d: f64) -> Self {
        Self {
            raw: (d * Self::DIVISOR as f64).round() as u64,
        }
    }
}

impl<const PLACES: u32> From<u64> for FixedPrecisionPrice<u64, PLACES> {
    fn from(raw: u64) -> Self {
        Self { raw }
    }
}

// ── Comparison / ordering ─────────────────────────────────────────────────────

impl<const PLACES: u32> PartialEq for FixedPrecisionPrice<u64, PLACES> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<const PLACES: u32> Eq for FixedPrecisionPrice<u64, PLACES> {}

impl<const PLACES: u32> PartialOrd for FixedPrecisionPrice<u64, PLACES> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<const PLACES: u32> Ord for FixedPrecisionPrice<u64, PLACES> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.raw.cmp(&other.raw)
    }
}

// ── Hashing ───────────────────────────────────────────────────────────────────

impl<const PLACES: u32> Hash for FixedPrecisionPrice<u64, PLACES> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

// ── Display ───────────────────────────────────────────────────────────────────

impl<const PLACES: u32> fmt::Display for FixedPrecisionPrice<u64, PLACES> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.prec$}", self.to_f64(), prec = PLACES as usize)
    }
}

// ── Named precision aliases ───────────────────────────────────────────────────
//
// These mirror the pattern of having named typedefs for common precisions.
// Each implementation can import the alias it needs, or define its own:
//
//   pub type MyPrice = FixedPrecisionPrice<u64, N>;

/// 4 decimal places — e.g. FX pip-level pricing (1.2345).
pub type Price4 = FixedPrecisionPrice<u64, 4>;

/// 6 decimal places — equity / crypto standard (100.000001).
pub type Price6 = FixedPrecisionPrice<u64, 6>;

/// 9 decimal places — high-precision crypto / fixed-income (1.000000001).
pub type Price9 = FixedPrecisionPrice<u64, 9>;

// No default `Price` alias — each module must explicitly choose its precision:
//
//   use crate::price::Price6 as Price;   // equities / crypto (default choice)
//   use crate::price::Price4 as Price;   // FX pip-level
//   use crate::price::Price9 as Price;   // high-precision crypto / fixed-income
//   pub type MyPrice = FixedPrecisionPrice<u64, N>;  // custom
