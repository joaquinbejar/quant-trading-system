use crate::domain::{events::*, market_data::*};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

/// Stream ingester that processes market events concurrently
pub struct StreamIngester {
    /// Channel receiver for incoming market events
    receiver: mpsc::Receiver<MarketEvent>,
    /// Market data manager to process events
    manager: Arc<MarketDataManager>,
    /// Buffer size for backpressure handling
    buffer_size: usize,
}

impl StreamIngester {
    /// Create new stream ingester
    pub fn new(receiver: mpsc::Receiver<MarketEvent>, manager: Arc<MarketDataManager>) -> Self {
        Self {
            receiver,
            manager,
            buffer_size: 1000, // Default buffer size
        }
    }

    /// Create stream ingester with custom buffer size
    pub fn with_buffer_size(
        receiver: mpsc::Receiver<MarketEvent>,
        manager: Arc<MarketDataManager>,
        buffer_size: usize,
    ) -> Self {
        Self {
            receiver,
            manager,
            buffer_size,
        }
    }

    /// Run the ingestion loop
    pub async fn run(&mut self) {
        info!(
            "Starting stream ingester with buffer size: {}",
            self.buffer_size
        );

        let mut event_count = 0u64;
        let mut error_count = 0u64;

        while let Some(event) = self.receiver.recv().await {
            event_count += 1;

            // Process the event
            if let Err(e) = self.manager.process_event(event.clone()) {
                error_count += 1;
                error!("Failed to process event: {:?}, error: {}", event, e);

                // Log error details based on event type
                match event {
                    MarketEvent::OrderBookSnapshot(symbol, _) => {
                        error!("Failed to process snapshot for symbol: {}", symbol);
                    }
                    MarketEvent::OrderBookDelta(symbol, delta) => {
                        error!(
                            "Failed to process delta for symbol: {}, seq: {}",
                            symbol, delta.sequence
                        );
                    }
                    MarketEvent::Trade(symbol, trade) => {
                        error!(
                            "Failed to process trade for symbol: {}, seq: {}",
                            symbol, trade.sequence
                        );
                    }
                    MarketEvent::AMMUpdate(address, _) => {
                        error!("Failed to process AMM update for pool: {}", address);
                    }
                }
            }

            // Log progress periodically
            if event_count % 1000 == 0 {
                info!(
                    "Processed {} events, {} errors ({}% error rate)",
                    event_count,
                    error_count,
                    (error_count as f64 / event_count as f64) * 100.0
                );
            }
        }

        info!(
            "Stream ingester finished. Total events: {}, errors: {}",
            event_count, error_count
        );
    }

    /// Get current buffer size
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }
}

/// Multi-stream ingester that can handle multiple concurrent streams
pub struct MultiStreamIngester {
    /// Multiple receivers for different data streams
    receivers: Vec<mpsc::Receiver<MarketEvent>>,
    /// Market data manager to process events
    manager: Arc<MarketDataManager>,
    /// Buffer size per stream
    buffer_size: usize,
}

impl MultiStreamIngester {
    /// Create new multi-stream ingester
    pub fn new(
        receivers: Vec<mpsc::Receiver<MarketEvent>>,
        manager: Arc<MarketDataManager>,
    ) -> Self {
        Self {
            receivers,
            manager,
            buffer_size: 1000,
        }
    }

    /// Run all streams concurrently
    pub async fn run(self) {
        info!(
            "Starting multi-stream ingester with {} streams",
            self.receivers.len()
        );

        let mut handles = Vec::new();

        // Spawn a task for each stream
        for (stream_id, receiver) in self.receivers.into_iter().enumerate() {
            let manager = Arc::clone(&self.manager);
            let buffer_size = self.buffer_size;

            let handle = tokio::spawn(async move {
                let mut ingester = StreamIngester::with_buffer_size(receiver, manager, buffer_size);
                info!("Starting stream {} ingestion", stream_id);
                ingester.run().await;
                info!("Stream {} ingestion completed", stream_id);
            });

            handles.push(handle);
        }

        // Wait for all streams to complete
        for (stream_id, handle) in handles.into_iter().enumerate() {
            if let Err(e) = handle.await {
                error!("Stream {} failed: {}", stream_id, e);
            }
        }

        info!("All streams completed");
    }
}

/// Event statistics for stream ingestion performance
#[derive(Debug, Clone, Default)]
pub struct IngestionStats {
    /// Total number of events processed
    pub total_events: u64,
    /// Number of successfully processed events
    pub successful_events: u64,
    /// Number of events that failed processing
    pub failed_events: u64,
    /// Current processing rate in events per second
    pub events_per_second: f64,
    /// Timestamp of the last processed event
    pub last_event_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    /// Timestamp when ingestion started
    pub start_time: chrono::DateTime<chrono::Utc>,
}

impl IngestionStats {
    /// Creates new ingestion statistics
    pub fn new() -> Self {
        Self {
            start_time: chrono::Utc::now(),
            ..Default::default()
        }
    }

    /// Records an event and updates statistics
    pub fn record_event(&mut self, success: bool) {
        self.total_events += 1;
        self.last_event_timestamp = Some(chrono::Utc::now());

        if success {
            self.successful_events += 1;
        } else {
            self.failed_events += 1;
        }
    }

