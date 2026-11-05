# Benchmark Template — Zylos-Next

## Metadata
- **Crate:** <crate-name>
- **Component:** <component>
- **Date:** YYYY-MM-DD
- **Runner:** <machine-specs>

## Configuração

```rust
// criterion benchmark setup
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_<name>(c: &mut Criterion) {
    // setup
    c.bench_function("<name>", |b| {
        b.iter(|| {
            // measured code
        })
    });
}

criterion_group!(benches, bench_<name>);
criterion_main!(benches);
```

## Resultados Esperados

| Métrica | Target | Atual | Status |
|---------|--------|-------|--------|
| Latência p50 | <Xms> | - | ⏳ |
| Latência p99 | <Yms> | - | ⏳ |
| Throughput | >Z ops/s | - | ⏳ |
| RAM idle | <30MB | - | ⏳ |

## Reprodução

```bash
cargo bench --package <crate> --bench <name>
```
