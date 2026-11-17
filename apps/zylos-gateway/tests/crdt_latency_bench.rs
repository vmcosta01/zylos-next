//! Benchmark de latência de sincronização CRDT (Fase 11.9 — T11.9.8).
//!
//! Mede, em loopback local:
//! 1. **RTT** cliente→servidor→cliente (ping `Custom(PING_TAG)`).
//! 2. **Propagação de edição**: tempo entre o envio de um update por um
//!    cliente e a aplicação desse update em *todos* os outros N-1 clientes.
//!
//! Executar (release recomendado):
//! `cargo test -p zylos-gateway --release --test crdt_latency_bench -- --nocapture`
//!
//! Critério (POT `runbooks/pot-crdt-latency-benchmark.md`): p95 < 100 ms local.

mod crdt_common;

use crdt_common::{wait_until, Client, TestServer};
use std::time::{Duration, Instant};

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn summarize(name: &str, mut samples: Vec<f64>) -> f64 {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let p50 = percentile(&samples, 50.0);
    let p95 = percentile(&samples, 95.0);
    let p99 = percentile(&samples, 99.0);
    let max = *samples.last().unwrap();
    println!(
        "BENCH {name}: n={} mean={mean:.3}ms p50={p50:.3}ms p95={p95:.3}ms p99={p99:.3}ms max={max:.3}ms",
        samples.len()
    );
    p95
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_bench_ping_rtt() {
    let server = TestServer::start(None).await;
    let c = Client::connect(&server.url("bench-ping"), 1).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    for i in 0..20 {
        c.ping(i).await; // aquecimento
    }
    let mut samples = Vec::new();
    for i in 0..500u32 {
        samples.push(c.ping(1000 + i).await.as_secs_f64() * 1000.0);
    }
    let p95 = summarize("ping_rtt", samples);
    assert!(p95 < 100.0, "p95 RTT {p95:.3}ms >= 100ms");
    server.stop();
}

async fn propagation(n_clients: u64, edits: usize) -> f64 {
    let server = TestServer::start(None).await;
    let doc = format!("bench-prop-{n_clients}");
    let mut clients = Vec::new();
    for i in 0..n_clients {
        clients.push(Client::connect(&server.url(&doc), 100 + i).await);
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut samples = Vec::new();
    for e in 0..edits {
        let writer = &clients[e % clients.len()];
        let expected = e + 1;
        let t0 = Instant::now();
        writer.push("x");
        let ok = wait_spin(Duration::from_secs(5), || {
            clients.iter().all(|c| c.text().len() == expected)
        })
        .await;
        assert!(ok, "edição {e} não propagou");
        samples.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    server.stop();
    summarize(&format!("edit_propagation_{n_clients}_clients"), samples)
}

/// Espera ativa com `yield_now` (resolução sub-ms, ao contrário de `sleep`).
async fn wait_spin(timeout: Duration, mut pred: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if pred() {
            return true;
        }
        tokio::task::yield_now().await;
    }
    pred()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_bench_edit_propagation_3_clients() {
    let p95 = propagation(3, 300).await;
    assert!(p95 < 100.0, "p95 {p95:.3}ms >= 100ms");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_bench_edit_propagation_10_clients() {
    let p95 = propagation(10, 300).await;
    assert!(p95 < 100.0, "p95 {p95:.3}ms >= 100ms");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_bench_burst_throughput_10_clients() {
    let server = TestServer::start(None).await;
    let mut clients = Vec::new();
    for i in 0..10u64 {
        clients.push(Client::connect(&server.url("bench-burst"), 200 + i).await);
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    let per_client = 100usize;
    let t0 = Instant::now();
    for c in &clients {
        for _ in 0..per_client {
            c.push("b");
        }
    }
    let total = clients.len() * per_client;
    assert!(
        wait_until(Duration::from_secs(30), || clients
            .iter()
            .all(|c| c.text().len() == total))
        .await,
        "burst não convergiu"
    );
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!(
        "BENCH burst_10_clients: edits={total} converged_in={ms:.3}ms throughput={:.0} edits/s",
        total as f64 / (ms / 1000.0)
    );
    server.stop();
}
