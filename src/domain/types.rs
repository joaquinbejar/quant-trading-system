use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Trading symbol identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol(pub String);

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Price with decimal precision to avoid floating-point errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Price(pub Decimal);

impl Price {
    /// Creates a zero price
    pub fn zero() -> Self {
        Price(Decimal::ZERO)
    }

    /// Returns true if the price is zero
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Quantity with decimal precision
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Quantity(pub Decimal);

impl Quantity {
    /// Creates a zero quantity
    pub fn zero() -> Self {
        Quantity(Decimal::ZERO)
    }

    /// Returns true if the quantity is zero
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Order side (bid or ask)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    /// Bid (buy) side
    Bid,
    /// Ask (sell) side
    Ask,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Side::Bid => write!(f, "BID"),
            Side::Ask => write!(f, "ASK"),
        }
    }
}

/// Sequence number for idempotent message processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SequenceNumber(pub u64);

impl fmt::Display for SequenceNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Update action for price level changes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateAction {
    /// Add or update a price level
    Update,
    /// Remove a price level
    Delete,
}

/// AMM pool address identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PoolAddress(pub String);

impl fmt::Display for PoolAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Token reserves in an AMM pool
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenReserves {
    /// Reserve amount for token0
    pub token0: Decimal,
    /// Reserve amount for token1
    pub token1: Decimal,
}

impl TokenReserves {
    /// Creates new token reserves with the given amounts
    pub fn new(token0: Decimal, token1: Decimal) -> Self {
        Self { token0, token1 }
    }
}

/// Square root price for Uniswap V3 style pools
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SqrtPriceX96(pub u128);

impl SqrtPriceX96 {
    /// Convert sqrtPriceX96 to regular price
    pub fn to_price(&self) -> Price {
        // sqrtPriceX96 = sqrt(price) * 2^96
        // So price = (sqrtPriceX96 / 2^96)^2

        // Use a safer conversion that handles large numbers better
        if self.0 == 0 {
            return Price(Decimal::ZERO);
        }

        // Convert to f64 for calculation, then back to Decimal
        let sqrt_price_f64 = self.0 as f64 / (2u128.pow(96) as f64);
        let price_f64 = sqrt_price_f64 * sqrt_price_f64;

        // Convert back to Decimal safely
        match Decimal::try_from(price_f64) {
            Ok(price) => Price(price),
            Err(_) => Price(Decimal::ZERO), // Fallback for conversion errors
        }
    }
}

/// Fee tier for AMM pools (in basis points)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeeTier(pub u32);

impl FeeTier {
    /// Convert to decimal (e.g., 30 basis points = 0.003)
    pub fn to_decimal(&self) -> Decimal {
        Decimal::from(self.0) / Decimal::from(10000u32)
    }
}

/// Timestamp for market events
pub type Timestamp = chrono::DateTime<chrono::Utc>;

/// Trading system errors
#[derive(Debug, thiserror::Error)]
pub enum TradingError {
    /// Invalid price value
    #[error("Invalid price: {0}")]
    /// Invalid price provided
    InvalidPrice(String),

    /// Invalid quantity value
    #[error("Invalid quantity: {0}")]
    /// Invalid quantity provided
    InvalidQuantity(String),

    /// Sequence number out of order
    #[error("Sequence out of order: expected {expected}, got {actual}")]
    /// Sequence number is out of expected order
    SequenceOutOfOrder {
        /// Expected sequence number
        expected: u64,
        /// Actual sequence number received
        actual: u64,
    },

    /// Insufficient liquidity for trade
    #[error("Insufficient liquidity: {0}")]
    /// Insufficient liquidity available for the requested trade
    InsufficientLiquidity(Decimal),

    /// Parse error
    #[error("Parse error: {0}")]
    /// Error parsing data
    ParseError(String),

    /// IO error
    #[error("IO error: {0}")]
    /// Input/output error
    IoError(#[from] std::io::Error),

    /// Lock error
    #[error("Lock error: {0}")]
    /// Error acquiring lock
    LockError(String),

    /// JSON error
    #[error("JSON error: {0}")]
    /// JSON serialization/deserialization error
    JsonError(#[from] serde_json::Error),
}

/// Result type for trading operations
pub type TradingResult<T> = Result<T, TradingError>;

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_price_ordering() {
        let p1 = Price(dec!(100.50));
        let p2 = Price(dec!(100.51));
        assert!(p1 < p2);
    }

    #[test]
    fn test_quantity_operations() {
        let q1 = Quantity(dec!(10.5));
        let q2 = Quantity(dec!(5.25));
        assert!(q1 > q2);
        assert!(!q1.is_zero());
        assert!(Quantity::zero().is_zero());
    }

    #[test]
    fn test_fee_tier_conversion() {
        let fee = FeeTier(30); // 30 basis points
        assert_eq!(fee.to_decimal(), dec!(0.003));
    }

    #[test]
    fn test_sqrt_price_conversion() {
        // Test with a simple case where sqrtPriceX96 represents sqrt(1) = 1
        let sqrt_price = SqrtPriceX96(2u128.pow(96)); // This represents sqrt(1)
        let price = sqrt_price.to_price();
        // Should be approximately 1.0, allowing for some precision loss
        assert!((price.0 - dec!(1.0)).abs() < dec!(0.1));
    }
}
