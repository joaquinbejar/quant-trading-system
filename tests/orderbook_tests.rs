use quant_trading_system::domain::{events::*, order_book::*, types::*};
use rust_decimal_macros::dec;
use std::collections::BTreeMap;

#[test]
fn test_order_book_creation() {
    let symbol = Symbol("ETHUSDC".to_string());
    let order_book = ThreadSafeOrderBook::new(symbol.clone());

    assert!(order_book.best_bid().is_none());
    assert!(order_book.best_ask().is_none());
    assert_eq!(order_book.symbol(), symbol);
}

#[test]
fn test_order_book_snapshot_application() {
    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    bids.insert(Price(dec!(2399.0)), Quantity(dec!(5.0)));
    asks.insert(Price(dec!(2401.0)), Quantity(dec!(8.0)));
    asks.insert(Price(dec!(2402.0)), Quantity(dec!(12.0)));

    let snapshot =
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks);

    let result = order_book.apply_snapshot(snapshot);
    assert!(result.is_ok());

    // Verify best prices
    assert_eq!(order_book.best_bid().unwrap(), Price(dec!(2400.0)));
    assert_eq!(order_book.best_ask().unwrap(), Price(dec!(2401.0)));

    // Verify bid/ask quantities
    assert_eq!(
        order_book.quantity_at_price(Side::Bid, Price(dec!(2400.0))),
        Quantity(dec!(10.0))
    );
    assert_eq!(
        order_book.quantity_at_price(Side::Ask, Price(dec!(2401.0))),
        Quantity(dec!(8.0))
    );
}

#[test]
fn test_order_book_delta_updates() {
    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

    // Apply initial snapshot
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2401.0)), Quantity(dec!(10.0)));

    let snapshot =
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks);
    order_book.apply_snapshot(snapshot).unwrap();

    // Apply delta update - add new bid level
    let updates = vec![PriceLevelUpdate::new(
        Side::Bid,
        Price(dec!(2399.5)),
        Quantity(dec!(5.0)),
        UpdateAction::Update,
    )];

    let delta = OrderBookDelta::with_updates(SequenceNumber(1001), chrono::Utc::now(), updates);

    let result = order_book.apply_delta(&delta);
    assert!(result.is_ok());

    // Verify the new bid level was added
    assert_eq!(
        order_book.quantity_at_price(Side::Bid, Price(dec!(2399.5))),
        Quantity(dec!(5.0))
    );
    // Best bid should still be 2400.0
    assert_eq!(order_book.best_bid().unwrap(), Price(dec!(2400.0)));
}

#[test]
fn test_order_book_price_level_deletion() {
    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

    // Apply initial snapshot with multiple levels
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    bids.insert(Price(dec!(2399.0)), Quantity(dec!(5.0)));
    asks.insert(Price(dec!(2401.0)), Quantity(dec!(8.0)));
    asks.insert(Price(dec!(2402.0)), Quantity(dec!(12.0)));

    let snapshot =
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks);
    order_book.apply_snapshot(snapshot).unwrap();

    // Delete the best bid
    let updates = vec![PriceLevelUpdate::new(
        Side::Bid,
        Price(dec!(2400.0)),
        Quantity::zero(),
        UpdateAction::Delete,
    )];

    let delta = OrderBookDelta::with_updates(SequenceNumber(1001), chrono::Utc::now(), updates);

    order_book.apply_delta(&delta).unwrap();

    // Best bid should now be 2399.0
    assert_eq!(order_book.best_bid().unwrap(), Price(dec!(2399.0)));
    // The deleted price level should not exist
    assert_eq!(
        order_book.quantity_at_price(Side::Bid, Price(dec!(2400.0))),
        Quantity::zero()
    );
}

#[test]
fn test_order_book_spread_calculation() {
    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2401.5)), Quantity(dec!(8.0)));

    let snapshot =
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks);
    order_book.apply_snapshot(snapshot).unwrap();

    let spread = order_book.spread();
    assert!(spread.is_some());
    assert_eq!(spread.unwrap(), Price(dec!(1.5))); // 2401.5 - 2400.0
}

#[test]
fn test_order_book_mid_price_calculation() {
    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2402.0)), Quantity(dec!(8.0)));

    let snapshot =
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks);
    order_book.apply_snapshot(snapshot).unwrap();

    let mid_price = order_book.mid_price();
    assert!(mid_price.is_some());
    assert_eq!(mid_price.unwrap(), Price(dec!(2401.0))); // (2400 + 2402) / 2
}

