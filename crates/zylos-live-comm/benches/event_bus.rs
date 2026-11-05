//! Benchmarks for zylos-live-comm EventBus publish throughput.
//!
//! Target: >500k ops/s for heartbeat-sized events on a single subscriber.

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use tokio::runtime::Runtime;
use zylos_live_comm::event_bus::EventBus;
use zylos_live_comm::event_types::{CustomEvent, ZylosEvent};

fn bench_publish(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let bus = EventBus::new(1024, None);

    // Register one subscriber so publish doesn't take the zero-subscriber fast path
    let _sub = rt.block_on(bus.subscribe("bench-sub", "heartbeat.#", None)).unwrap();

    let event = ZylosEvent::Custom(CustomEvent {
        name: "heartbeat".into(),
        payload: serde_json::json!({"ts": 0}),
    });

    let mut group = c.benchmark_group("publish_heartbeat");
    group.throughput(Throughput::Elements(1));

    group.bench_function("single_subscriber", |b| {
        b.iter(|| {
            rt.block_on(bus.publish("heartbeat.ping", event.clone(), 1)).unwrap();
        })
    });

    group.finish();
}

criterion_group!(benches, bench_publish);
criterion_main!(benches);
