/// Macro that generates a complete, self-contained codec module for a given
/// price precision.
///
/// Each invocation produces a submodule containing:
///   - `Price`           — type alias for `FixedPrecisionPrice<u64, PLACES>`
///   - Message structs   — `SymbolMsg`, `AddOrderMsg`, `ModifyOrderMsg`, `CancelOrderMsg`
///   - `CoreMessage`     — tagged enum over those structs
///   - `Packet`          — ordered collection of `CoreMessage`s
///   - Wire-format codec — `encode`, `decode`, `write_packet`, `read_packet`
///
/// # Usage
/// ```
/// define_precision_codec!(price4, 4);   // FX pip-level
/// define_precision_codec!(price6, 6);   // equities / crypto standard
/// define_precision_codec!(price9, 9);   // high-precision crypto
/// ```
///
/// Callers import the precision they need:
/// ```
/// use crate::precision_codec::price6::{CoreMessage, Packet};
/// use crate::precision_codec::price4::Price as FxPrice;
/// ```
macro_rules! define_precision_codec {
    ($mod_name:ident, $places:literal) => {
        pub mod $mod_name {
            use crate::price::FixedPrecisionPrice;
            use crate::types::{InstrumentId, OrderId, OrderType, Quantity, Side, Timestamp};
            use std::io::{self, Read, Write};

            // ── Price alias ────────────────────────────────────────────────────
            pub type Price = FixedPrecisionPrice<u64, $places>;

            // ── Generic message types ─────────────────────────────────────────
            #[derive(Debug, Clone)]
            pub struct SymbolMsg<P> {
                pub symbol:        String,
                pub instrument_id: InstrumentId,
                pub last_price:    P,
            }

            #[derive(Debug, Clone)]
            pub struct AddOrderMsg<P> {
                pub instrument_id:  InstrumentId,
                pub order_id:       OrderId,
                pub price:          P,
                pub quantity:       Quantity,
                pub side:           Side,
                pub order_type:     OrderType,
                pub update_time_ns: Timestamp,
            }

            #[derive(Debug, Clone)]
            pub struct ModifyOrderMsg<P> {
                pub order_id:       OrderId,
                pub instrument_id:  InstrumentId,
                pub price:          P,
                pub side:           Side,
                pub quantity:       Quantity,
                pub update_time_ns: Timestamp,
            }

            #[derive(Debug, Clone)]
            pub struct CancelOrderMsg {
                pub order_id:      OrderId,
                pub instrument_id: InstrumentId,
            }

            #[derive(Debug, Clone)]
            pub enum CoreMessage<P> {
                Symbol(SymbolMsg<P>),
                AddOrder(AddOrderMsg<P>),
                ModifyOrder(ModifyOrderMsg<P>),
                CancelOrder(CancelOrderMsg),
            }

            // ── Type aliases for this precision ──────────────────────────────
            pub type SymbolMsgAlias = SymbolMsg<Price>;
            pub type AddOrderMsgAlias = AddOrderMsg<Price>;
            pub type ModifyOrderMsgAlias = ModifyOrderMsg<Price>;
            pub type CoreMessageAlias = CoreMessage<Price>;

            // ── Wire-format constants ──────────────────────────────────────────
            pub const HEADER_SIZE: usize        = 4;
            pub const PAYLOAD_SIZE: usize       = 43;
            pub const CORE_MSG_SIZE: usize      = 1 + PAYLOAD_SIZE; // 44 bytes
            pub const MESSAGES_PER_PACKET: usize = (1500 - HEADER_SIZE) / CORE_MSG_SIZE; // 34

            const TAG_SYMBOL: u8 = 0;
            const TAG_ADD:    u8 = 1;
            const TAG_MODIFY: u8 = 2;
            const TAG_CANCEL: u8 = 3;

            /// Ordered collection of `CoreMessage`s transported over TCP.
            #[derive(Debug, Clone, Default)]
            pub struct Packet {
                pub messages: Vec<CoreMessageAlias>,
            }

            impl Packet {
                pub fn new() -> Self { Packet { messages: Vec::new() } }
                pub fn push(&mut self, msg: CoreMessageAlias) { self.messages.push(msg); }
                pub fn is_empty(&self) -> bool { self.messages.is_empty() }
                pub fn len(&self) -> usize { self.messages.len() }
            }

            // ── Encode ────────────────────────────────────────────────────────

            pub fn encode(packet: &Packet) -> Vec<u8> {
                let n          = packet.len();
                let payload_sz = n * CORE_MSG_SIZE;
                let total      = HEADER_SIZE + payload_sz;
                let mut buf    = vec![0u8; total];
                buf[0..2].copy_from_slice(&(n as u16).to_le_bytes());
                buf[2..4].copy_from_slice(&(payload_sz as u16).to_le_bytes());
                for (i, msg) in packet.messages.iter().enumerate() {
                    let off = HEADER_SIZE + i * CORE_MSG_SIZE;
                    encode_message(msg, &mut buf[off..off + CORE_MSG_SIZE]);
                }
                buf
            }

            fn encode_message(msg: &CoreMessageAlias, out: &mut [u8]) {
                debug_assert_eq!(out.len(), CORE_MSG_SIZE);
                match msg {
                    CoreMessage::Symbol(s) => {
                        out[0] = TAG_SYMBOL;
                        let p    = &mut out[1..];
                        let name = s.symbol.as_bytes();
                        let copy = name.len().min(20);
                        p[..copy].copy_from_slice(&name[..copy]);
                        p[20..28].copy_from_slice(&s.instrument_id.to_le_bytes());
                        p[28..36].copy_from_slice(&s.last_price.raw_value().to_le_bytes());
                        // [36..43] zero-padded
                    }
                    CoreMessage::AddOrder(a) => {
                        out[0] = TAG_ADD;
                        let p = &mut out[1..];
                        p[0..8].copy_from_slice(&a.instrument_id.to_le_bytes());
                        p[8..16].copy_from_slice(&a.order_id.to_le_bytes());
                        p[16..24].copy_from_slice(&a.price.raw_value().to_le_bytes());
                        p[24..32].copy_from_slice(&a.quantity.to_le_bytes());
                        p[32] = side_to_u8(a.side);
                        p[33] = order_type_to_u8(a.order_type);
                        p[34..42].copy_from_slice(&a.update_time_ns.to_le_bytes());
                        // [42] padding
                    }
                    CoreMessage::ModifyOrder(m) => {
                        out[0] = TAG_MODIFY;
                        let p = &mut out[1..];
                        p[0..8].copy_from_slice(&m.order_id.to_le_bytes());
                        p[8..16].copy_from_slice(&m.instrument_id.to_le_bytes());
                        p[16..24].copy_from_slice(&m.price.raw_value().to_le_bytes());
                        p[24] = side_to_u8(m.side);
                        p[25..33].copy_from_slice(&m.quantity.to_le_bytes());
                        p[33..41].copy_from_slice(&m.update_time_ns.to_le_bytes());
                        // [41..43] padding
                    }
                    CoreMessage::CancelOrder(c) => {
                        out[0] = TAG_CANCEL;
                        let p = &mut out[1..];
                        p[0..8].copy_from_slice(&c.order_id.to_le_bytes());
                        p[8..16].copy_from_slice(&c.instrument_id.to_le_bytes());
                        // [16..43] padding
                    }
                }
            }

            // ── Decode ────────────────────────────────────────────────────────

            pub fn decode(bytes: &[u8]) -> Option<Packet> {
                if bytes.len() < HEADER_SIZE { return None; }
                let num_messages = u16::from_le_bytes([bytes[0], bytes[1]]) as usize;
                let payload_len  = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
                if num_messages == 0
                    || payload_len != num_messages * CORE_MSG_SIZE
                    || bytes.len() < HEADER_SIZE + payload_len
                {
                    return None;
                }
                let mut packet = Packet::new();
                for i in 0..num_messages {
                    let off = HEADER_SIZE + i * CORE_MSG_SIZE;
                    if let Some(msg) = decode_message(&bytes[off..off + CORE_MSG_SIZE]) {
                        packet.push(msg);
                    }
                }
                Some(packet)
            }

            fn decode_message(data: &[u8]) -> Option<CoreMessageAlias> {
                debug_assert_eq!(data.len(), CORE_MSG_SIZE);
                let tag = data[0];
                let p   = &data[1..];
                match tag {
                    TAG_SYMBOL => {
                        let name    = parse_str(&p[0..20]);
                        let inst_id = u64::from_le_bytes(p[20..28].try_into().ok()?);
                        let raw     = u64::from_le_bytes(p[28..36].try_into().ok()?);
                        Some(CoreMessage::Symbol(SymbolMsg {
                            symbol:        name,
                            instrument_id: inst_id,
                            last_price:    Price::from_raw(raw),
                        }))
                    }
                    TAG_ADD => {
                        let inst_id   = u64::from_le_bytes(p[0..8].try_into().ok()?);
                        let order_id  = u64::from_le_bytes(p[8..16].try_into().ok()?);
                        let price_raw = u64::from_le_bytes(p[16..24].try_into().ok()?);
                        let qty       = u64::from_le_bytes(p[24..32].try_into().ok()?);
                        let side      = u8_to_side(p[32]);
                        let otype     = u8_to_order_type(p[33]);
                        let ts        = u64::from_le_bytes(p[34..42].try_into().ok()?);
                        Some(CoreMessage::AddOrder(AddOrderMsg {
                            instrument_id:  inst_id,
                            order_id,
                            price:          Price::from_raw(price_raw),
                            quantity:       qty,
                            side,
                            order_type:     otype,
                            update_time_ns: ts,
                        }))
                    }
                    TAG_MODIFY => {
                        let order_id  = u64::from_le_bytes(p[0..8].try_into().ok()?);
                        let inst_id   = u64::from_le_bytes(p[8..16].try_into().ok()?);
                        let price_raw = u64::from_le_bytes(p[16..24].try_into().ok()?);
                        let side      = u8_to_side(p[24]);
                        let qty       = u64::from_le_bytes(p[25..33].try_into().ok()?);
                        let ts        = u64::from_le_bytes(p[33..41].try_into().ok()?);
                        Some(CoreMessage::ModifyOrder(ModifyOrderMsg {
                            order_id,
                            instrument_id:  inst_id,
                            price:          Price::from_raw(price_raw),
                            side,
                            quantity:       qty,
                            update_time_ns: ts,
                        }))
                    }
                    TAG_CANCEL => {
                        let order_id = u64::from_le_bytes(p[0..8].try_into().ok()?);
                        let inst_id  = u64::from_le_bytes(p[8..16].try_into().ok()?);
                        Some(CoreMessage::CancelOrder(CancelOrderMsg {
                            order_id,
                            instrument_id: inst_id,
                        }))
                    }
                    _ => None,
                }
            }

            // ── Streaming helpers ──────────────────────────────────────────────

            pub fn read_exact_bytes<R: Read>(reader: &mut R, n: usize) -> io::Result<Vec<u8>> {
                let mut buf = vec![0u8; n];
                reader.read_exact(&mut buf)?;
                Ok(buf)
            }

            pub fn write_packet<W: Write>(writer: &mut W, packet: &Packet) -> io::Result<()> {
                writer.write_all(&encode(packet))
            }

            pub fn read_packet<R: Read>(reader: &mut R) -> io::Result<Option<Packet>> {
                let header = match read_exact_bytes(reader, HEADER_SIZE) {
                    Ok(h)  => h,
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
                    Err(e) => return Err(e),
                };
                let num_messages = u16::from_le_bytes([header[0], header[1]]) as usize;
                let payload_len  = u16::from_le_bytes([header[2], header[3]]) as usize;
                if num_messages == 0 || payload_len == 0 { return Ok(None); }
                let payload = read_exact_bytes(reader, payload_len)?;
                let mut all = Vec::with_capacity(HEADER_SIZE + payload_len);
                all.extend_from_slice(&header);
                all.extend_from_slice(&payload);
                Ok(decode(&all))
            }

            // ── Enum ↔ u8 helpers ─────────────────────────────────────────────

            fn side_to_u8(s: Side) -> u8 {
                match s { Side::Bid => 0, Side::Ask => 1, Side::Invalid => 2 }
            }
            fn u8_to_side(v: u8) -> Side {
                match v { 0 => Side::Bid, 1 => Side::Ask, _ => Side::Invalid }
            }
            fn order_type_to_u8(t: OrderType) -> u8 {
                match t {
                    OrderType::Ioc => 0, OrderType::Gfd => 1,
                    OrderType::Market => 2, OrderType::Invalid => 3,
                }
            }
            fn u8_to_order_type(v: u8) -> OrderType {
                match v {
                    0 => OrderType::Ioc, 1 => OrderType::Gfd,
                    2 => OrderType::Market, _ => OrderType::Invalid,
                }
            }
            fn parse_str(bytes: &[u8]) -> String {
                let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                String::from_utf8_lossy(&bytes[..end]).into_owned()
            }
        }
    };
}

// ── Instantiate the three precision tiers ─────────────────────────────────────

define_precision_codec!(price4, 4);   // FX pip-level      (e.g. 1.2345)
define_precision_codec!(price6, 6);   // equities / crypto  (e.g. 100.000001)
define_precision_codec!(price9, 9);   // high-precision     (e.g. 1.000000001)
