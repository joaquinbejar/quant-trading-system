use quant_trading_system::domain::{
    amm_pool::*, events::*, market_data::*, order_book::*, types::*,
};
use rust_decimal_macros::dec;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn test_concurrent_order_book_access() {
    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

    // Apply initial snapshot
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2401.0)), Quantity(dec!(10.0)));

    let snapshot =
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks);
    order_book.apply_snapshot(snapshot).unwrap();

    let order_book_clone = order_book.clone();

    // Spawn multiple threads to read from order book
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let ob = order_book_clone.clone();
            thread::spawn(move || {
                for _ in 0..100 {
                    let best_bid = ob.best_bid();
                    let best_ask = ob.best_ask();
                    assert!(best_bid.is_some());
                    assert!(best_ask.is_some());

                    thread::sleep(Duration::from_micros(1));
                }
            })
        })
        .collect();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify order book is still in valid state
    assert!(order_book.best_bid().is_some());
    assert!(order_book.best_ask().is_some());
}

#[test]
fn test_concurrent_amm_pool_access() {
    let amm_pool = ThreadSafeAMMPool::new_v2(
        PoolAddress("0x123".to_string()),
        TokenReserves::new(dec!(1000.0), dec!(2000.0)),
        FeeTier(30),
    );

    let pool_clone = amm_pool.clone();

    // Spawn multiple threads to read from AMM pool
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let pool = pool_clone.clone();
            thread::spawn(move || {
                for _ in 0..100 {
                    let mid_price = pool.implied_mid();
                    let reserves = pool.get_reserves();
                    let fee_tier = pool.get_fee_tier();

                    assert_eq!(mid_price, Price(dec!(2.0)));
                    assert_eq!(reserves.token0, dec!(1000.0));
                    assert_eq!(reserves.token1, dec!(2000.0));
                    assert_eq!(fee_tier, FeeTier(30));

                    thread::sleep(Duration::from_micros(1));
                }
            })
        })
        .collect();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_market_data_manager_access() {
    let manager = Arc::new(MarketDataManager::new());

    // Add initial data
    let symbol = Symbol("ETHUSDC".to_string());
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2401.0)), Quantity(dec!(10.0)));

    let snapshot_event = MarketEvent::OrderBookSnapshot(
        symbol.clone(),
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks),
    );
    manager.process_event(snapshot_event).unwrap();

    let manager_clone = Arc::clone(&manager);

    // Spawn multiple threads to access market data
    let handles: Vec<_> = (0..5)
        .map(|_| {
            let mgr = Arc::clone(&manager_clone);
            let sym = symbol.clone();
            thread::spawn(move || {
                for _ in 0..50 {
                    if let Some(order_book) = mgr.get_orderbook(&sym) {
                        let best_bid = order_book.best_bid();
                        let best_ask = order_book.best_ask();
                        assert!(best_bid.is_some());
                        assert!(best_ask.is_some());
                    }

                    let symbols = mgr.get_orderbook_symbols();
                    assert!(!symbols.is_empty());

                    thread::sleep(Duration::from_micros(1));
                }
            })
        })
        .collect();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_order_book_updates() {
    let order_book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

    // Apply initial snapshot
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2401.0)), Quantity(dec!(10.0)));

    let snapshot =
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks);
    order_book.apply_snapshot(snapshot).unwrap();

    let order_book_clone = order_book.clone();

    // Spawn threads that apply deltas concurrently
    let handles: Vec<_> = (0..5)
        .map(|i| {
            let ob = order_book_clone.clone();
            thread::spawn(move || {
                for j in 0..20 {
                    let seq = 1001 + (i * 20) + j;
                    let mut updates = Vec::new();
                    updates.push(PriceLevelUpdate::new(
                        Side::Bid,
                        Price(dec!(2399.0) + rust_decimal::Decimal::from(j)),
                        Quantity(dec!(1.0)),
                        UpdateAction::Update,
                    ));

                    let delta = OrderBookDelta::with_updates(
                        SequenceNumber(seq as u64),
                        chrono::Utc::now(),
                        updates,
                    );

                    let _ = ob.apply_delta(&delta);
                    thread::sleep(Duration::from_micros(10));
                }
            })
        })
        .collect();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify order book is still in valid state
    assert!(order_book.best_bid().is_some());
    assert!(order_book.best_ask().is_some());
}

#[test]
fn test_concurrent_amm_pool_updates() {
    let amm_pool = ThreadSafeAMMPool::new_v2(
        PoolAddress("0x123".to_string()),
        TokenReserves::new(dec!(1000.0), dec!(2000.0)),
        FeeTier(30),
    );

    let pool_clone = amm_pool.clone();

    // Spawn threads that update pool state concurrently
    let handles: Vec<_> = (0..3)
        .map(|i| {
            let pool = pool_clone.clone();
            thread::spawn(move || {
                for j in 0..10 {
                    let new_reserves = TokenReserves::new(
                        dec!(1000.0) + rust_decimal::Decimal::from(i * 10 + j),
                        dec!(2000.0) + rust_decimal::Decimal::from(i * 20 + j * 2),
                    );

                    let update =
                        AMMPoolUpdate::new(chrono::Utc::now(), new_reserves, None, FeeTier(30));

                    let _ = pool.update_state(update);
                    thread::sleep(Duration::from_micros(10));
                }
            })
        })
        .collect();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify pool is still in valid state
    let final_reserves = amm_pool.get_reserves();
    assert!(final_reserves.token0 > dec!(0.0));
    assert!(final_reserves.token1 > dec!(0.0));
}

#[test]
fn test_concurrent_price_impact_calculations() {
    let amm_pool = ThreadSafeAMMPool::new_v2(
        PoolAddress("0x123".to_string()),
        TokenReserves::new(dec!(10000.0), dec!(20000.0)),
        FeeTier(30),
    );

    let pool_clone = amm_pool.clone();

    // Spawn multiple threads calculating price impact
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let pool = pool_clone.clone();
            thread::spawn(move || {
                for j in 0..25 {
                    let trade_amount = dec!(1.0) + rust_decimal::Decimal::from(i + j);
                    let result = pool.calculate_price_impact(TokenIndex::Token0, trade_amount);

                    assert!(result.is_ok());
                    let impact = result.unwrap();
                    assert!(impact.output_amount > dec!(0.0));
                    assert!(impact.price_impact_percent >= dec!(0.0));

                    thread::sleep(Duration::from_micros(1));
                }
            })
        })
        .collect();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_metrics_collection() {
    let manager = Arc::new(MarketDataManager::new());

    // Add some initial data
    let symbol = Symbol("ETHUSDC".to_string());
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2401.0)), Quantity(dec!(10.0)));

    let snapshot_event = MarketEvent::OrderBookSnapshot(
        symbol.clone(),
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks),
    );
    manager.process_event(snapshot_event).unwrap();

    let manager_clone = Arc::clone(&manager);

    // Spawn multiple threads collecting metrics
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let mgr = Arc::clone(&manager_clone);
            thread::spawn(move || {
                for _ in 0..50 {
                    let metrics = mgr.get_market_metrics();
                    assert!(!metrics.orderbook_metrics.is_empty());

                    thread::sleep(Duration::from_micros(5));
                }
            })
        })
        .collect();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }
}
