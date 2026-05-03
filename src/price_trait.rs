/// Trait for types that behave like FixedPrecisionPrice.
pub trait FixedPrecisionPriceLike: Copy + Ord + std::fmt::Display + std::hash::Hash {
    fn to_f64(self) -> f64;
}

// Blanket impl for all FixedPrecisionPrice<u64, N>
impl<const N: u32> FixedPrecisionPriceLike for crate::price::FixedPrecisionPrice<u64, N> {
    fn to_f64(self) -> f64 {
        self.to_f64()
    }
}