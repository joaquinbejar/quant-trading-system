use criterion::{criterion_group, criterion_main, Criterion};
use quant_trading_system::*;
use rust_decimal_macros::dec;
use std::hint::black_box;

fn benchmark_orderbook_operations(c: &mut Criterion) {
    let book = ThreadSafeOrderBook::new(Symbol("ETHUSDC".to_string()));

    // Setup initial state
    let mut snapshot = OrderBookSnapshot::new(SequenceNumber(1), chrono::Utc::now());
    for i in 0..1000 {
        let price = dec!(2000.0) + rust_decimal::Decimal::from(i);
        snapshot.bids.insert(Price(price), Quantity(dec!(1.0)));
        snapshot
            .asks
            .insert(Price(price + dec!(1.0)), Quantity(dec!(1.0)));
    }
    book.apply_snapshot(snapshot).unwrap();

    c.bench_function("orderbook_best_bid", |b| {
        b.iter(|| black_box(book.best_bid()))
    });

    c.bench_function("orderbook_best_ask", |b| {
        b.iter(|| black_box(book.best_ask()))
    });

    c.bench_function("orderbook_mid_price", |b| {
        b.iter(|| black_box(book.mid_price()))
    });
}

criterion_group!(benches, benchmark_orderbook_operations);
criterion_main!(benches);
