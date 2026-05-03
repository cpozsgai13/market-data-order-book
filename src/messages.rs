/// Message types that flow through the system.
///
/// Thin re-export of `precision_codec::price6` (6 decimal places).
/// For a different precision import directly from:
///   `crate::precision_codec::price4`  — FX pip-level
///   `crate::precision_codec::price9`  — high-precision crypto
pub use crate::precision_codec::price6::{
    Price,
    AddOrderMsg,
    CancelOrderMsg,
    CoreMessage,
    ModifyOrderMsg,
    SymbolMsg,
    Packet,
};
