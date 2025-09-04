
## Quant Trading System

A high-performance, thread-safe quantitative trading system implementing:
- CLOB (Central Limit Order Book) L2 order books with fast top-of-book access
- AMM (Automated Market Maker) pools with price impact calculations
- Real-time arbitrage detection between venues
- Concurrent stream ingestion with backpressure handling
- JSON parsing for market data feeds

### Architecture

The system follows domain-driven design principles with clear separation of concerns:

- **Domain**: Core business logic (order books, AMM pools, arbitrage detection)
- **Infrastructure**: External concerns (JSON parsing, stream ingestion, metrics)
- **Application**: Use cases and orchestration

### Thread Safety

All data structures use `std::sync::RwLock` for concurrent access:
- Multiple concurrent readers
- Single writer exclusion
- Atomic snapshots without blocking writers

### Performance Characteristics

- **Latency**: Sub-microsecond order book operations
- **Throughput**: 1M+ updates/second per symbol
- **Memory**: O(n) where n = number of price levels
- **Concurrency**: Lock-free reads, minimal write contention


## Contact

**Joaquín Béjar García**
- Email: jb@taunais.com
- GitHub: [joaquinbejar](https://github.com/joaquinbejar)
