use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use tokio::runtime::Runtime;
use zylos_memory_system::{ContextEdge, ContextGraph, ContextNode, MemoryScope};

fn bench_add_node(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let graph = Arc::new(rt.block_on(async { ContextGraph::in_memory().unwrap() }));

    let mut group = c.benchmark_group("add_node");
    group.throughput(Throughput::Elements(1));

    group.bench_function("single", |b| {
        b.to_async(&rt).iter(|| async {
            let node = ContextNode::new(
                serde_json::json!({"text": "benchmark node"}),
                MemoryScope::Agent,
                "bench-agent",
            );
            graph.add_node(node).await.unwrap()
        })
    });

    group.finish();
}

fn bench_get_node(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let graph = rt.block_on(async {
        let g = ContextGraph::in_memory().unwrap();
        // Pre-populate with 100 nodes
        for i in 0..100 {
            let node = ContextNode::new(
                serde_json::json!({"text": format!("node {}", i)}),
                MemoryScope::Agent,
                "bench-agent",
            );
            g.add_node(node).await.unwrap();
        }
        g
    });

    // Get first node ID for lookup
    let first_id = rt.block_on(async {
        let results = graph
            .query(
                zylos_memory_system::ContextQuery::new()
                    .with_scope(MemoryScope::Agent)
                    .with_limit(1),
            )
            .await
            .unwrap();
        results[0].id
    });

    let mut group = c.benchmark_group("get_node");
    group.throughput(Throughput::Elements(1));

    group.bench_function("lookup_by_id", |b| {
        b.to_async(&rt)
            .iter(|| async { graph.get_node(first_id).await.unwrap() })
    });

    group.finish();
}

fn bench_query_by_scope(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let graph = rt.block_on(async {
        let g = ContextGraph::in_memory().unwrap();
        // Pre-populate: 50 agent + 50 team nodes
        for i in 0..50 {
            let node = ContextNode::new(
                serde_json::json!({"text": format!("agent node {}", i)}),
                MemoryScope::Agent,
                "bench-agent",
            );
            g.add_node(node).await.unwrap();
        }
        for i in 0..50 {
            let node = ContextNode::new(
                serde_json::json!({"text": format!("team node {}", i)}),
                MemoryScope::Team,
                "bench-team",
            );
            g.add_node(node).await.unwrap();
        }
        g
    });

    let mut group = c.benchmark_group("query_by_scope");
    group.throughput(Throughput::Elements(50));

    group.bench_function("filter_agent_scope", |b| {
        b.to_async(&rt).iter(|| async {
            graph
                .query(zylos_memory_system::ContextQuery::new().with_scope(MemoryScope::Agent))
                .await
                .unwrap()
        })
    });

    group.finish();
}

fn bench_get_neighbors(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (graph, center_id) = rt.block_on(async {
        let g = ContextGraph::in_memory().unwrap();
        // Create a hub node with 20 neighbors
        let center = ContextNode::new(
            serde_json::json!({"text": "hub"}),
            MemoryScope::Global,
            "global",
        );
        let center_id = g.add_node(center).await.unwrap();

        for i in 0..20 {
            let neighbor = ContextNode::new(
                serde_json::json!({"text": format!("neighbor {}", i)}),
                MemoryScope::Global,
                "global",
            );
            let nid = g.add_node(neighbor).await.unwrap();
            let edge = ContextEdge::new(center_id, nid, "references");
            g.add_edge(edge).await.unwrap();
        }
        (g, center_id)
    });

    let mut group = c.benchmark_group("get_neighbors");
    group.throughput(Throughput::Elements(20));

    group.bench_function("hub_20_edges", |b| {
        b.to_async(&rt)
            .iter(|| async { graph.get_neighbors(center_id).await.unwrap() })
    });

    group.finish();
}

fn bench_cosine_similarity(c: &mut Criterion) {
    use zylos_memory_system::utils::cosine_similarity;

    let mut group = c.benchmark_group("cosine_similarity");

    for dim in [128, 384, 768, 1536] {
        let a: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.001).collect();
        let b: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.002 + 0.1).collect();

        group.throughput(Throughput::Elements(dim as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(dim),
            &(a, b),
            |bencher, (a, b)| bencher.iter(|| cosine_similarity(a, b)),
        );
    }

    group.finish();
}

fn bench_vector_store_search(c: &mut Criterion) {
    use zylos_memory_system::{InMemoryVectorStore, SearchParams, VectorDocument, VectorStore};

    let rt = Runtime::new().unwrap();
    let store = Arc::new(InMemoryVectorStore::new());

    // Pre-populate with 10k embeddings (target scale per EVALUATION.md)
    rt.block_on(async {
        let docs: Vec<VectorDocument> = (0..10_000)
            .map(|i| {
                let embedding: Vec<f32> =
                    (0..384).map(|d| ((i * 384 + d) as f32) * 0.0001).collect();
                VectorDocument::new(embedding).with_content(format!("document {}", i))
            })
            .collect();
        store.insert(docs).await.unwrap();
    });

    let query_embedding: Vec<f32> = (0..384).map(|i| (i as f32) * 0.00015).collect();

    let mut group = c.benchmark_group("vector_store_search");
    group.throughput(Throughput::Elements(10_000));

    group.bench_function("10k_docs_top10", |b| {
        b.to_async(&rt).iter(|| async {
            let params = SearchParams::new(query_embedding.clone(), 10);
            store.search(params).await.unwrap()
        })
    });

    group.finish();
}

fn bench_brief_aware_reranker(c: &mut Criterion) {
    use zylos_memory_system::{
        BriefAwareReranker, ContextNode, MemoryScope, RerankConfig, ResearchBrief,
    };

    let reranker = BriefAwareReranker::new(RerankConfig {
        max_results: 50,
        ..RerankConfig::default()
    });

    let brief = ResearchBrief::new("bench-brief", "optimize vector search performance")
        .with_embedding((0..384).map(|i| (i as f32) * 0.001).collect())
        .with_tags(vec!["performance".into(), "vector".into()]);

    // Generate 1k candidate nodes (target scale per EVALUATION.md)
    let candidates: Vec<ContextNode> = (0..1_000)
        .map(|i| {
            let emb: Vec<f32> = (0..384).map(|d| ((i * 384 + d) as f32) * 0.0005).collect();
            ContextNode::new(
                serde_json::json!({"text": format!("memory node {} about search optimization", i)}),
                MemoryScope::Agent,
                "bench-agent",
            )
            .with_embedding(emb)
            .with_tags(vec!["search".into()])
        })
        .collect();

    let mut group = c.benchmark_group("brief_aware_reranker");
    group.throughput(Throughput::Elements(1_000));

    group.bench_function("rerank_1k_candidates", |b| {
        b.iter(|| reranker.rerank(&brief, candidates.clone()).unwrap())
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_add_node,
    bench_get_node,
    bench_query_by_scope,
    bench_get_neighbors,
    bench_cosine_similarity,
    bench_vector_store_search,
    bench_brief_aware_reranker,
);
criterion_main!(benches);
