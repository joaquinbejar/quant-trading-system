use crate::domain::{events::*, types::*};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::RwLock;

/// Thread-safe L2 order book implementation
/// Uses BTreeMap for automatic price sorting and O(log n) operations
#[derive(Debug)]
pub struct OrderBookL2 {
    symbol: Symbol,
    /// Bids sorted by price (descending - highest first)
    bids: BTreeMap<Price, Quantity>,
    /// Asks sorted by price (ascending - lowest first)  
    asks: BTreeMap<Price, Quantity>,
    /// Last processed sequence number for idempotent replay
    last_sequence: SequenceNumber,
    /// Timestamp of last update
    last_update: Timestamp,
}

impl OrderBookL2 {
    /// Creates a new order book for the given symbol
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            last_sequence: SequenceNumber(0),
            last_update: chrono::Utc::now(),
        }
    }

    /// Apply a complete snapshot, replacing all existing data
    pub fn apply_snapshot(&mut self, snapshot: OrderBookSnapshot) -> TradingResult<()> {
        // Validate sequence number progression
        if snapshot.sequence.0 <= self.last_sequence.0 {
            return Err(TradingError::SequenceOutOfOrder {
                expected: self.last_sequence.0 + 1,
                actual: snapshot.sequence.0,
            });
        }

        self.bids = snapshot.bids;
        self.asks = snapshot.asks;
        self.last_sequence = snapshot.sequence;
        self.last_update = snapshot.timestamp;

        Ok(())
    }

    /// Apply incremental delta updates
    pub fn apply_delta(&mut self, delta: OrderBookDelta) -> TradingResult<()> {
        // Validate sequence number progression for idempotent replay
        if delta.sequence.0 <= self.last_sequence.0 {
            // Skip if we've already processed this sequence
            return Ok(());
        }

        if delta.sequence.0 != self.last_sequence.0 + 1 {
            return Err(TradingError::SequenceOutOfOrder {
                expected: self.last_sequence.0 + 1,
                actual: delta.sequence.0,
            });
        }

        // Apply each update in the delta
        for update in delta.updates {
            self.apply_price_level_update(update)?;
        }

        self.last_sequence = delta.sequence;
        self.last_update = delta.timestamp;

        Ok(())
    }

    /// Apply a single price level update
    fn apply_price_level_update(&mut self, update: PriceLevelUpdate) -> TradingResult<()> {
        let levels = match update.side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };

        match update.action {
            UpdateAction::Update => {
                if update.quantity.is_zero() {
                    levels.remove(&update.price);
                } else {
                    levels.insert(update.price, update.quantity);
                }
            }
            UpdateAction::Delete => {
                levels.remove(&update.price);
            }
        }

        Ok(())
    }

    /// Get atomic snapshot without blocking writers
    pub fn get_snapshot(&self) -> OrderBookSnapshot {
        OrderBookSnapshot::with_levels(
            self.last_sequence,
            self.last_update,
            self.bids.clone(),
            self.asks.clone(),
        )
    }

    /// Get best bid price (highest bid)
    pub fn best_bid(&self) -> Option<Price> {
        self.bids.keys().next_back().copied()
    }

    /// Get best ask price (lowest ask)
    pub fn best_ask(&self) -> Option<Price> {
        self.asks.keys().next().copied()
    }

    /// Get mid price if both sides have liquidity
    pub fn mid_price(&self) -> Option<Price> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(Price((bid.0 + ask.0) / rust_decimal::Decimal::from(2))),
            _ => None,
        }
    }

    /// Get spread between best bid and ask
    pub fn spread(&self) -> Option<Price> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(Price(ask.0 - bid.0)),
            _ => None,
        }
    }

    /// Get quantity at specific price level
    pub fn quantity_at_price(&self, side: Side, price: Price) -> Quantity {
        let levels = match side {
            Side::Bid => &self.bids,
            Side::Ask => &self.asks,
        };

        levels.get(&price).copied().unwrap_or(Quantity::zero())
    }

    /// Get depth (total quantity) up to a certain price
    pub fn depth_to_price(&self, side: Side, max_price: Price) -> Quantity {
        let levels = match side {
            Side::Bid => &self.bids,
            Side::Ask => &self.asks,
        };

        let mut total = rust_decimal::Decimal::ZERO;

        match side {
            Side::Bid => {
                // For bids, sum all levels >= max_price (since we want depth at or above this price)
                for (_price, quantity) in levels.range(max_price..) {
                    total += quantity.0;
                }
            }
            Side::Ask => {
                // For asks, sum all levels <= max_price (since we want depth at or below this price)
                for (_price, quantity) in levels.range(..=max_price) {
                    total += quantity.0;
                }
            }
        }

        Quantity(total)
    }

    /// Get the symbol for this order book
    pub fn symbol(&self) -> &Symbol {
        &self.symbol
    }

    /// Get last processed sequence number
    pub fn last_sequence(&self) -> SequenceNumber {
        self.last_sequence
    }

    /// Get timestamp of last update
    pub fn last_update(&self) -> Timestamp {
        self.last_update
    }
}

