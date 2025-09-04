use quant_trading_system::domain::{
    amm_pool::*, arbitrage::*, events::*, market_data::*, types::*,
};
use rust_decimal_macros::dec;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_full_market_data_processing_pipeline() {
    let manager = Arc::new(MarketDataManager::new());
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();

    // We'll process events directly without the ingester for this test

    // Send order book snapshot
    let symbol = Symbol("ETHUSDC".to_string());
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2401.0)), Quantity(dec!(10.0)));

    let snapshot_event = MarketEvent::OrderBookSnapshot(
        symbol.clone(),
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks),
    );

    event_tx.send(snapshot_event).unwrap();

    // Process the event
    if let Some(event) = event_rx.recv().await {
        manager.process_event(event).unwrap();
    }

    // Verify order book was created and populated
    let order_book = manager.get_orderbook(&symbol).unwrap();
    assert!(order_book.best_bid().is_some());
    assert!(order_book.best_ask().is_some());
    assert_eq!(order_book.best_bid().unwrap(), Price(dec!(2400.0)));
    assert_eq!(order_book.best_ask().unwrap(), Price(dec!(2401.0)));

    // Send a delta update
    let updates = vec![PriceLevelUpdate::new(
        Side::Bid,
        Price(dec!(2399.5)),
        Quantity(dec!(5.0)),
        UpdateAction::Update,
    )];

    let delta_event = MarketEvent::OrderBookDelta(
        symbol.clone(),
        OrderBookDelta::with_updates(SequenceNumber(1001), chrono::Utc::now(), updates),
    );

    event_tx.send(delta_event).unwrap();

    // Process the delta
    if let Some(event) = event_rx.recv().await {
        manager.process_event(event).unwrap();
    }

    // Verify the update was applied
    let updated_order_book = manager.get_orderbook(&symbol).unwrap();
    assert!(updated_order_book.best_bid().is_some());
    // Best bid should still be 2400.0 since 2399.5 is lower
    assert_eq!(updated_order_book.best_bid().unwrap(), Price(dec!(2400.0)));
}

#[tokio::test]
async fn test_arbitrage_detection_integration() {
    let manager = Arc::new(MarketDataManager::new());
    let detector = ArbitrageDetector::new(10); // 0.1% min profit

    // Set up order book with higher prices
    let symbol = Symbol("ETHUSDC".to_string());
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2450.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2451.0)), Quantity(dec!(10.0)));

    let snapshot_event = MarketEvent::OrderBookSnapshot(
        symbol.clone(),
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks),
    );

    manager.process_event(snapshot_event).unwrap();

    // Set up AMM pool with lower price
    let pool_address = PoolAddress("0x123".to_string());
    let reserves = TokenReserves::new(dec!(1000.0), dec!(2400000.0)); // Price = 2400
    let amm_pool = ThreadSafeAMMPool::new_v2(pool_address.clone(), reserves, FeeTier(30));

    manager.add_amm_pool(pool_address.clone(), amm_pool.clone());

    // Check for arbitrage opportunity
    let order_book = manager.get_orderbook(&symbol).unwrap();
    let opportunity = detector.check_arbitrage(&order_book, &amm_pool);

    assert!(opportunity.is_some());
    let opp = opportunity.unwrap();
    assert!(opp.estimated_profit > dec!(0.0));
    // Check that we're buying from AMM (lower price) and selling to OrderBook (higher price)
    assert_eq!(opp.buy_venue, Venue::AMM(pool_address));
    assert_eq!(opp.sell_venue, Venue::OrderBook(symbol));
}

