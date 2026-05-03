/// Line-by-line parser for market-data text files.
///
/// Mirrors the parsing logic in `MarketDataFileReader.cpp`.
///
/// ─── File formats ────────────────────────────────────────────────────────────
///
/// Symbols file (`Symbols.txt`):
///   `SYMBOL <inst_id> <price> <name>`
///   e.g.  `SYMBOL 1 207.8 AAPL`
///
/// Order data file (`AAPLOrders.txt` etc.):
///   `BUY   <order_type> <inst_id> <price> <qty> <order_id>`
///   `SELL  <order_type> <inst_id> <price> <qty> <order_id>`
///   `MODIFY <inst_id> <order_id> <side> <price> <qty>`
///   `CANCEL <inst_id> <order_id>`
///
///   where <order_type> ∈ { GFD, IOC }
///         <side>       ∈ { BUY, SELL }
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// Import message types generic over P (must be re-exported from the correct module)
use crate::messages::{
    AddOrderMsg, CancelOrderMsg, CoreMessage, ModifyOrderMsg, SymbolMsg,
};
use crate::types::{Action, InstrumentId, OrderId, OrderType, Quantity, Side};



pub fn load_messages<P>(path: &Path) -> Result<Vec<CoreMessage<P>>, std::io::Error>
where
    P: Copy + From<f64>,
{
    load_messages_inner::<P>(path)
}

fn load_messages_inner<P>(path: &Path) -> Result<Vec<CoreMessage<P>>, std::io::Error>
where
    P: Copy + From<f64>,
{
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();

    for line in reader.lines() {
        let line = line?.trim().to_string();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(msg) = parse_line::<P>(&line) {
            messages.push(msg);
        }
    }

    Ok(messages)
}

// ── Core line dispatcher ──────────────────────────────────────────────────────

/// Parse a single non-empty, non-comment line into a `CoreMessage`.
fn parse_line<P>(line: &str) -> Option<CoreMessage<P>>
where
    P: Copy + From<f64>,
{
    // Split at the first space to get the action keyword.
    let (action_str, rest) = split_first(line)?;
    let action = parse_action(action_str);

    match action {
        Action::Symbol => parse_symbol::<P>(rest).map(CoreMessage::Symbol),
        Action::Buy    => parse_add_order::<P>(rest, Side::Bid).map(CoreMessage::AddOrder),
        Action::Sell   => parse_add_order::<P>(rest, Side::Ask).map(CoreMessage::AddOrder),
        Action::Modify => parse_modify::<P>(rest).map(CoreMessage::ModifyOrder),
        Action::Cancel => parse_cancel(rest).map(CoreMessage::CancelOrder),
        _              => None,
    }
}

// ── Token helpers ─────────────────────────────────────────────────────────────

/// Split `s` at the first ASCII space.  Returns `(before, after)`.
fn split_first(s: &str) -> Option<(&str, &str)> {
    let pos = s.find(' ')?;
    Some((&s[..pos], s[pos + 1..].trim_start()))
}

/// Parse action keyword.
fn parse_action(s: &str) -> Action {
    match s {
        "BUY"    => Action::Buy,
        "SELL"   => Action::Sell,
        "CANCEL" => Action::Cancel,
        "MODIFY" => Action::Modify,
        "PRINT"  => Action::Print,
        "SYMBOL" => Action::Symbol,
        _        => Action::Invalid,
    }
}

/// Parse side keyword.
fn parse_side(s: &str) -> Option<Side> {
    match s {
        "BUY"  => Some(Side::Bid),
        "SELL" => Some(Side::Ask),
        _      => None,
    }
}

/// Parse order type keyword.
fn parse_order_type(s: &str) -> Option<OrderType> {
    match s {
        "GFD" => Some(OrderType::Gfd),
        "IOC" => Some(OrderType::Ioc),
        _     => None,
    }
}

/// Current time in nanoseconds since Unix epoch.
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

// ── Individual parsers ────────────────────────────────────────────────────────

/// `<inst_id> <price> <symbol_name>`
fn parse_symbol<P>(rest: &str) -> Option<SymbolMsg<P>>
where
    P: Copy + From<f64>,
{
    let mut iter = rest.splitn(3, ' ');
    let inst_id:   InstrumentId = iter.next()?.parse().ok()?;
    let price_f64: f64         = iter.next()?.parse().ok()?;
    let price = P::from(price_f64);
    let symbol:    String      = iter.next()?.trim().to_string();
    Some(SymbolMsg { symbol, instrument_id: inst_id, last_price: price })
}

/// `<order_type> <inst_id> <price> <qty> <order_id>`  (side already known)
fn parse_add_order<P>(rest: &str, side: Side) -> Option<AddOrderMsg<P>>
where
    P: Copy + From<f64>,
{
    let mut tokens = rest.split_whitespace();
    let order_type: OrderType   = parse_order_type(tokens.next()?)?;
    let inst_id:    InstrumentId = tokens.next()?.parse().ok()?;
    let price_f64:  f64         = tokens.next()?.parse().ok()?;
    let price = P::from(price_f64);
    let qty:        Quantity     = tokens.next()?.parse().ok()?;
    let order_id:   OrderId      = tokens.next()?.parse().ok()?;

    Some(AddOrderMsg {
        instrument_id:  inst_id,
        order_id,
        price,
        quantity:       qty,
        side,
        order_type,
        update_time_ns: now_ns(),
    })
}

/// `<inst_id> <order_id> <side> <price> <qty>`
fn parse_modify<P>(rest: &str) -> Option<ModifyOrderMsg<P>>
where
    P: Copy + From<f64>,
{
    let mut tokens = rest.split_whitespace();
    let inst_id:  InstrumentId = tokens.next()?.parse().ok()?;
    let order_id: OrderId      = tokens.next()?.parse().ok()?;
    let side:     Side         = parse_side(tokens.next()?)?;
    let price_f64: f64         = tokens.next()?.parse().ok()?;
    let price = P::from(price_f64);
    let qty:      Quantity     = tokens.next()?.parse().ok()?;

    Some(ModifyOrderMsg {
        order_id,
        instrument_id:  inst_id,
        price,
        side,
        quantity:       qty,
        update_time_ns: now_ns(),
    })
}

/// `<inst_id> <order_id>`
fn parse_cancel(rest: &str) -> Option<CancelOrderMsg> {
    let mut tokens = rest.split_whitespace();
    let inst_id:  InstrumentId = tokens.next()?.parse().ok()?;
    let order_id: OrderId      = tokens.next()?.parse().ok()?;
    Some(CancelOrderMsg { order_id, instrument_id: inst_id })
}
