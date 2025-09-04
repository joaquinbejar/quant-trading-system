use crate::domain::{amm_pool::*, order_book::*, types::*};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Arbitrage opportunity between two venues
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    /// Venue where the asset should be bought
    pub buy_venue: Venue,
    /// Venue where the asset should be sold
    pub sell_venue: Venue,
    /// Price to buy at
    pub buy_price: Price,
    /// Price to sell at
    pub sell_price: Price,
    /// Maximum quantity that can be traded
    pub max_quantity: Quantity,
    /// Profit percentage
    pub profit_percent: Decimal,
    /// Estimated profit amount
    pub estimated_profit: Decimal,
}

impl ArbitrageOpportunity {
    /// Creates a new arbitrage opportunity
    pub fn new(
        buy_venue: Venue,
        sell_venue: Venue,
        buy_price: Price,
        sell_price: Price,
        max_quantity: Quantity,
    ) -> Self {
        let profit_per_unit = sell_price.0 - buy_price.0;
        let estimated_profit = profit_per_unit * max_quantity.0;
        let profit_percent = if !buy_price.is_zero() {
            (profit_per_unit / buy_price.0) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        Self {
            buy_venue,
            sell_venue,
            buy_price,
            sell_price,
            max_quantity,
            profit_percent,
            estimated_profit,
        }
    }
}

/// Trading venue identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Venue {
    /// Order book venue with symbol
    OrderBook(Symbol),
    /// AMM pool venue with address
    AMM(PoolAddress),
}

impl fmt::Display for Venue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Venue::OrderBook(symbol) => write!(f, "OrderBook({})", symbol),
            Venue::AMM(address) => write!(f, "AMM({})", address),
        }
    }
}

/// Arbitrage detector with configurable profit thresholds
#[derive(Debug)]
pub struct ArbitrageDetector {
    /// Minimum profit percentage to consider an opportunity viable (in basis points)
    min_profit_bps: u32,
    /// Maximum slippage tolerance (in basis points)
    max_slippage_bps: u32,
    /// Minimum trade size to consider
    min_trade_size: Decimal,
}

impl ArbitrageDetector {
    /// Create new arbitrage detector
    /// min_profit_bps: minimum profit in basis points (e.g., 10 = 0.1%)
    pub fn new(min_profit_bps: u32) -> Self {
        Self {
            min_profit_bps,
            max_slippage_bps: 50,               // 0.5% default max slippage
            min_trade_size: Decimal::from(100), // Default minimum trade size
        }
    }

    /// Create detector with custom parameters
    pub fn with_params(
        min_profit_bps: u32,
        max_slippage_bps: u32,
        min_trade_size: Decimal,
    ) -> Self {
        Self {
            min_profit_bps,
            max_slippage_bps,
            min_trade_size,
        }
    }

    /// Check for arbitrage opportunities between order book and AMM pool
    pub fn check_arbitrage(
        &self,
        order_book: &ThreadSafeOrderBook,
        amm_pool: &ThreadSafeAMMPool,
    ) -> Option<ArbitrageOpportunity> {
        let ob_symbol = order_book.symbol();
        let pool_address = amm_pool.get_address();

        // Get order book prices
        let ob_best_bid = order_book.best_bid()?;
        let ob_best_ask = order_book.best_ask()?;

        // Get AMM implied price
        let amm_mid = amm_pool.implied_mid();

        // Check both directions of arbitrage

        // Direction 1: Buy from AMM, sell to order book
        if let Some(opp1) = self.check_amm_to_orderbook(
            amm_pool,
            order_book,
            amm_mid,
            ob_best_bid,
            &pool_address,
            &ob_symbol,
        ) {
            return Some(opp1);
        }

        // Direction 2: Buy from order book, sell to AMM
        if let Some(opp2) = self.check_orderbook_to_amm(
            order_book,
            amm_pool,
            ob_best_ask,
            amm_mid,
            &ob_symbol,
            &pool_address,
        ) {
            return Some(opp2);
        }

        None
    }

    /// Check arbitrage: buy from AMM, sell to order book
    fn check_amm_to_orderbook(
        &self,
        amm_pool: &ThreadSafeAMMPool,
        order_book: &ThreadSafeOrderBook,
        amm_price: Price,
        ob_best_bid: Price,
        pool_address: &PoolAddress,
        ob_symbol: &Symbol,
    ) -> Option<ArbitrageOpportunity> {
        // Check if order book bid is higher than AMM price (profitable to buy from AMM, sell to OB)
        if ob_best_bid.0 <= amm_price.0 {
            return None;
        }

        let profit_per_unit = ob_best_bid.0 - amm_price.0;
        let profit_percent = (profit_per_unit / amm_price.0) * Decimal::from(10000); // Convert to basis points

        if profit_percent < Decimal::from(self.min_profit_bps) {
            return None;
        }

        // Calculate maximum tradeable quantity considering:
        // 1. Order book liquidity at best bid
        // 2. AMM price impact
        let ob_liquidity = order_book.quantity_at_price(Side::Bid, ob_best_bid);
        let max_amm_trade = self.calculate_max_amm_trade(amm_pool, amm_price);

        let max_quantity = Quantity(ob_liquidity.0.min(max_amm_trade.0).max(self.min_trade_size));

        if max_quantity.0 < self.min_trade_size {
            return None;
        }

        Some(ArbitrageOpportunity::new(
            Venue::AMM(pool_address.clone()),
            Venue::OrderBook(ob_symbol.clone()),
            amm_price,
            ob_best_bid,
            max_quantity,
        ))
    }