#[tokio::test]
async fn test_concurrent_event_processing() {
    let manager = Arc::new(MarketDataManager::new());
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();

    // Spawn a task to process events
    let manager_clone = Arc::clone(&manager);
    let processing_task = tokio::spawn(async move {
        let mut processed_count = 0;
        while let Some(event) = event_rx.recv().await {
            if manager_clone.process_event(event).is_ok() {
                processed_count += 1;
            }
            if processed_count >= 10 {
                break;
            }
        }
        processed_count
    });

    // Send multiple events concurrently
    for i in 0..10 {
        let symbol = Symbol(format!("PAIR{}", i));
        let mut bids = BTreeMap::new();
        let mut asks = BTreeMap::new();
        bids.insert(
            Price(dec!(100.0) + rust_decimal::Decimal::from(i)),
            Quantity(dec!(10.0)),
        );
        asks.insert(
            Price(dec!(101.0) + rust_decimal::Decimal::from(i)),
            Quantity(dec!(10.0)),
        );

        let snapshot_event = MarketEvent::OrderBookSnapshot(
            symbol,
            OrderBookSnapshot::with_levels(
                SequenceNumber(1000 + i as u64),
                chrono::Utc::now(),
                bids,
                asks,
            ),
        );

        event_tx.send(snapshot_event).unwrap();
    }

    // Wait for processing to complete
    let processed_count = processing_task.await.unwrap();
    assert_eq!(processed_count, 10);

    // Verify all symbols were processed
    let symbols = manager.get_orderbook_symbols();
    assert_eq!(symbols.len(), 10);
}

#[tokio::test]
async fn test_market_metrics_integration() {
    let manager = Arc::new(MarketDataManager::new());

    // Add multiple order books
    for i in 0..5 {
        let symbol = Symbol(format!("PAIR{}", i));
        let mut bids = BTreeMap::new();
        let mut asks = BTreeMap::new();
        bids.insert(
            Price(dec!(100.0) + rust_decimal::Decimal::from(i)),
            Quantity(dec!(10.0)),
        );
        asks.insert(
            Price(dec!(101.0) + rust_decimal::Decimal::from(i)),
            Quantity(dec!(10.0)),
        );

        let snapshot_event = MarketEvent::OrderBookSnapshot(
            symbol,
            OrderBookSnapshot::with_levels(
                SequenceNumber(1000 + i as u64),
                chrono::Utc::now(),
                bids,
                asks,
            ),
        );

        manager.process_event(snapshot_event).unwrap();
    }

    // Add AMM pools
    for i in 0..3 {
        let pool_address = PoolAddress(format!("0x{:x}", i));
        let reserves = TokenReserves::new(
            dec!(1000.0) + rust_decimal::Decimal::from(i * 100),
            dec!(2000.0) + rust_decimal::Decimal::from(i * 200),
        );
        let amm_pool = ThreadSafeAMMPool::new_v2(pool_address.clone(), reserves, FeeTier(30));

        manager.add_amm_pool(pool_address, amm_pool);
    }

    // Get metrics
    let metrics = manager.get_market_metrics();

    assert_eq!(metrics.orderbook_metrics.len(), 5);
    assert_eq!(metrics.amm_metrics.len(), 3);

    // Verify metrics contain expected data
    for (symbol, ob_metrics) in &metrics.orderbook_metrics {
        assert!(ob_metrics.best_bid > Price::zero());
        assert!(ob_metrics.best_ask > Price::zero());
        assert!(ob_metrics.spread > dec!(0.0));
        assert!(symbol.0.starts_with("PAIR"));
    }

    for (address, amm_metrics) in &metrics.amm_metrics {
        assert!(amm_metrics.implied_mid > Price::zero());
        assert!(amm_metrics.reserves.token0 > dec!(0.0));
        assert!(amm_metrics.reserves.token1 > dec!(0.0));
        assert!(address.0.starts_with("0x"));
    }
}