/// Thread-safe wrapper around OrderBookL2
#[derive(Debug, Clone)]
pub struct ThreadSafeOrderBook {
    inner: Arc<RwLock<OrderBookL2>>,
}

impl ThreadSafeOrderBook {
    /// Creates a new thread-safe order book for the given symbol
    pub fn new(symbol: Symbol) -> Self {
        Self {
            inner: Arc::new(RwLock::new(OrderBookL2::new(symbol))),
        }
    }

    /// Apply snapshot with write lock
    pub fn apply_snapshot(&self, snapshot: OrderBookSnapshot) -> TradingResult<()> {
        let mut book = self
            .inner
            .write()
            .map_err(|_| TradingError::LockError("Failed to acquire write lock".to_string()))?;
        book.apply_snapshot(snapshot)
    }

    /// Apply a delta update to the order book
    pub fn apply_delta(&self, delta: &OrderBookDelta) -> TradingResult<()> {
        let mut book = self
            .inner
            .write()
            .map_err(|_| TradingError::LockError("Failed to acquire write lock".to_string()))?;
        book.apply_delta(delta.clone())
    }

    /// Get a snapshot of the current order book state
    pub fn get_snapshot(&self) -> OrderBookSnapshot {
        self.inner
            .read()
            .expect("Failed to acquire read lock")
            .get_snapshot()
    }

    /// Get the best bid price
    pub fn best_bid(&self) -> Option<Price> {
        self.inner
            .read()
            .expect("Failed to acquire read lock")
            .best_bid()
    }

    /// Get the best ask price
    pub fn best_ask(&self) -> Option<Price> {
        self.inner
            .read()
            .expect("Failed to acquire read lock")
            .best_ask()
    }

    /// Get the mid price (average of best bid and ask)
    pub fn mid_price(&self) -> Option<Price> {
        self.inner
            .read()
            .expect("Failed to acquire read lock")
            .mid_price()
    }

    /// Get the spread (difference between best ask and bid)
    pub fn spread(&self) -> Option<Price> {
        self.inner
            .read()
            .expect("Failed to acquire read lock")
            .spread()
    }

    /// Get quantity available at a specific price level
    pub fn quantity_at_price(&self, side: Side, price: Price) -> Quantity {
        self.inner
            .read()
            .expect("Failed to acquire read lock")
            .quantity_at_price(side, price)
    }

    /// Get total depth up to a maximum price
    pub fn depth_to_price(&self, side: Side, max_price: Price) -> Quantity {
        self.inner
            .read()
            .expect("Failed to acquire read lock")
            .depth_to_price(side, max_price)
    }

    /// Get the symbol for this order book
    pub fn symbol(&self) -> Symbol {
        self.inner
            .read()
            .expect("Failed to acquire read lock")
            .symbol()
            .clone()
    }

    /// Get the last sequence number
    pub fn last_sequence(&self) -> SequenceNumber {
        self.inner
            .read()
            .expect("Failed to acquire read lock")
            .last_sequence()
    }

    /// Get the last update timestamp
    pub fn last_update(&self) -> Timestamp {
        self.inner
            .read()
            .expect("Failed to acquire read lock")
            .last_update()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_order_book_snapshot_application() {
        let mut book = OrderBookL2::new(Symbol("ETHUSDC".to_string()));

        let mut snapshot = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());
        snapshot
            .bids
            .insert(Price(dec!(100.0)), Quantity(dec!(10.0)));
        snapshot.bids.insert(Price(dec!(99.5)), Quantity(dec!(5.0)));
        snapshot
            .asks
            .insert(Price(dec!(100.5)), Quantity(dec!(8.0)));
        snapshot
            .asks
            .insert(Price(dec!(101.0)), Quantity(dec!(12.0)));

        book.apply_snapshot(snapshot).unwrap();

        assert_eq!(book.best_bid(), Some(Price(dec!(100.0))));
        assert_eq!(book.best_ask(), Some(Price(dec!(100.5))));
        assert_eq!(book.mid_price(), Some(Price(dec!(100.25))));
    }

