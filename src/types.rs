/// Core type aliases mirroring MarketDataDefinitions.h

pub type InstrumentId = u64;
pub type Timestamp    = u64;
pub type OrderId      = u64;
pub type Quantity     = u64;
pub type Volume       = u64;

/// Which side of the book an order lives on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Bid,
    Ask,
    Invalid,
}

/// Order duration / execution type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    /// Immediate-or-Cancel: fill what you can right now, cancel the rest.
    Ioc,
    /// Good-for-Day: rest in the book until filled or cancelled.
    Gfd,
    /// Market order: fill at top-of-book (price = 0 internally).
    Market,
    Invalid,
}

/// Top-level action parsed from an input line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Buy,
    Sell,
    Cancel,
    Modify,
    Print,
    Symbol,
    Invalid,
}
