/// Wire-format codec — thin re-export of `precision_codec::price6`.
///
/// For a different precision use the submodule directly:
///   `crate::precision_codec::price4::read_packet`
///   `crate::precision_codec::price9::write_packet`
#[allow(unused_imports)]
pub use crate::precision_codec::price6::{
    decode, encode, read_exact_bytes, read_packet, write_packet,
    CORE_MSG_SIZE, HEADER_SIZE, MESSAGES_PER_PACKET, PAYLOAD_SIZE,
};
