use crate::domain::{amm_pool::*, events::*, order_book::*, types::*};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

/// Market data manager that holds all order books and AMM pools
#[derive(Debug)]
pub struct MarketDataManager {
    /// Thread-safe order books indexed by symbol
    pub orderbooks: Arc<RwLock<HashMap<Symbol, ThreadSafeOrderBook>>>,
    /// Thread-safe AMM pools indexed by address
    pub amm_pools: Arc<RwLock<HashMap<PoolAddress, ThreadSafeAMMPool>>>,
}

impl MarketDataManager {
    /// Creates a new market data manager
    pub fn new() -> Self {
        Self {
            orderbooks: Arc::new(RwLock::new(HashMap::new())),
            amm_pools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Process a market event
    pub fn process_event(&self, event: MarketEvent) -> TradingResult<()> {
        match event {
            MarketEvent::OrderBookSnapshot(symbol, snapshot) => {
                self.apply_orderbook_snapshot(symbol, snapshot)
            }
            MarketEvent::OrderBookDelta(symbol, delta) => {
                self.apply_orderbook_delta(symbol, &delta)
            }
            MarketEvent::Trade(symbol, trade) => self.process_trade(symbol, trade),
            MarketEvent::AMMUpdate(address, update) => self.process_amm_update(address, update),
        }
    }

    /// Apply order book snapshot
    pub fn apply_orderbook_snapshot(
        &self,
        symbol: Symbol,
        snapshot: OrderBookSnapshot,
    ) -> TradingResult<()> {
        let mut books = self
            .orderbooks
            .write()
            .expect("Failed to acquire write lock");
        if let Some(book) = books.get(&symbol) {
            book.apply_snapshot(snapshot)?
        } else {
            // Create new order book if it doesn't exist
            let new_book = ThreadSafeOrderBook::new(symbol.clone());
            new_book.apply_snapshot(snapshot)?;
            books.insert(symbol, new_book);
        }
        Ok(())
    }

    /// Apply order book delta update
    pub fn apply_orderbook_delta(
        &self,
        symbol: Symbol,
        delta: &OrderBookDelta,
    ) -> TradingResult<()> {
        let books = self.orderbooks.read().expect("Failed to acquire read lock");
        if let Some(book) = books.get(&symbol) {
            book.apply_delta(delta)?
        } else {
            return Err(TradingError::ParseError(format!(
                "No order book found for symbol: {}",
                symbol
            )));
        }
        Ok(())
    }

    /// Process trade (for now, just log it - could be used for analytics)
    fn process_trade(&self, _symbol: Symbol, _trade: Trade) -> TradingResult<()> {
        // In a full implementation, this would update trade statistics,
        // volume metrics, etc.
        Ok(())
    }

    /// Process AMM pool update
    fn process_amm_update(&self, address: PoolAddress, update: AMMPoolUpdate) -> TradingResult<()> {
        let mut pools = self
            .amm_pools
            .write()
            .expect("Failed to acquire write lock");
        match pools.get(&address) {
            Some(pool) => {
                pool.update_state(update)?;
            }
            None => {
                // Create new pool - assume V2 style if no sqrt_price, V3 if sqrt_price exists
                let pool = match update.sqrt_price {
                    Some(sqrt_price) => ThreadSafeAMMPool::new_v3(
                        address.clone(),
                        update.reserves,
                        sqrt_price,
                        update.fee_tier,
                    ),
                    None => {
                        ThreadSafeAMMPool::new_v2(address.clone(), update.reserves, update.fee_tier)
                    }
                };
                pools.insert(address, pool);
            }
        }

        Ok(())
    }

    /// Get order book for symbol
    pub fn get_orderbook(&self, symbol: &Symbol) -> Option<ThreadSafeOrderBook> {
        let books = self.orderbooks.read().expect("Failed to acquire read lock");
        books.get(symbol).cloned()
    }

    /// Get AMM pool for address
    pub fn get_amm_pool(&self, address: &PoolAddress) -> Option<ThreadSafeAMMPool> {
        let pools = self.amm_pools.read().expect("Failed to acquire read lock");
        pools.get(address).cloned()
    }

    /// Add new AMM pool
    pub fn add_amm_pool(&self, address: PoolAddress, pool: ThreadSafeAMMPool) {
        let mut pools = self
            .amm_pools
            .write()
            .expect("Failed to acquire write lock");
        pools.insert(address, pool);
    }

    /// Get all order book symbols
    pub fn get_orderbook_symbols(&self) -> Vec<Symbol> {
        let books = self.orderbooks.read().expect("Failed to acquire read lock");
        books.keys().cloned().collect()
    }

    /// Get all AMM pool addresses
    pub fn get_amm_pool_addresses(&self) -> Vec<PoolAddress> {
        let pools = self.amm_pools.read().expect("Failed to acquire read lock");
        pools.keys().cloned().collect()
    }

    /// Get market metrics for all instruments
    pub fn get_market_metrics(&self) -> MarketMetrics {
        let mut metrics = MarketMetrics::new();

        // Collect order book metrics
        let books = self.orderbooks.read().expect("Failed to acquire read lock");
        for (symbol, book) in books.iter() {
            if let (Some(bid), Some(ask)) = (book.best_bid(), book.best_ask()) {
                let spread = ask.0 - bid.0;
                let mid_price = (bid.0 + ask.0) / rust_decimal::Decimal::from(2);

                metrics.orderbook_metrics.insert(
                    symbol.clone(),
                    OrderBookMetrics {
                        symbol: symbol.clone(),
                        best_bid: bid,
                        best_ask: ask,
                        mid_price: Price(mid_price),
                        spread,
                        last_update: book.last_update(),
                    },
                );
            }
        }

        // Collect AMM pool metrics
        let pools = self.amm_pools.read().expect("Failed to acquire read lock");
        for (address, pool) in pools.iter() {
            metrics.amm_metrics.insert(
                address.clone(),
                AMMMetrics {
                    address: address.clone(),
                    pool_type: pool.pool_type(),
                    implied_mid: pool.implied_mid(),
                    reserves: pool.get_reserves(),
                    fee_tier: pool.get_fee_tier(),
                    last_update: pool.last_update(),
                },
            );
        }

        metrics.timestamp = chrono::Utc::now();
        metrics
    }
}

impl Default for MarketDataManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Market metrics aggregated across all instruments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMetrics {
    /// Timestamp when metrics were collected
    pub timestamp: Timestamp,
    /// Order book metrics by symbol
    pub orderbook_metrics: HashMap<Symbol, OrderBookMetrics>,
    /// AMM pool metrics by pool address
    pub amm_metrics: HashMap<PoolAddress, AMMMetrics>,
}

impl MarketMetrics {
    /// Creates new market metrics with current timestamp
    pub fn new() -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            orderbook_metrics: HashMap::new(),
            amm_metrics: HashMap::new(),
        }
    }
}

