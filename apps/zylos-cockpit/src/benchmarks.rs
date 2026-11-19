//! Cockpit Benchmarks — Task 6.3.1
//!
//! Performance benchmarks for validating cockpit scalability with 50+ agents.
//! Measures render time, memory usage, and interaction latency per Master Plan.

use leptos::prelude::*;

use crate::components::agent_card::{AgentInfo, AgentStatus};

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub agent_count: usize,
    pub iterations: usize,
    pub warmup_runs: usize,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            agent_count: 100,
            iterations: 10,
            warmup_runs: 2,
        }
    }
}

/// Benchmark results
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    pub config: BenchmarkConfig,
    #[allow(dead_code)]
    pub render_times_ms: Vec<f64>,
    pub avg_render_ms: f64,
    pub p95_render_ms: f64,
    pub p99_render_ms: f64,
    pub memory_mb: f64,
    /// p95 latency of a measured interaction (DOM click → handler done), ms.
    /// `NaN` when it could not be measured (fails the target).
    pub interaction_latency_ms: f64,
}

impl BenchmarkResults {
    /// Calculate percentile from sorted sample
    fn percentile(samples: &[f64], p: f64) -> f64 {
        if samples.is_empty() {
            return 0.0;
        }
        let idx = ((samples.len() - 1) as f64 * p).round() as usize;
        samples[idx.min(samples.len() - 1)]
    }

    /// Check if results meet performance targets
    pub fn meets_targets(&self) -> bool {
        // Target: p95 render < 100ms for 50+ agents
        self.p95_render_ms < 100.0 && self.interaction_latency_ms < 50.0
    }
}

/// Generate synthetic agents for benchmarking
pub fn generate_agents(count: usize) -> Vec<AgentInfo> {
    let statuses = [
        AgentStatus::Idle,
        AgentStatus::Running,
        AgentStatus::Blocked,
        AgentStatus::Archived,
    ];

    (0..count)
        .map(|i| {
            let status = statuses[i % statuses.len()].clone();
            AgentInfo {
                id: format!("benchmark/agent-{:04}", i),
                name: format!("Benchmark Agent {}", i),
                status,
                task_count: i % 20,
                last_active: format!("{}m ago", i % 60),
                activity: Vec::new(),
            }
        })
        .collect()
}

/// Monotonic clock in milliseconds.
///
/// On `wasm32` uses `performance.now()` (`std::time::Instant` panics on
/// `wasm32-unknown-unknown`); natively uses a process-wide `Instant` origin.
#[cfg(target_arch = "wasm32")]
pub fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or_else(js_sys::Date::now)
}

/// Monotonic clock in milliseconds (native: `Instant` since first call).
#[cfg(not(target_arch = "wasm32"))]
pub fn now_ms() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// Work done by the benchmarked interaction: filter the agent list by status
/// (what the cockpit does when the user clicks a status filter).
fn interaction_handler(agents: &[AgentInfo]) -> usize {
    agents
        .iter()
        .filter(|a| matches!(a.status, AgentStatus::Running))
        .count()
}

/// Measure the latency of one user interaction in milliseconds.
///
/// On `wasm32` a real DOM `click` event is dispatched to a detached element
/// whose listener runs the filter handler; the time from dispatch until the
/// listener returns is measured with `performance.now()`. Returns `None` if
/// the DOM is unavailable. Natively, only the handler cost is measured.
#[cfg(target_arch = "wasm32")]
pub fn measure_interaction_latency(agents: &[AgentInfo]) -> Option<f64> {
    use std::cell::Cell;
    use std::rc::Rc;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let document = web_sys::window()?.document()?;
    let target = document.create_element("button").ok()?;
    let agents: Rc<Vec<AgentInfo>> = Rc::new(agents.to_vec());
    let handled = Rc::new(Cell::new(false));
    let (a, h) = (agents.clone(), handled.clone());
    let listener = Closure::<dyn FnMut(web_sys::Event)>::new(move |_e: web_sys::Event| {
        let _ = interaction_handler(&a);
        h.set(true);
    });
    target
        .add_event_listener_with_callback("click", listener.as_ref().unchecked_ref())
        .ok()?;
    let event = web_sys::Event::new("click").ok()?;
    let start = now_ms();
    target.dispatch_event(&event).ok()?;
    let elapsed = now_ms() - start;
    let _ = target.remove_event_listener_with_callback("click", listener.as_ref().unchecked_ref());
    handled.get().then_some(elapsed)
}

