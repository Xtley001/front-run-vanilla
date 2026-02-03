use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use front_run_vanilla::OrderBook;
use front_run_vanilla::Side;
use rust_decimal_macros::dec;

/// Benchmark order book update performance
/// TARGET: <1ms for update operations
fn bench_update_level(c: &mut Criterion) {
    let ob = OrderBook::new("BTCUSDT");
    
    c.bench_function("update_single_level", |b| {
        b.iter(|| {
            ob.update_level(
                black_box(Side::Buy),
                black_box(dec!(100.0)),
                black_box(dec!(1.5)),
            ).unwrap();
        });
    });
}

/// Benchmark top of book access
/// This is called frequently for signal calculation
fn bench_top_of_book(c: &mut Criterion) {
    let ob = OrderBook::new("BTCUSDT");
    
    // Populate with realistic depth
    for i in 0..20 {
        let price = dec!(100.0) - rust_decimal::Decimal::from(i);
        ob.update_level(Side::Buy, price, dec!(1.0)).unwrap();
    }
    for i in 0..20 {
        let price = dec!(101.0) + rust_decimal::Decimal::from(i);
        ob.update_level(Side::Sell, price, dec!(1.0)).unwrap();
    }
    
    c.bench_function("get_top_of_book", |b| {
        b.iter(|| {
            black_box(ob.get_top_of_book());
        });
    });
}

/// Benchmark imbalance calculation
/// CRITICAL: This is called for every signal generation
/// TARGET: <2ms
fn bench_imbalance_calculation(c: &mut Criterion) {
    let ob = OrderBook::new("BTCUSDT");
    
    // Populate with realistic depth
    for i in 0..20 {
        let price = dec!(100.0) - rust_decimal::Decimal::from(i);
        ob.update_level(Side::Buy, price, dec!(1.0 + rust_decimal::Decimal::from(i) * dec!(0.1))).unwrap();
    }
    for i in 0..20 {
        let price = dec!(101.0) + rust_decimal::Decimal::from(i);
        ob.update_level(Side::Sell, price, dec!(1.0 + rust_decimal::Decimal::from(i) * dec!(0.1))).unwrap();
    }
    
    let mut group = c.benchmark_group("imbalance_calculation");
    
    for levels in [5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(levels),
            levels,
            |b, &levels| {
                b.iter(|| {
                    black_box(ob.calculate_imbalance(black_box(levels)));
                });
            },
        );
    }
    group.finish();
}

/// Benchmark depth retrieval
fn bench_get_depth(c: &mut Criterion) {
    let ob = OrderBook::new("BTCUSDT");
    
    // Populate with 50 levels each side
    for i in 0..50 {
        let bid_price = dec!(100.0) - rust_decimal::Decimal::from(i);
        let ask_price = dec!(101.0) + rust_decimal::Decimal::from(i);
        ob.update_level(Side::Buy, bid_price, dec!(1.0)).unwrap();
        ob.update_level(Side::Sell, ask_price, dec!(1.0)).unwrap();
    }
    
    let mut group = c.benchmark_group("get_depth");
    
    for levels in [5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(levels),
            levels,
            |b, &levels| {
                b.iter(|| {
                    black_box(ob.get_depth(black_box(levels)));
                });
            },
        );
    }
    group.finish();
}

/// Benchmark concurrent updates (simulating real WebSocket load)
fn bench_concurrent_updates(c: &mut Criterion) {
    use std::sync::Arc;
    
    c.bench_function("concurrent_100_updates", |b| {
        b.iter(|| {
            let ob = Arc::new(OrderBook::new("BTCUSDT"));
            let mut handles = vec![];
            
            // Simulate 100 concurrent updates
            for i in 0..100 {
                let ob_clone = Arc::clone(&ob);
                let handle = std::thread::spawn(move || {
                    let price = dec!(100.0) + rust_decimal::Decimal::from(i % 20);
                    let side = if i % 2 == 0 { Side::Buy } else { Side::Sell };
                    ob_clone.update_level(side, price, dec!(1.0)).unwrap();
                });
                handles.push(handle);
            }
            
            for handle in handles {
                handle.join().unwrap();
            }
        });
    });
}

criterion_group!(
    benches,
    bench_update_level,
    bench_top_of_book,
    bench_imbalance_calculation,
    bench_get_depth,
    bench_concurrent_updates
);
criterion_main!(benches);
