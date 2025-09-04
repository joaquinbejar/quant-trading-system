use quant_trading_system::domain::{amm_pool::*, arbitrage::*, events::*, order_book::*, types::*};
use rust_decimal_macros::dec;
use std::collections::BTreeMap;

#[test]
fn test_arbitrage_detector_creation() {
    let _detector = ArbitrageDetector::new(10); // 0.1% min profit

    // Test with custom parameters
    let _custom_detector = ArbitrageDetector::with_params(50, 200, dec!(1.0));

    // Should create successfully without panicking
}

#[test]
fn test_arbitrage_opportunity_creation() {
    let buy_venue = Venue::AMM(PoolAddress("0x123".to_string()));
    let sell_venue = Venue::OrderBook(Symbol("ETHUSDC".to_string()));
    let buy_price = Price(dec!(2400.0));
    let sell_price = Price(dec!(2450.0));
    let max_quantity = Quantity(dec!(10.0));

    let opportunity = ArbitrageOpportunity::new(
        buy_venue.clone(),
        sell_venue.clone(),
        buy_price,
        sell_price,
        max_quantity,
    );

    assert_eq!(opportunity.buy_venue, buy_venue);
    assert_eq!(opportunity.sell_venue, sell_venue);
    assert_eq!(opportunity.buy_price, buy_price);
    assert_eq!(opportunity.sell_price, sell_price);
    assert_eq!(opportunity.max_quantity, max_quantity);

    // Check profit calculation
    let expected_profit_percent = (dec!(50.0) / dec!(2400.0)) * dec!(100);
    assert!((opportunity.profit_percent - expected_profit_percent).abs() < dec!(0.01));
}

#[test]
fn test_no_arbitrage_when_prices_equal() {
    let detector = ArbitrageDetector::new(10);

    // Create order book with equal prices
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2401.0)), Quantity(dec!(10.0)));

    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));
    let snapshot =
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks);
    order_book.apply_snapshot(snapshot).unwrap();

    // Create AMM pool with similar price
    let reserves = TokenReserves::new(dec!(1000.0), dec!(2400000.0)); // Price = 2400
    let amm_pool =
        ThreadSafeAMMPool::new_v2(PoolAddress("0x123".to_string()), reserves, FeeTier(30));

    let result = detector.check_arbitrage(&order_book, &amm_pool);
    assert!(result.is_none()); // No arbitrage opportunity
}

#[test]
fn test_arbitrage_amm_to_orderbook() {
    let detector = ArbitrageDetector::new(10); // 0.1% min profit

    // Create order book with higher bid
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2450.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2451.0)), Quantity(dec!(10.0)));

    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));
    let snapshot =
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks);
    order_book.apply_snapshot(snapshot).unwrap();

    // Create AMM pool with lower price
    let reserves = TokenReserves::new(dec!(1000.0), dec!(2400000.0)); // Price = 2400
    let amm_pool =
        ThreadSafeAMMPool::new_v2(PoolAddress("0x123".to_string()), reserves, FeeTier(30));

    let result = detector.check_arbitrage(&order_book, &amm_pool);
    assert!(result.is_some());

    let opportunity = result.unwrap();
    match opportunity.buy_venue {
        Venue::AMM(_) => {} // AMM venue displays correctly
        _ => panic!("Expected AMM as buy venue"),
    }
    match opportunity.sell_venue {
        Venue::OrderBook(_) => {} // OrderBook venue displays correctly
        _ => panic!("Expected OrderBook as sell venue"),
    }
}

#[test]
fn test_minimum_profit_threshold() {
    let detector = ArbitrageDetector::new(500); // 5% min profit

    // Create small price difference (1%)
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2424.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2425.0)), Quantity(dec!(10.0)));

    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));
    let snapshot =
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks);
    order_book.apply_snapshot(snapshot).unwrap();

    // AMM price lower by 1%
    let reserves = TokenReserves::new(dec!(1000.0), dec!(2400000.0)); // Price = 2400
    let amm_pool =
        ThreadSafeAMMPool::new_v2(PoolAddress("0x123".to_string()), reserves, FeeTier(30));

    let result = detector.check_arbitrage(&order_book, &amm_pool);
    assert!(result.is_none()); // Should not detect due to minimum threshold
}

#[test]
fn test_profit_calculation_accuracy() {
    let buy_price = Price(dec!(2000.0));
    let sell_price = Price(dec!(2100.0));
    let max_quantity = Quantity(dec!(5.0));

    let opportunity = ArbitrageOpportunity::new(
        Venue::AMM(PoolAddress("0x123".to_string())),
        Venue::OrderBook(Symbol("ETHUSDC".to_string())),
        buy_price,
        sell_price,
        max_quantity,
    );

    // Profit per unit = 2100 - 2000 = 100
    // Profit percent = (100 / 2000) * 100 = 5%
    // Estimated profit = 100 * 5 = 500

    assert_eq!(opportunity.profit_percent, dec!(5.0));
    assert_eq!(opportunity.estimated_profit, dec!(500.0));
}

#[test]
fn test_zero_price_handling() {
    let opportunity = ArbitrageOpportunity::new(
        Venue::AMM(PoolAddress("0x123".to_string())),
        Venue::OrderBook(Symbol("ETHUSDC".to_string())),
        Price::zero(),
        Price(dec!(100.0)),
        Quantity(dec!(1.0)),
    );

    // Should handle zero buy price gracefully
    assert_eq!(opportunity.profit_percent, dec!(0.0));
}

#[test]
fn test_venue_display() {
    let amm_venue = Venue::AMM(PoolAddress("0xABC123".to_string()));
    let ob_venue = Venue::OrderBook(Symbol("ETHUSDC".to_string()));

    let amm_str = format!("{}", amm_venue);
    let ob_str = format!("{}", ob_venue);

    assert!(amm_str.contains("AMM"));
    assert!(amm_str.contains("0xABC123"));
    assert!(ob_str.contains("OrderBook"));
    assert!(ob_str.contains("ETHUSDC"));
}