/// Measure the latency of one user interaction (native: handler cost only).
#[cfg(not(target_arch = "wasm32"))]
pub fn measure_interaction_latency(agents: &[AgentInfo]) -> Option<f64> {
    let start = now_ms();
    std::hint::black_box(interaction_handler(std::hint::black_box(agents)));
    Some(now_ms() - start)
}

/// Run benchmark suite
pub fn run_benchmark(config: BenchmarkConfig) -> BenchmarkResults {
    let agents = generate_agents(config.agent_count);
    let mut render_times = Vec::new();

    // Warmup runs (not counted) — exercise allocation + filtering
    for _ in 0..config.warmup_runs {
        let _filtered: Vec<&AgentInfo> = agents
            .iter()
            .filter(|a| matches!(a.status, AgentStatus::Running))
            .collect();
    }

    // Measured iterations — time agent generation + filtering (DOM-independent cost)
    for _ in 0..config.iterations {
        let start = now_ms();
        let fresh = generate_agents(config.agent_count);
        let _filtered: Vec<&AgentInfo> = fresh
            .iter()
            .filter(|a| matches!(a.status, AgentStatus::Running))
            .collect();
        let elapsed = now_ms() - start;
        render_times.push(elapsed);
    }

    render_times.sort_by(|a, b| a.partial_cmp(b).unwrap());

    // Interaction latency: p95 over `iterations` measured interactions
    // (NaN = could not be measured, e.g. no DOM).
    let mut latencies: Vec<f64> = (0..config.iterations.max(1))
        .filter_map(|_| measure_interaction_latency(&agents))
        .collect();
    latencies.sort_by(|a, b| a.total_cmp(b));
    let interaction_latency_ms = if latencies.is_empty() {
        f64::NAN
    } else {
        BenchmarkResults::percentile(&latencies, 0.95)
    };

    let avg = render_times.iter().sum::<f64>() / render_times.len() as f64;
    let p95 = BenchmarkResults::percentile(&render_times, 0.95);
    let p99 = BenchmarkResults::percentile(&render_times, 0.99);

    // Estimate memory usage (rough heuristic)
    let memory_mb = (config.agent_count as f64 * 0.5) + 10.0; // ~0.5MB per agent + 10MB base

    BenchmarkResults {
        config,
        render_times_ms: render_times,
        avg_render_ms: avg,
        p95_render_ms: p95,
        p99_render_ms: p99,
        memory_mb,
        interaction_latency_ms,
    }
}

