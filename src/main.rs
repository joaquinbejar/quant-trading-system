//! # Quant Trading System
//!
//! A high-performance quantitative trading system with real-time market data processing,
//! order book management, AMM pool integration, and arbitrage detection.
//!
//! This binary provides an example entry point that demonstrates the full system capabilities
//! including market data ingestion, real-time processing, arbitrage monitoring, and metrics API.

use quant_trading_system::utils::logger::setup_logger;
use quant_trading_system::*;
use rust_decimal_macros::dec;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::error;
use tracing::log::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing/logging
    info!("Initializing tracing subscriber...");
    setup_logger().expect("Failed to initialize logger");
    info!("Tracing subscriber initialized");

    info!("Starting Quant Trading System v{}", VERSION);
    info!("Initializing system components...");

    // Create the market data manager
    info!("Creating market data manager...");
    let manager = Arc::new(MarketDataManager::new());
    info!("Market data manager created successfully");

    // Create channels for different data streams
    info!("Setting up communication channels...");
    let (market_tx, market_rx) = mpsc::channel::<MarketEvent>(1000);
    info!("Channels created with buffer size 1000");

    // Start the stream ingester FIRST
    info!("Starting stream ingester task...");
    let manager_clone = Arc::clone(&manager);
    let _ingester_handle = tokio::spawn(async move {
        info!("Stream ingester task started");
        let mut ingester = StreamIngester::new(market_rx, manager_clone);
        info!("Running stream ingester...");
        ingester.run().await;
    });
    info!("Stream ingester task spawned");

    // Load initial data from JSON files
    info!("Loading initial market data from JSON files...");
    load_initial_data(&market_tx).await?;
    info!("Initial market data loaded successfully");

    // Give some time for events to be processed
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    info!("Events processed, continuing with setup...");

    // Start real-time simulation (simulated here with periodic updates)
    info!("Starting real-time simulation task...");
    let market_tx_clone = market_tx.clone();
    tokio::spawn(async move {
        info!("Real-time simulation task started");
        simulate_real_time_updates(market_tx_clone).await;
    });
    info!("Real-time simulation task spawned");

    // Start arbitrage monitoring
    info!("Starting arbitrage monitoring task...");
    let manager_clone = Arc::clone(&manager);
    tokio::spawn(async move {
        info!("Arbitrage monitoring task started");
        monitor_arbitrage(manager_clone).await;
    });
    info!("Arbitrage monitoring task spawned");

    // Start metrics API server
    info!("Starting metrics API server...");
    expose_metrics_api(manager).await;
    info!("Metrics API server started");

    Ok(())
}

async fn load_initial_data(tx: &mpsc::Sender<MarketEvent>) -> anyhow::Result<()> {
    info!("Loading order book snapshot from data/LOB_snapshot.json...");

    // Load snapshot from file
    match load_lob_snapshot("data/LOB_snapshot.json") {
        Ok((symbol, snapshot)) => {
            info!("Loaded snapshot for symbol: {}", symbol);
            tx.send(MarketEvent::OrderBookSnapshot(symbol, snapshot))
                .await?;
        }
        Err(e) => {
            error!("Failed to load order book snapshot: {}", e);
        }
    }

    // Load delta updates
    info!("Loading order book delta from data/LOB_delta.json...");
    match load_lob_delta("data/LOB_delta.json") {
        Ok((symbol, delta)) => {
            info!(
                "Loaded delta for symbol: {}, seq: {}",
                symbol, delta.sequence
            );
            tx.send(MarketEvent::OrderBookDelta(symbol, delta)).await?;
        }
        Err(e) => {
            error!("Failed to load order book delta: {}", e);
        }
    }

    // Load AMM pool state
    info!("Loading AMM pool data from data/amm_pool.json...");
    match load_amm_pool("data/amm_pool.json") {
        Ok((address, update)) => {
            info!("Loaded AMM pool for address: {}", address);
            tx.send(MarketEvent::AMMUpdate(address, update)).await?;
        }
        Err(e) => {
            error!("Failed to load AMM pool: {}", e);
        }
    }

    info!("Initial data loading completed");
    Ok(())
}