    /// Check arbitrage: buy from order book, sell to AMM
    fn check_orderbook_to_amm(
        &self,
        order_book: &ThreadSafeOrderBook,
        amm_pool: &ThreadSafeAMMPool,
        ob_best_ask: Price,
        amm_price: Price,
        ob_symbol: &Symbol,
        pool_address: &PoolAddress,
    ) -> Option<ArbitrageOpportunity> {
        // Check if AMM price is higher than order book ask (profitable to buy from OB, sell to AMM)
        if amm_price.0 <= ob_best_ask.0 {
            return None;
        }

        let profit_per_unit = amm_price.0 - ob_best_ask.0;
        let profit_percent = (profit_per_unit / ob_best_ask.0) * Decimal::from(10000); // Convert to basis points

        if profit_percent < Decimal::from(self.min_profit_bps) {
            return None;
        }

        // Calculate maximum tradeable quantity
        let ob_liquidity = order_book.quantity_at_price(Side::Ask, ob_best_ask);
        let max_amm_trade = self.calculate_max_amm_trade(amm_pool, amm_price);

        let max_quantity = Quantity(ob_liquidity.0.min(max_amm_trade.0).max(self.min_trade_size));

        if max_quantity.0 < self.min_trade_size {
            return None;
        }

        Some(ArbitrageOpportunity::new(
            Venue::OrderBook(ob_symbol.clone()),
            Venue::AMM(pool_address.clone()),
            ob_best_ask,
            amm_price,
            max_quantity,
        ))
    }

    /// Calculate maximum trade size for AMM considering slippage tolerance
    fn calculate_max_amm_trade(
        &self,
        amm_pool: &ThreadSafeAMMPool,
        _current_price: Price,
    ) -> Quantity {
        let max_slippage_decimal = Decimal::from(self.max_slippage_bps) / Decimal::from(10000);
        let reserves = amm_pool.get_reserves();

        // For constant product AMM, calculate max trade size that keeps slippage under threshold
        // This is a simplified calculation - in production would need more sophisticated modeling
        let max_trade_estimate = reserves.token0 * max_slippage_decimal;

        Quantity(max_trade_estimate.max(self.min_trade_size))
    }

    /// Check multiple order books against multiple AMM pools
    pub fn scan_opportunities(
        &self,
        order_books: &[ThreadSafeOrderBook],
        amm_pools: &[ThreadSafeAMMPool],
    ) -> Vec<ArbitrageOpportunity> {
        let mut opportunities = Vec::new();

        for order_book in order_books {
            for amm_pool in amm_pools {
                if let Some(opportunity) = self.check_arbitrage(order_book, amm_pool) {
                    opportunities.push(opportunity);
                }
            }
        }

        // Sort by profit percentage (descending)
        opportunities.sort_by(|a, b| b.profit_percent.cmp(&a.profit_percent));

        opportunities
    }

    /// Get minimum profit threshold in basis points
    pub fn min_profit_bps(&self) -> u32 {
        self.min_profit_bps
    }

    /// Get maximum slippage tolerance in basis points
    pub fn max_slippage_bps(&self) -> u32 {
        self.max_slippage_bps
    }

    /// Get minimum trade size
    pub fn min_trade_size(&self) -> Decimal {
        self.min_trade_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{events::OrderBookSnapshot, types::TokenReserves};
    use rust_decimal_macros::dec;

    fn create_test_order_book() -> ThreadSafeOrderBook {
        let book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

        let mut snapshot = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());
        snapshot
            .bids
            .insert(Price(dec!(2000.0)), Quantity(dec!(10.0)));
        snapshot
            .bids
            .insert(Price(dec!(1999.0)), Quantity(dec!(5.0)));
        snapshot
            .asks
            .insert(Price(dec!(2001.0)), Quantity(dec!(8.0)));
        snapshot
            .asks
            .insert(Price(dec!(2002.0)), Quantity(dec!(12.0)));

        book.apply_snapshot(snapshot).unwrap();
        book
    }