impl Default for MarketMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Order book metrics for a specific symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookMetrics {
    /// Trading symbol
    pub symbol: Symbol,
    /// Best bid price
    pub best_bid: Price,
    /// Best ask price
    pub best_ask: Price,
    /// Mid price (average of best bid and ask)
    pub mid_price: Price,
    /// Bid-ask spread
    pub spread: rust_decimal::Decimal,
    /// Timestamp of last update
    pub last_update: Timestamp,
}

/// AMM pool metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AMMMetrics {
    /// Pool address
    pub address: PoolAddress,
    /// Type of AMM pool (V2/V3)
    pub pool_type: AMMPoolType,
    /// Implied mid price from reserves
    pub implied_mid: Price,
    /// Token reserves in the pool
    pub reserves: TokenReserves,
    /// Fee tier for the pool
    pub fee_tier: FeeTier,
    /// Timestamp of last update
    pub last_update: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_market_data_manager_creation() {
        let manager = MarketDataManager::new();
        assert!(manager.get_orderbook_symbols().is_empty());
        assert!(manager.get_amm_pool_addresses().is_empty());
    }

    #[test]
    fn test_orderbook_snapshot_processing() {
        let manager = MarketDataManager::new();
        let symbol = Symbol("ETHUSDC".to_string());

        let mut snapshot = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());
        snapshot
            .bids
            .insert(Price(dec!(2000.0)), Quantity(dec!(10.0)));
        snapshot
            .asks
            .insert(Price(dec!(2001.0)), Quantity(dec!(8.0)));

        let event = MarketEvent::OrderBookSnapshot(symbol.clone(), snapshot);
        manager.process_event(event).unwrap();

        let book = manager.get_orderbook(&symbol).unwrap();
        assert_eq!(book.best_bid(), Some(Price(dec!(2000.0))));
        assert_eq!(book.best_ask(), Some(Price(dec!(2001.0))));
    }

    #[test]
    fn test_orderbook_delta_processing() {
        let manager = MarketDataManager::new();
        let symbol = Symbol("ETHUSDC".to_string());

        // First, create initial snapshot
        let snapshot = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());
        let event = MarketEvent::OrderBookSnapshot(symbol.clone(), snapshot);
        manager.process_event(event).unwrap();

        // Then apply delta
        let delta = OrderBookDelta::with_updates(
            SequenceNumber(2),
            chrono::Utc::now(),
            vec![PriceLevelUpdate::update(
                Side::Bid,
                Price(dec!(1999.0)),
                Quantity(dec!(5.0)),
            )],
        );

        let event = MarketEvent::OrderBookDelta(symbol.clone(), delta);
        manager.process_event(event).unwrap();

        let book = manager.get_orderbook(&symbol).unwrap();
        assert_eq!(
            book.quantity_at_price(Side::Bid, Price(dec!(1999.0))),
            Quantity(dec!(5.0))
        );
    }

    #[test]
    fn test_amm_update_processing() {
        let manager = MarketDataManager::new();
        let address = PoolAddress("0x123".to_string());

        let reserves = TokenReserves::new(dec!(1000.0), dec!(2000.0));
        let update = AMMPoolUpdate::new(chrono::Utc::now(), reserves, None, FeeTier(30));

        let event = MarketEvent::AMMUpdate(address.clone(), update);
        manager.process_event(event).unwrap();

        let pool = manager.get_amm_pool(&address).unwrap();
        assert_eq!(pool.implied_mid(), Price(dec!(2.0)));
        assert_eq!(pool.pool_type(), AMMPoolType::ConstantProduct);
    }

    #[test]
    fn test_market_metrics_collection() {
        let manager = MarketDataManager::new();

        // Add order book
        let symbol = Symbol("ETHUSDC".to_string());
        let mut snapshot = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());
        snapshot
            .bids
            .insert(Price(dec!(2000.0)), Quantity(dec!(10.0)));
        snapshot
            .asks
            .insert(Price(dec!(2001.0)), Quantity(dec!(8.0)));
        let event = MarketEvent::OrderBookSnapshot(symbol.clone(), snapshot);
        manager.process_event(event).unwrap();

        // Add AMM pool
        let address = PoolAddress("0x123".to_string());
        let reserves = TokenReserves::new(dec!(1000.0), dec!(2000.0));
        let update = AMMPoolUpdate::new(chrono::Utc::now(), reserves, None, FeeTier(30));
        let event = MarketEvent::AMMUpdate(address.clone(), update);
        manager.process_event(event).unwrap();

        // Get metrics
        let metrics = manager.get_market_metrics();

        assert_eq!(metrics.orderbook_metrics.len(), 1);
        assert_eq!(metrics.amm_metrics.len(), 1);

        let ob_metrics = metrics.orderbook_metrics.get(&symbol).unwrap();
        assert_eq!(ob_metrics.best_bid, Price(dec!(2000.0)));
        assert_eq!(ob_metrics.best_ask, Price(dec!(2001.0)));
        assert_eq!(ob_metrics.mid_price, Price(dec!(2000.5)));

        let amm_metrics = metrics.amm_metrics.get(&address).unwrap();
        assert_eq!(amm_metrics.implied_mid, Price(dec!(2.0)));
    }
}