    #[test]
    fn test_order_book_delta_application() {
        let mut book = OrderBookL2::new(Symbol("ETHUSDC".to_string()));

        // Apply initial snapshot
        let mut snapshot = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());
        snapshot
            .bids
            .insert(Price(dec!(100.0)), Quantity(dec!(10.0)));
        book.apply_snapshot(snapshot).unwrap();

        // Apply delta update
        let delta = OrderBookDelta::with_updates(
            SequenceNumber(2),
            chrono::Utc::now(),
            vec![PriceLevelUpdate::update(
                Side::Bid,
                Price(dec!(100.5)),
                Quantity(dec!(15.0)),
            )],
        );

        book.apply_delta(delta).unwrap();

        assert_eq!(book.best_bid(), Some(Price(dec!(100.5))));
        assert_eq!(
            book.quantity_at_price(Side::Bid, Price(dec!(100.5))),
            Quantity(dec!(15.0))
        );
    }

    #[test]
    fn test_idempotent_replay() {
        let mut book = OrderBookL2::new(Symbol("ETHUSDC".to_string()));

        let snapshot = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());
        book.apply_snapshot(snapshot).unwrap();

        let delta = OrderBookDelta::with_updates(
            SequenceNumber(2),
            chrono::Utc::now(),
            vec![PriceLevelUpdate::update(
                Side::Bid,
                Price(dec!(100.0)),
                Quantity(dec!(10.0)),
            )],
        );

        // Apply delta first time
        book.apply_delta(delta.clone()).unwrap();
        assert_eq!(
            book.quantity_at_price(Side::Bid, Price(dec!(100.0))),
            Quantity(dec!(10.0))
        );

        // Apply same delta again - should be idempotent
        book.apply_delta(delta).unwrap();
        assert_eq!(
            book.quantity_at_price(Side::Bid, Price(dec!(100.0))),
            Quantity(dec!(10.0))
        );
    }

    #[test]
    fn test_sequence_validation() {
        let mut book = OrderBookL2::new(Symbol("ETHUSDC".to_string()));

        let snapshot = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());
        book.apply_snapshot(snapshot).unwrap();

        // Try to apply delta with wrong sequence number
        let delta = OrderBookDelta::new(SequenceNumber(5), chrono::Utc::now()); // Should be 2

        let result = book.apply_delta(delta);
        assert!(result.is_err());

        if let Err(TradingError::SequenceOutOfOrder { expected, actual }) = result {
            assert_eq!(expected, 2);
            assert_eq!(actual, 5);
        } else {
            panic!("Expected SequenceOutOfOrder error");
        }
    }

    #[test]
    fn test_thread_safe_order_book() {
        let book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

        // Apply initial snapshot
        let mut snapshot = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());
        snapshot
            .bids
            .insert(Price(dec!(100.0)), Quantity(dec!(10.0)));
        book.apply_snapshot(snapshot).unwrap();

        // Test concurrent reads
        let book_clone = book.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let _ = book_clone.best_bid();
                let _ = book_clone.get_snapshot();
                thread::sleep(Duration::from_micros(1));
            }
        });

        // Concurrent reads from main thread
        for _ in 0..100 {
            let _ = book.best_ask();
            let _ = book.mid_price();
            thread::sleep(Duration::from_micros(1));
        }

        handle.join().unwrap();

        // Verify state is still consistent
        assert_eq!(book.best_bid(), Some(Price(dec!(100.0))));
    }

    #[test]
    fn test_depth_calculation() {
        let mut book = OrderBookL2::new(Symbol("ETHUSDC".to_string()));

        let mut snapshot = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());
        snapshot
            .bids
            .insert(Price(dec!(100.0)), Quantity(dec!(10.0)));
        snapshot.bids.insert(Price(dec!(99.5)), Quantity(dec!(5.0)));
        snapshot.bids.insert(Price(dec!(99.0)), Quantity(dec!(8.0)));

        snapshot
            .asks
            .insert(Price(dec!(100.5)), Quantity(dec!(12.0)));
        snapshot
            .asks
            .insert(Price(dec!(101.0)), Quantity(dec!(7.0)));

        book.apply_snapshot(snapshot).unwrap();

        // Test bid depth
        let bid_depth = book.depth_to_price(Side::Bid, Price(dec!(99.5)));
        assert_eq!(bid_depth, Quantity(dec!(15.0))); // 10.0 + 5.0

        // Test ask depth
        let ask_depth = book.depth_to_price(Side::Ask, Price(dec!(101.0)));
        assert_eq!(ask_depth, Quantity(dec!(19.0))); // 12.0 + 7.0
    }
}
