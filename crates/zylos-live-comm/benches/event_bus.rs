//! Benchmarks for zylos-live-comm EventBus publish throughput.
//!
//! Target: >500k ops/s for heartbeat-sized events on a single subscriber.

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use tokio::runtime::Runtime;
use zylos_live_comm::event_bus::EventBus;
use zylos_live_comm::event_types::{AgentStateChanged, ZylosEvent};

fn bench_publish(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let bus = EventBus::new(1024, None);

    // Register one subscriber so publish doesn't take the zero-subscriber fast path
    let _sub = rt
        .block_on(bus.subscribe("bench-sub", "agent.#", None))
        .unwrap();

    let event = ZylosEvent::AgentStateChanged(AgentStateChanged {
        agent_id: "bench-agent".into(),
        previous_state: "idle".into(),
        new_state: "working".into(),
        reason: Some("benchmark".into()),
    });

    let mut group = c.benchmark_group("publish_agent_state");
    group.throughput(Throughput::Elements(1));

    group.bench_function("single_subscriber", |b| {
        b.iter(|| {
            rt.block_on(bus.publish("agent.state_changed", event.clone(), 1))
                .unwrap();
        })
    });

    group.finish();
}

criterion_group!(benches, bench_publish);
criterion_main!(benches);