    fn create_test_amm_pool(price_ratio: Decimal) -> ThreadSafeAMMPool {
        // Create pool with specific price ratio (token1/token0)
        let token0_reserve = dec!(1000.0);
        let token1_reserve = token0_reserve * price_ratio;
        let reserves = TokenReserves::new(token0_reserve, token1_reserve);

        ThreadSafeAMMPool::new_v2(PoolAddress("0x123".to_string()), reserves, FeeTier(30))
    }

    #[test]
    fn test_arbitrage_detection_amm_to_orderbook() {
        let detector = ArbitrageDetector::new(10); // 0.1% minimum profit
        let order_book = create_test_order_book();

        // Create AMM with lower price (1950) than order book best bid (2000)
        let amm_pool = create_test_amm_pool(dec!(1950.0));

        let opportunity = detector.check_arbitrage(&order_book, &amm_pool);

        assert!(opportunity.is_some());
        let opp = opportunity.unwrap();

        assert_eq!(opp.buy_venue, Venue::AMM(PoolAddress("0x123".to_string())));
        assert_eq!(
            opp.sell_venue,
            Venue::OrderBook(Symbol("ETHUSDC".to_string()))
        );
        assert_eq!(opp.buy_price, Price(dec!(1950.0)));
        assert_eq!(opp.sell_price, Price(dec!(2000.0)));
        assert!(opp.profit_percent > dec!(0.0));
    }

    #[test]
    fn test_arbitrage_detection_orderbook_to_amm() {
        let detector = ArbitrageDetector::new(10); // 0.1% minimum profit
        let order_book = create_test_order_book();

        // Create AMM with higher price (2050) than order book best ask (2001)
        let amm_pool = create_test_amm_pool(dec!(2050.0));

        let opportunity = detector.check_arbitrage(&order_book, &amm_pool);

        assert!(opportunity.is_some());
        let opp = opportunity.unwrap();

        assert_eq!(
            opp.buy_venue,
            Venue::OrderBook(Symbol("ETHUSDC".to_string()))
        );
        assert_eq!(opp.sell_venue, Venue::AMM(PoolAddress("0x123".to_string())));
        assert_eq!(opp.buy_price, Price(dec!(2001.0)));
        assert_eq!(opp.sell_price, Price(dec!(2050.0)));
        assert!(opp.profit_percent > dec!(0.0));
    }

    #[test]
    fn test_no_arbitrage_opportunity() {
        let detector = ArbitrageDetector::new(10); // 0.1% minimum profit
        let order_book = create_test_order_book();

        // Create AMM with price within the spread (2000.5)
        let amm_pool = create_test_amm_pool(dec!(2000.5));

        let opportunity = detector.check_arbitrage(&order_book, &amm_pool);

        assert!(opportunity.is_none());
    }

    #[test]
    fn test_insufficient_profit_threshold() {
        let detector = ArbitrageDetector::new(500); // 5% minimum profit (very high)
        let order_book = create_test_order_book();

        // Create AMM with small arbitrage opportunity
        let amm_pool = create_test_amm_pool(dec!(1990.0));

        let opportunity = detector.check_arbitrage(&order_book, &amm_pool);

        // Should be None because profit is below 5% threshold
        assert!(opportunity.is_none());
    }

    #[test]
    fn test_scan_multiple_opportunities() {
        let detector = ArbitrageDetector::new(10);

        let order_books = vec![create_test_order_book()];

        let amm_pools = vec![
            create_test_amm_pool(dec!(1950.0)), // Should create arbitrage opportunity
            create_test_amm_pool(dec!(2000.5)), // No opportunity (within spread)
            create_test_amm_pool(dec!(2050.0)), // Should create arbitrage opportunity
        ];

        let opportunities = detector.scan_opportunities(&order_books, &amm_pools);

        // Should find 2 opportunities
        assert_eq!(opportunities.len(), 2);

        // Should be sorted by profit percentage (descending)
        assert!(opportunities[0].profit_percent >= opportunities[1].profit_percent);
    }

    #[test]
    fn test_arbitrage_opportunity_calculations() {
        let opp = ArbitrageOpportunity::new(
            Venue::OrderBook(Symbol("ETHUSDC".to_string())),
            Venue::AMM(PoolAddress("0x123".to_string())),
            Price(dec!(2000.0)),
            Price(dec!(2020.0)),
            Quantity(dec!(10.0)),
        );

        assert_eq!(opp.estimated_profit, dec!(200.0)); // (2020 - 2000) * 10
        assert_eq!(opp.profit_percent, dec!(1.0)); // (20 / 2000) * 100 = 1%
    }

    #[test]
    fn test_detector_configuration() {
        let detector = ArbitrageDetector::with_params(
            25,         // 0.25% min profit
            100,        // 1% max slippage
            dec!(50.0), // min trade size
        );

        assert_eq!(detector.min_profit_bps(), 25);
        assert_eq!(detector.max_slippage_bps(), 100);
        assert_eq!(detector.min_trade_size(), dec!(50.0));
    }
}
