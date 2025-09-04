use crate::domain::types::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Price level in an order book
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceLevel {
    /// Price at this level
    pub price: Price,
    /// Quantity available at this price
    pub quantity: Quantity,
}

impl PriceLevel {
    /// Creates a new price level with the given price and quantity
    pub fn new(price: Price, quantity: Quantity) -> Self {
        Self { price, quantity }
    }
}

/// Price level update for order book deltas
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceLevelUpdate {
    /// Order side (bid or ask)
    pub side: Side,
    /// Price level
    pub price: Price,
    /// Quantity at this price level
    pub quantity: Quantity,
    /// Action to perform (update or delete)
    pub action: UpdateAction,
}

impl PriceLevelUpdate {
    /// Creates a new price level update
    pub fn new(side: Side, price: Price, quantity: Quantity, action: UpdateAction) -> Self {
        Self {
            side,
            price,
            quantity,
            action,
        }
    }

    /// Creates an update action for the given side, price, and quantity
    pub fn update(side: Side, price: Price, quantity: Quantity) -> Self {
        Self::new(side, price, quantity, UpdateAction::Update)
    }

    /// Creates a delete action for the given side and price
    pub fn delete(side: Side, price: Price) -> Self {
        Self::new(side, price, Quantity::zero(), UpdateAction::Delete)
    }
}

/// Complete order book snapshot with full state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    /// Sequence number for ordering
    pub sequence: SequenceNumber,
    /// Timestamp of the snapshot
    pub timestamp: Timestamp,
    /// Bid levels (price -> quantity)
    pub bids: BTreeMap<Price, Quantity>,
    /// Ask levels (price -> quantity)
    pub asks: BTreeMap<Price, Quantity>,
}

impl OrderBookSnapshot {
    /// Creates a new empty order book snapshot
    pub fn new(sequence: SequenceNumber, timestamp: Timestamp) -> Self {
        Self {
            sequence,
            timestamp,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    /// Creates a new order book snapshot with the given levels
    pub fn with_levels(
        sequence: SequenceNumber,
        timestamp: Timestamp,
        bids: BTreeMap<Price, Quantity>,
        asks: BTreeMap<Price, Quantity>,
    ) -> Self {
        Self {
            sequence,
            timestamp,
            bids,
            asks,
        }
    }

    /// Get the best bid (highest price on bid side)
    pub fn best_bid(&self) -> Option<&Price> {
        self.bids.keys().next_back()
    }

    /// Get the best ask (lowest price on ask side)
    pub fn best_ask(&self) -> Option<&Price> {
        self.asks.keys().next()
    }

    /// Get the mid price if both sides have liquidity
    pub fn mid_price(&self) -> Option<Price> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(Price((bid.0 + ask.0) / rust_decimal::Decimal::from(2))),
            _ => None,
        }
    }

    /// Get the spread between best bid and ask
    pub fn spread(&self) -> Option<Price> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(Price(ask.0 - bid.0)),
            _ => None,
        }
    }
}

/// Incremental order book delta with incremental updates
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderBookDelta {
    /// Sequence number for ordering
    pub sequence: SequenceNumber,
    /// Timestamp of the delta
    pub timestamp: Timestamp,
    /// List of price level updates
    pub updates: Vec<PriceLevelUpdate>,
}

impl OrderBookDelta {
    /// Creates a new empty order book delta
    pub fn new(sequence: SequenceNumber, timestamp: Timestamp) -> Self {
        Self {
            sequence,
            timestamp,
            updates: Vec::new(),
        }
    }

    /// Creates a new order book delta with the given updates
    pub fn with_updates(
        sequence: SequenceNumber,
        timestamp: Timestamp,
        updates: Vec<PriceLevelUpdate>,
    ) -> Self {
        Self {
            sequence,
            timestamp,
            updates,
        }
    }

    /// Adds a price level update to this delta
    pub fn add_update(&mut self, update: PriceLevelUpdate) {
        self.updates.push(update);
    }
}

/// Trade execution event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trade {
    /// Sequence number for ordering
    pub sequence: SequenceNumber,
    /// Timestamp of the trade
    pub timestamp: Timestamp,
    /// Trade price
    pub price: Price,
    /// Trade quantity
    pub quantity: Quantity,
    /// Taker side of the trade
    pub side: Side,
}

impl Trade {
    /// Creates a new trade event
    pub fn new(
        sequence: SequenceNumber,
        timestamp: Timestamp,
        price: Price,
        quantity: Quantity,
        side: Side,
    ) -> Self {
        Self {
            sequence,
            timestamp,
            price,
            quantity,
            side,
        }
    }
}