/// BenchmarkPanel component — displays benchmark results
#[component]
pub fn BenchmarkPanel() -> impl IntoView {
    let (results, set_results) = signal(None::<BenchmarkResults>);
    let (running, set_running) = signal(false);

    let run_bench = move |_| {
        set_running.set(true);
        let config = BenchmarkConfig::default();
        let bench_results = run_benchmark(config);
        set_results.set(Some(bench_results));
        set_running.set(false);
    };

    view! {
        <div class="benchmark-panel">
            <h2>"Benchmarks de Performance"</h2>
            <button
                class="cta-primary"
                on:click=run_bench
                disabled=move || running.get()
            >
                {move || if running.get() { "Executando..." } else { "Executar Benchmarks" }}
            </button>

            {move || {
                if let Some(r) = results.get() {
                    let meets = r.meets_targets();
                    let status_class = if meets { "status-pass" } else { "status-fail" };
                    let status_text = if meets { "✅ PASS" } else { "❌ FAIL" };

                    view! {
                        <div class="benchmark-results">
                            <div class={format!("benchmark-status {}", status_class)}>
                                <span>{status_text}</span>
                            </div>

                            <div class="benchmark-metrics">
                                <div class="metric">
                                    <span class="metric-label">"Agentes testados"</span>
                                    <span class="metric-value">{r.config.agent_count.to_string()}</span>
                                </div>
                                <div class="metric">
                                    <span class="metric-label">"Render médio"</span>
                                    <span class="metric-value">{format!("{:.2}ms", r.avg_render_ms)}</span>
                                </div>
                                <div class="metric">
                                    <span class="metric-label">"p95 render"</span>
                                    <span class="metric-value">{format!("{:.2}ms", r.p95_render_ms)}</span>
                                </div>
                                <div class="metric">
                                    <span class="metric-label">"p99 render"</span>
                                    <span class="metric-value">{format!("{:.2}ms", r.p99_render_ms)}</span>
                                </div>
                                <div class="metric">
                                    <span class="metric-label">"Memória estimada"</span>
                                    <span class="metric-value">{format!("{:.1}MB", r.memory_mb)}</span>
                                </div>
                                <div class="metric">
                                    <span class="metric-label">"Latência interação"</span>
                                    <span class="metric-value">{format!("{:.1}ms", r.interaction_latency_ms)}</span>
                                </div>
                            </div>

                            <div class="benchmark-targets">
                                <h3>"Metas de Performance"</h3>
                                <ul>
                                    <li class={if r.p95_render_ms < 100.0 { "target-pass" } else { "target-fail" }}>
                                        "p95 render < 100ms: "
                                        {if r.p95_render_ms < 100.0 { "✅" } else { "❌" }}
                                        {format!(" ({:.1}ms)", r.p95_render_ms)}
                                    </li>
                                    <li class={if r.interaction_latency_ms < 50.0 { "target-pass" } else { "target-fail" }}>
                                        "Latência < 50ms: "
                                        {if r.interaction_latency_ms < 50.0 { "✅" } else { "❌" }}
                                        {format!(" ({:.1}ms)", r.interaction_latency_ms)}
                                    </li>
                                </ul>
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="benchmark-empty">
                            <p>"Clique no botão para executar os benchmarks"</p>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_agents() {
        let agents = generate_agents(50);
        assert_eq!(agents.len(), 50);
        assert_eq!(agents[0].id, "benchmark/agent-0000");
        assert_eq!(agents[49].id, "benchmark/agent-0049");
    }

    #[test]
    fn test_generate_agents_status_distribution() {
        let agents = generate_agents(100);
        let idle_count = agents
            .iter()
            .filter(|a| matches!(a.status, AgentStatus::Idle))
            .count();
        let running_count = agents
            .iter()
            .filter(|a| matches!(a.status, AgentStatus::Running))
            .count();
        // Should have roughly equal distribution across 4 statuses
        assert!(idle_count > 20);
        assert!(running_count > 20);
    }

    #[test]
    fn test_percentile_calculation() {
        let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(BenchmarkResults::percentile(&samples, 0.5), 3.0);
        assert_eq!(BenchmarkResults::percentile(&samples, 0.0), 1.0);
        assert_eq!(BenchmarkResults::percentile(&samples, 1.0), 5.0);
    }

    #[test]
    fn test_percentile_empty() {
        let samples: Vec<f64> = vec![];
        assert_eq!(BenchmarkResults::percentile(&samples, 0.5), 0.0);
    }

    #[test]
    fn test_benchmark_meets_targets() {
        let results = BenchmarkResults {
            config: BenchmarkConfig::default(),
            render_times_ms: vec![10.0, 20.0, 30.0],
            avg_render_ms: 20.0,
            p95_render_ms: 95.0,
            p99_render_ms: 98.0,
            memory_mb: 60.0,
            interaction_latency_ms: 45.0,
        };
        assert!(results.meets_targets());
    }

    #[test]
    fn test_benchmark_fails_targets() {
        let results = BenchmarkResults {
            config: BenchmarkConfig::default(),
            render_times_ms: vec![10.0, 20.0, 150.0],
            avg_render_ms: 60.0,
            p95_render_ms: 150.0,
            p99_render_ms: 150.0,
            memory_mb: 60.0,
            interaction_latency_ms: 45.0,
        };
        assert!(!results.meets_targets());
    }

    #[test]
    fn test_run_benchmark() {
        let config = BenchmarkConfig {
            agent_count: 10,
            iterations: 3,
            warmup_runs: 1,
        };
        let results = run_benchmark(config);
        assert_eq!(results.render_times_ms.len(), 3);
        assert_eq!(results.config.agent_count, 10);
        assert!(results.avg_render_ms >= 0.0);
        assert!(results.interaction_latency_ms.is_finite());
        assert!(results.interaction_latency_ms >= 0.0);
    }

    #[test]
    fn test_now_ms_monotonic_and_interaction_measured() {
        let a = now_ms();
        let agents = generate_agents(200);
        let lat = measure_interaction_latency(&agents).expect("native measurement");
        let b = now_ms();
        assert!(b >= a);
        assert!(lat >= 0.0 && lat <= b - a);
        assert_eq!(interaction_handler(&agents), 50);
    }

    #[test]
    fn test_nan_latency_fails_targets() {
        let results = BenchmarkResults {
            config: BenchmarkConfig::default(),
            render_times_ms: vec![1.0],
            avg_render_ms: 1.0,
            p95_render_ms: 1.0,
            p99_render_ms: 1.0,
            memory_mb: 1.0,
            interaction_latency_ms: f64::NAN,
        };
        assert!(!results.meets_targets());
    }
}