async fn simulate_real_time_updates(tx: mpsc::Sender<MarketEvent>) {
    info!("Starting real-time market data simulation...");

    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
    let mut seq = 1001u64;

    loop {
        interval.tick().await;

        // Simulate order book updates
        let delta = OrderBookDelta::with_updates(
            SequenceNumber(seq),
            chrono::Utc::now(),
            vec![
                PriceLevelUpdate::update(
                    Side::Bid,
                    Price(dec!(2445) + rust_decimal::Decimal::from(seq % 10)),
                    Quantity(dec!(1) + rust_decimal::Decimal::from(seq % 5)),
                ),
                PriceLevelUpdate::update(
                    Side::Ask,
                    Price(dec!(2446) + rust_decimal::Decimal::from(seq % 10)),
                    Quantity(dec!(1) + rust_decimal::Decimal::from(seq % 3)),
                ),
            ],
        );

        if tx
            .send(MarketEvent::OrderBookDelta(
                Symbol("ETHUSDC".to_string()),
                delta,
            ))
            .await
            .is_err()
        {
            break; // Channel closed
        }

        // Occasionally simulate AMM pool updates
        if seq % 50 == 0 {
            let new_reserves = TokenReserves::new(
                dec!(1000) + rust_decimal::Decimal::from(seq % 100),
                dec!(2445000) + rust_decimal::Decimal::from(seq % 10000),
            );

            let amm_update =
                AMMPoolUpdate::new(chrono::Utc::now(), new_reserves, None, FeeTier(30));

            if tx
                .send(MarketEvent::AMMUpdate(
                    PoolAddress("0xPOOL".to_string()),
                    amm_update,
                ))
                .await
                .is_err()
            {
                break;
            }
        }

        seq += 1;

        // Stop after some time for demo purposes
        if seq > 2000 {
            info!("Simulation completed after {} updates", seq - 1001);
            break;
        }
    }
}

async fn monitor_arbitrage(manager: Arc<MarketDataManager>) {
    info!("Starting arbitrage monitoring loop...");

    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
    let detector = ArbitrageDetector::new(10); // 0.1% min profit

    loop {
        interval.tick().await;

        // Get all order books and AMM pools
        let symbols = manager.get_orderbook_symbols();
        let addresses = manager.get_amm_pool_addresses();

        // Check each combination for arbitrage
        for symbol in &symbols {
            if let Some(book) = manager.get_orderbook(symbol) {
                for address in &addresses {
                    if let Some(pool) = manager.get_amm_pool(address) {
                        if let Some(opportunity) = detector.check_arbitrage(&book, &pool) {
                            info!(
                                "ARBITRAGE DETECTED! Buy {} @ {:.2}, Sell {} @ {:.2}, Profit: {:.2}%, Max Qty: {}",
                                opportunity.buy_venue,
                                opportunity.buy_price,
                                opportunity.sell_venue,
                                opportunity.sell_price,
                                opportunity.profit_percent,
                                opportunity.max_quantity
                            );
                        }
                    }
                }
            }
        }
    }
}

async fn expose_metrics_api(manager: Arc<MarketDataManager>) {
    info!("Starting metrics API server on port 3030...");
    info!("Setting up API routes...");

    use warp::Filter;

    let manager = warp::any().map(move || Arc::clone(&manager));

    // Metrics endpoint
    let metrics = warp::path("metrics")
        .and(warp::get())
        .and(manager.clone())
        .map(|mgr: Arc<MarketDataManager>| {
            let metrics = mgr.get_market_metrics();
            warp::reply::json(&metrics)
        });

    // Health check endpoint
    let _health = warp::path("health").and(warp::get()).map(|| {
        warp::reply::json(&serde_json::json!({
            "status": "healthy",
            "version": VERSION,
            "timestamp": chrono::Utc::now()
        }))
    });

    // Order book endpoint
    let orderbooks = warp::path("orderbooks")
        .and(warp::get())
        .and(manager.clone())
        .map(|mgr: Arc<MarketDataManager>| {
            let symbols = mgr.get_orderbook_symbols();
            let mut books = std::collections::HashMap::new();

            for symbol in symbols {
                if let Some(book) = mgr.get_orderbook(&symbol) {
                    books.insert(
                        symbol.to_string(),
                        serde_json::json!({
                            "symbol": symbol,
                            "best_bid": book.best_bid(),
                            "best_ask": book.best_ask(),
                            "mid_price": book.mid_price(),
                            "spread": book.spread(),
                            "last_update": book.last_update()
                        }),
                    );
                }
            }

            warp::reply::json(&books)
        });

    // AMM pools endpoint
    let pools = warp::path("pools")
        .and(warp::get())
        .and(manager.clone())
        .map(|mgr: Arc<MarketDataManager>| {
            let addresses = mgr.get_amm_pool_addresses();
            let mut pool_data = std::collections::HashMap::new();

            for address in addresses {
                if let Some(pool) = mgr.get_amm_pool(&address) {
                    pool_data.insert(
                        address.to_string(),
                        serde_json::json!({
                            "address": address,
                            "pool_type": pool.pool_type(),
                            "implied_mid": pool.implied_mid(),
                            "reserves": pool.get_reserves(),
                            "fee_tier": pool.get_fee_tier(),
                            "last_update": pool.last_update()
                        }),
                    );
                }
            }

            warp::reply::json(&pool_data)
        });

    let routes = metrics
        .or(orderbooks)
        .or(pools)
        .with(warp::cors().allow_any_origin());

    info!("API routes configured successfully");
    info!("API server ready at http://localhost:3030");
    info!("Available endpoints:");
    info!("  GET /metrics - System metrics");
    info!("  GET /orderbooks - Order book data");
    info!("  GET /pools - AMM pool data");
    info!("Server starting on 127.0.0.1:3030...");

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