#[tokio::test]
async fn test_error_handling_and_recovery() {
    let manager = Arc::new(MarketDataManager::new());

    // Test processing invalid sequence number
    let symbol = Symbol("ETHUSDC".to_string());
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2401.0)), Quantity(dec!(10.0)));

    // Send initial snapshot
    let snapshot_event = MarketEvent::OrderBookSnapshot(
        symbol.clone(),
        OrderBookSnapshot::with_levels(
            SequenceNumber(1000),
            chrono::Utc::now(),
            bids.clone(),
            asks.clone(),
        ),
    );

    manager.process_event(snapshot_event).unwrap();

    // Try to send a snapshot with lower sequence number (should fail)
    let invalid_snapshot = MarketEvent::OrderBookSnapshot(
        symbol.clone(),
        OrderBookSnapshot::with_levels(
            SequenceNumber(999), // Lower than previous
            chrono::Utc::now(),
            bids,
            asks,
        ),
    );

    let result = manager.process_event(invalid_snapshot);
    assert!(result.is_err());

    // Verify the original order book is still intact
    let order_book = manager.get_orderbook(&symbol).unwrap();
    assert!(order_book.best_bid().is_some());
    assert!(order_book.best_ask().is_some());
}

#[tokio::test]
async fn test_high_frequency_updates() {
    let manager = Arc::new(MarketDataManager::new());
    let symbol = Symbol("ETHUSDC".to_string());

    // Initialize with snapshot
    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();
    bids.insert(Price(dec!(2400.0)), Quantity(dec!(10.0)));
    asks.insert(Price(dec!(2401.0)), Quantity(dec!(10.0)));

    let snapshot_event = MarketEvent::OrderBookSnapshot(
        symbol.clone(),
        OrderBookSnapshot::with_levels(SequenceNumber(1000), chrono::Utc::now(), bids, asks),
    );

    manager.process_event(snapshot_event).unwrap();

    // Send rapid delta updates
    for i in 1..=100 {
        let updates = vec![PriceLevelUpdate::new(
            Side::Bid,
            Price(dec!(2400.0) + rust_decimal::Decimal::from(i) / dec!(100.0)),
            Quantity(dec!(1.0)),
            UpdateAction::Update,
        )];

        let delta_event = MarketEvent::OrderBookDelta(
            symbol.clone(),
            OrderBookDelta::with_updates(SequenceNumber(1000 + i), chrono::Utc::now(), updates),
        );

        let result = manager.process_event(delta_event);
        assert!(result.is_ok());
    }

    // Verify final state
    let order_book = manager.get_orderbook(&symbol).unwrap();
    assert!(order_book.best_bid().is_some());
    let best_bid = order_book.best_bid().unwrap();
    // Should be the highest bid price from our updates (2400.0 + 1.0 = 2401.0)
    assert!(best_bid.0 >= dec!(2401.0));
}

#[tokio::test]
async fn test_amm_pool_state_updates() {
    let manager = Arc::new(MarketDataManager::new());
    let pool_address = PoolAddress("0xabc123".to_string());

    // Create initial AMM pool
    let initial_reserves = TokenReserves::new(dec!(1000.0), dec!(2000.0));
    let amm_pool = ThreadSafeAMMPool::new_v2(pool_address.clone(), initial_reserves, FeeTier(30));

    manager.add_amm_pool(pool_address.clone(), amm_pool.clone());

    // Verify initial state
    let initial_mid = amm_pool.implied_mid();
    assert_eq!(initial_mid, Price(dec!(2.0)));

    // Update pool state
    let new_reserves = TokenReserves::new(dec!(1100.0), dec!(2100.0));
    let update = AMMPoolUpdate::new(chrono::Utc::now(), new_reserves, None, FeeTier(30));

    amm_pool.update_state(update).unwrap();

    // Verify updated state
    let updated_reserves = amm_pool.get_reserves();
    assert_eq!(updated_reserves.token0, dec!(1100.0));
    assert_eq!(updated_reserves.token1, dec!(2100.0));

    let updated_mid = amm_pool.implied_mid();
    assert!(updated_mid.0 > dec!(1.9) && updated_mid.0 < dec!(2.0));
}