    /// Calculates the success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_events > 0 {
            (self.successful_events as f64 / self.total_events as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Calculates the error rate as a percentage
    pub fn error_rate(&self) -> f64 {
        if self.total_events > 0 {
            (self.failed_events as f64 / self.total_events as f64) * 100.0
        } else {
            0.0
        }
    }
}

/// Stream ingester with detailed statistics tracking
pub struct StatisticalStreamIngester {
    receiver: mpsc::Receiver<MarketEvent>,
    manager: Arc<MarketDataManager>,
    stats: IngestionStats,
    report_interval: u64,
}

impl StatisticalStreamIngester {
    /// Creates a new stream ingester with the given receiver and market data manager
    pub fn new(receiver: mpsc::Receiver<MarketEvent>, manager: Arc<MarketDataManager>) -> Self {
        Self {
            receiver,
            manager,
            stats: IngestionStats::new(),
            report_interval: 1000, // Report every 1000 events
        }
    }

    /// Runs the statistical stream ingester, processing events and reporting statistics
    pub async fn run(&mut self) {
        info!("Starting statistical stream ingester");

        while let Some(event) = self.receiver.recv().await {
            // Process the event
            let success = self.manager.process_event(event.clone()).is_ok();
            self.stats.record_event(success);

            if !success {
                error!("Failed to process event: {:?}", event);
            }

            // Report statistics periodically
            if self.stats.total_events % self.report_interval == 0 {
                self.report_stats();
            }
        }

        // Final report
        self.report_stats();
        info!("Statistical stream ingester finished");
    }

    fn report_stats(&self) {
        info!(
            "Stats - Total: {}, Successful: {}, Failed: {}, Success Rate: {:.2}%, Error Rate: {:.2}%",
            self.stats.total_events,
            self.stats.successful_events,
            self.stats.failed_events,
            self.stats.success_rate(),
            self.stats.error_rate()
        );
    }

    /// Returns the current ingestion statistics
    pub fn get_stats(&self) -> &IngestionStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::*;
    use rust_decimal_macros::dec;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_stream_ingester_basic() {
        let manager = Arc::new(MarketDataManager::new());
        let (tx, rx) = mpsc::channel(10);
        let mut ingester = StreamIngester::new(rx, manager.clone());

        // Send a test event
        let symbol = Symbol("ETHUSDC".to_string());
        let snapshot = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());
        let event = MarketEvent::OrderBookSnapshot(symbol.clone(), snapshot);

        tx.send(event).await.unwrap();
        drop(tx); // Close the channel

        // Run ingester
        ingester.run().await;

        // Verify event was processed
        assert!(manager.get_orderbook(&symbol).is_some());
    }

    #[tokio::test]
    async fn test_multi_stream_ingester() {
        let manager = Arc::new(MarketDataManager::new());

        // Create multiple streams
        let (tx1, rx1) = mpsc::channel(10);
        let (tx2, rx2) = mpsc::channel(10);

        let multi_ingester = MultiStreamIngester::new(vec![rx1, rx2], manager.clone());

        // Send events to both streams
        let symbol1 = Symbol("ETHUSDC".to_string());
        let symbol2 = Symbol("BTCUSDC".to_string());

        let snapshot1 = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());
        let snapshot2 = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());

        tx1.send(MarketEvent::OrderBookSnapshot(symbol1.clone(), snapshot1))
            .await
            .unwrap();
        tx2.send(MarketEvent::OrderBookSnapshot(symbol2.clone(), snapshot2))
            .await
            .unwrap();

        drop(tx1);
        drop(tx2);

        // Run multi-stream ingester
        multi_ingester.run().await;

        // Verify both events were processed
        assert!(manager.get_orderbook(&symbol1).is_some());
        assert!(manager.get_orderbook(&symbol2).is_some());
    }

    #[tokio::test]
    async fn test_ingestion_stats() {
        let manager = Arc::new(MarketDataManager::new());
        let (tx, rx) = mpsc::channel(10);
        let mut ingester = StatisticalStreamIngester::new(rx, manager);

        // Send multiple events
        let symbol = Symbol("ETHUSDC".to_string());

        // Snapshot
        let snapshot = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());
        tx.send(MarketEvent::OrderBookSnapshot(symbol.clone(), snapshot))
            .await
            .unwrap();

        // Delta
        let delta = OrderBookDelta::new(SequenceNumber(2), chrono::Utc::now());
        tx.send(MarketEvent::OrderBookDelta(symbol.clone(), delta))
            .await
            .unwrap();

        // Trade
        let trade = Trade::new(
            SequenceNumber(3),
            chrono::Utc::now(),
            Price(dec!(2445.0)),
            Quantity(dec!(1.0)),
            Side::Bid,
        );
        tx.send(MarketEvent::Trade(symbol, trade)).await.unwrap();

        drop(tx);

        // Run ingester
        ingester.run().await;

        // Check stats
        let stats = ingester.get_stats();
        assert_eq!(stats.total_events, 3);
        assert_eq!(stats.successful_events, 3);
        assert_eq!(stats.failed_events, 0);
    }

    #[test]
    fn test_ingestion_stats_calculations() {
        let mut stats = IngestionStats::new();

        // Simulate some events

        for _ in 0..8 {
            stats.record_event(true);
        }

        // Record some failures
        stats.record_event(false);
        stats.record_event(false);

        assert_eq!(stats.total_events, 10);
        assert_eq!(stats.failed_events, 2);
        assert_eq!(stats.error_rate(), 20.0); // 2/10 * 100 = 20%
    }
}