#[test]
fn test_order_book_sequence_validation() {
    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

    // Apply initial snapshot
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2401.0)), Quantity(dec!(10.0)));

    let snapshot = OrderBookSnapshot::with_levels(
        SequenceNumber(1000),
        chrono::Utc::now(),
        bids.clone(),
        asks.clone(),
    );
    order_book.apply_snapshot(snapshot).unwrap();

    // Try to apply snapshot with lower sequence number (should fail)
    let invalid_snapshot =
        OrderBookSnapshot::with_levels(SequenceNumber(999), chrono::Utc::now(), bids, asks);

    let result = order_book.apply_snapshot(invalid_snapshot);
    assert!(result.is_err());

    // Try to apply delta with non-sequential sequence number (should be skipped for idempotent replay)
    let updates = vec![PriceLevelUpdate::new(
        Side::Bid,
        Price(dec!(2399.0)),
        Quantity(dec!(5.0)),
        UpdateAction::Update,
    )];

    let old_delta = OrderBookDelta::with_updates(
        SequenceNumber(999), // Lower than current - should be skipped
        chrono::Utc::now(),
        updates.clone(),
    );

    let result = order_book.apply_delta(&old_delta);
    assert!(result.is_ok()); // Should succeed but be skipped

    // Try to apply delta with gap in sequence number (should fail)
    let gap_delta = OrderBookDelta::with_updates(
        SequenceNumber(1002), // Gap in sequence - should fail
        chrono::Utc::now(),
        updates,
    );

    let result = order_book.apply_delta(&gap_delta);
    assert!(result.is_err());
}

#[test]
fn test_order_book_empty_state() {
    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

    // Test empty order book behavior
    assert!(order_book.best_bid().is_none());
    assert!(order_book.best_ask().is_none());
    assert!(order_book.mid_price().is_none());
    assert!(order_book.spread().is_none());
    assert_eq!(
        order_book.quantity_at_price(Side::Bid, Price(dec!(2400.0))),
        Quantity::zero()
    );
    assert_eq!(
        order_book.quantity_at_price(Side::Ask, Price(dec!(2401.0))),
        Quantity::zero()
    );
}

#[test]
fn test_order_book_partial_state() {
    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

    // Apply snapshot with only bids
    let mut bids = BTreeMap::new();
    let asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));

    let snapshot =
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks);
    order_book.apply_snapshot(snapshot).unwrap();

    // Should have bid but no ask
    assert!(order_book.best_bid().is_some());
    assert!(order_book.best_ask().is_none());
    assert!(order_book.mid_price().is_none());
    assert!(order_book.spread().is_none());
}

#[test]
fn test_order_book_price_level_updates() {
    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

    // Apply initial snapshot
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2401.0)), Quantity(dec!(10.0)));

    let snapshot =
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks);
    order_book.apply_snapshot(snapshot).unwrap();

    // Update existing bid quantity
    let updates = vec![PriceLevelUpdate::new(
        Side::Bid,
        Price(dec!(2400.0)),
        Quantity(dec!(15.0)), // Increased quantity
        UpdateAction::Update,
    )];

    let delta = OrderBookDelta::with_updates(SequenceNumber(1001), chrono::Utc::now(), updates);

    order_book.apply_delta(&delta).unwrap();

    // Verify quantity was updated
    assert_eq!(
        order_book.quantity_at_price(Side::Bid, Price(dec!(2400.0))),
        Quantity(dec!(15.0))
    );
}

#[test]
fn test_order_book_multiple_price_levels() {
    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

    // Create order book with multiple price levels
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();

    // Multiple bid levels (descending prices)
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    bids.insert(Price(dec!(2399.5)), Quantity(dec!(8.0)));
    bids.insert(Price(dec!(2399.0)), Quantity(dec!(5.0)));
    bids.insert(Price(dec!(2398.5)), Quantity(dec!(3.0)));

    // Multiple ask levels (ascending prices)
    asks.insert(Price(dec!(2401.0)), Quantity(dec!(7.0)));
    asks.insert(Price(dec!(2401.5)), Quantity(dec!(9.0)));
    asks.insert(Price(dec!(2402.0)), Quantity(dec!(12.0)));
    asks.insert(Price(dec!(2402.5)), Quantity(dec!(15.0)));

    let snapshot =
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks);
    order_book.apply_snapshot(snapshot).unwrap();

    // Verify best prices (highest bid, lowest ask)
    assert_eq!(order_book.best_bid().unwrap(), Price(dec!(2400.0)));
    assert_eq!(order_book.best_ask().unwrap(), Price(dec!(2401.0)));

    // Verify all levels exist
    assert_eq!(
        order_book.quantity_at_price(Side::Bid, Price(dec!(2398.5))),
        Quantity(dec!(3.0))
    );
    assert_eq!(
        order_book.quantity_at_price(Side::Ask, Price(dec!(2402.5))),
        Quantity(dec!(15.0))
    );
}

#[test]
fn test_order_book_cross_spread_handling() {
    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

    // Create crossed order book (bid > ask) - this is an invalid state but should be handled
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2402.0)), Quantity(dec!(10.0))); // Higher than ask
    asks.insert(Price(dec!(2401.0)), Quantity(dec!(10.0))); // Lower than bid

    let snapshot =
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks);

    // Should still apply the snapshot (system doesn't validate cross)
    let result = order_book.apply_snapshot(snapshot);
    assert!(result.is_ok());

    // Verify the crossed state
    assert_eq!(order_book.best_bid().unwrap(), Price(dec!(2402.0)));
    assert_eq!(order_book.best_ask().unwrap(), Price(dec!(2401.0)));

    // Spread should be negative in this case
    let spread = order_book.spread().unwrap();
    assert!(spread.0 < dec!(0.0));
}