/// AMM pool state update
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AMMPoolUpdate {
    /// Timestamp of the update
    pub timestamp: Timestamp,
    /// Updated token reserves
    pub reserves: TokenReserves,
    /// Updated square root price (for V3 pools)
    pub sqrt_price: Option<SqrtPriceX96>,
    /// Pool fee tier
    pub fee_tier: FeeTier,
}

impl AMMPoolUpdate {
    /// Creates a new AMM pool update
    pub fn new(
        timestamp: Timestamp,
        reserves: TokenReserves,
        sqrt_price: Option<SqrtPriceX96>,
        fee_tier: FeeTier,
    ) -> Self {
        Self {
            timestamp,
            reserves,
            sqrt_price,
            fee_tier,
        }
    }

    /// Calculate implied mid price from reserves (Uniswap V2 style)
    pub fn implied_mid_from_reserves(&self) -> Price {
        if self.reserves.token0.is_zero() {
            return Price::zero();
        }
        Price(self.reserves.token1 / self.reserves.token0)
    }

    /// Get implied mid price (prefer sqrt_price if available, otherwise use reserves)
    pub fn implied_mid(&self) -> Price {
        match self.sqrt_price {
            Some(sqrt_price) => sqrt_price.to_price(),
            None => self.implied_mid_from_reserves(),
        }
    }
}

/// Market events that can be processed by the system
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketEvent {
    /// Order book snapshot
    OrderBookSnapshot(Symbol, OrderBookSnapshot),
    /// Order book delta update
    OrderBookDelta(Symbol, OrderBookDelta),
    /// Trade execution
    Trade(Symbol, Trade),
    /// AMM pool update
    AMMUpdate(PoolAddress, AMMPoolUpdate),
}

impl MarketEvent {
    /// Returns the timestamp of this market event
    pub fn timestamp(&self) -> Timestamp {
        match self {
            MarketEvent::OrderBookSnapshot(_, snapshot) => snapshot.timestamp,
            MarketEvent::OrderBookDelta(_, delta) => delta.timestamp,
            MarketEvent::Trade(_, trade) => trade.timestamp,
            MarketEvent::AMMUpdate(_, update) => update.timestamp,
        }
    }

    /// Returns the sequence number if available
    pub fn sequence(&self) -> Option<SequenceNumber> {
        match self {
            MarketEvent::OrderBookSnapshot(_, snapshot) => Some(snapshot.sequence),
            MarketEvent::OrderBookDelta(_, delta) => Some(delta.sequence),
            MarketEvent::Trade(_, trade) => Some(trade.sequence),
            MarketEvent::AMMUpdate(_, _) => None, // AMM updates don't have sequence numbers
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_order_book_snapshot_best_prices() {
        let mut snapshot = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());

        // Add some bid levels
        snapshot
            .bids
            .insert(Price(dec!(100.0)), Quantity(dec!(10.0)));
        snapshot.bids.insert(Price(dec!(99.5)), Quantity(dec!(5.0)));

        // Add some ask levels
        snapshot
            .asks
            .insert(Price(dec!(100.5)), Quantity(dec!(8.0)));
        snapshot
            .asks
            .insert(Price(dec!(101.0)), Quantity(dec!(12.0)));

        assert_eq!(snapshot.best_bid(), Some(&Price(dec!(100.0))));
        assert_eq!(snapshot.best_ask(), Some(&Price(dec!(100.5))));
        assert_eq!(snapshot.mid_price(), Some(Price(dec!(100.25))));
        assert_eq!(snapshot.spread(), Some(Price(dec!(0.5))));
    }

    #[test]
    fn test_amm_pool_implied_price() {
        let reserves = TokenReserves::new(dec!(1000.0), dec!(2000.0));
        let update = AMMPoolUpdate::new(chrono::Utc::now(), reserves, None, FeeTier(30));

        let implied_price = update.implied_mid_from_reserves();
        assert_eq!(implied_price, Price(dec!(2.0))); // 2000 / 1000 = 2.0
    }

    #[test]
    fn test_price_level_update_creation() {
        let update = PriceLevelUpdate::update(Side::Bid, Price(dec!(100.0)), Quantity(dec!(5.0)));

        assert_eq!(update.side, Side::Bid);
        assert_eq!(update.price, Price(dec!(100.0)));
        assert_eq!(update.quantity, Quantity(dec!(5.0)));
        assert_eq!(update.action, UpdateAction::Update);

        let delete = PriceLevelUpdate::delete(Side::Ask, Price(dec!(101.0)));
        assert_eq!(delete.action, UpdateAction::Delete);
        assert!(delete.quantity.is_zero());
    }
}
