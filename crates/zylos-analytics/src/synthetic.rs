//! Gerador determinístico de dados sintéticos (testes, demo e benchmark T11.10.8).
//!
//! PRNG xorshift64* com seed fixa — mesma entrada, mesmos eventos. Gera
//! padrão diurno (mais tráfego 09–18h UTC em dias úteis), distribuição de
//! latência por skill e **uma skill deliberadamente degradada**
//! (`pdf-extract`, ~55% de sucesso) para exercitar o feedback loop.

use crate::event::AnalyticsEvent;
use crate::DAY_SECS;

/// Parâmetros do gerador.
#[derive(Debug, Clone)]
pub struct SyntheticConfig {
    /// Dias de histórico (terminando em `now`).
    pub days: i64,
    /// Eventos por dia (média).
    pub events_per_day: usize,
    /// Seed do PRNG.
    pub seed: u64,
}

impl Default for SyntheticConfig {
    fn default() -> Self {
        Self {
            days: 30,
            events_per_day: 1_000,
            seed: 0x5EED_2026_1110,
        }
    }
}

/// (skill, taxa de sucesso, latência base ms, custo médio USD)
pub const SKILLS: [(&str, f64, u64, f64); 10] = [
    ("web-search", 0.96, 800, 0.002),
    ("code-review", 0.91, 4_000, 0.030),
    ("email-triage", 0.94, 1_200, 0.004),
    ("pdf-extract", 0.55, 2_500, 0.010),
    ("summarize", 0.97, 1_500, 0.008),
    ("deploy-check", 0.88, 3_000, 0.012),
    ("calendar-sync", 0.99, 300, 0.000),
    ("translate", 0.95, 900, 0.005),
    ("sql-query", 0.83, 600, 0.003),
    ("browser-task", 0.78, 7_500, 0.020),
];

/// Agentes sintéticos.
pub const AGENTS: [&str; 6] = ["atlas", "hermes", "iris", "nexus", "orion", "vega"];

const CHANNELS: [&str; 4] = ["slack", "cockpit", "email", "api"];
const MODELS: [&str; 3] = ["haiku", "sonnet", "opus"];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn pick(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Peso relativo de tráfego por hora UTC e dia da semana (0 = segunda).
fn traffic_weight(weekday: i64, hour: i64) -> f64 {
    let business = (9..18).contains(&hour);
    let weekend = weekday >= 5;
    match (weekend, business) {
        (false, true) => 3.0,
        (false, false) => 0.8,
        (true, true) => 1.0,
        (true, false) => 0.4,
    }
}

/// Gera eventos para `[now - days, now)` em ordem cronológica.
pub fn generate(cfg: &SyntheticConfig, now: i64) -> Vec<AnalyticsEvent> {
    let mut rng = Rng(cfg.seed.max(1));
    let start = now - cfg.days * DAY_SECS;
    let total = cfg.days.max(0) as usize * cfg.events_per_day;
    let mut out = Vec::with_capacity(total);
    // distribui por hora conforme o peso
    let hours = (cfg.days * 24) as usize;
    let weights: Vec<f64> = (0..hours)
        .map(|h| {
            let t = start + h as i64 * 3600;
            let wd = ((t / DAY_SECS) + 3).rem_euclid(7);
            let hr = (t.rem_euclid(DAY_SECS)) / 3600;
            traffic_weight(wd, hr)
        })
        .collect();
    let wsum: f64 = weights.iter().sum();
    for (h, w) in weights.iter().enumerate() {
        let n = ((w / wsum) * total as f64).round() as usize;
        let hour_start = start + h as i64 * 3600;
        for _ in 0..n {
            let (skill, rate, base_lat, cost) = SKILLS[rng.pick(SKILLS.len())];
            let agent = AGENTS[rng.pick(AGENTS.len())];
            let success = rng.f() < rate;
            // latência: base × (0.5..2.0), falhas tendem a ser mais lentas
            let mut lat = base_lat as f64 * (0.5 + 1.5 * rng.f());
            if !success {
                lat *= 1.4;
            }
            let tokens = (cost * 200_000.0 * (0.5 + rng.f())) as u64;
            let ev = AnalyticsEvent::new(
                hour_start + rng.pick(3600) as i64,
                agent,
                skill,
                success,
                lat as u64,
            )
            .with_cost(cost * (0.5 + rng.f()), tokens)
            .with_dim("channel", CHANNELS[rng.pick(CHANNELS.len())])
            .with_dim("model", MODELS[rng.pick(MODELS.len())]);
            out.push(ev);
        }
    }
    out.sort_by_key(|e| e.ts);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_sized() {
        let cfg = SyntheticConfig {
            days: 3,
            events_per_day: 500,
            ..SyntheticConfig::default()
        };
        let a = generate(&cfg, 1_794_873_600);
        let b = generate(&cfg, 1_794_873_600);
        assert_eq!(a, b);
        let n = a.len() as i64;
        assert!((n - 1500).abs() <= 72, "n = {n}");
        assert!(a
            .iter()
            .all(|e| e.ts >= 1_794_873_600 - 3 * DAY_SECS && e.ts < 1_794_873_600));
    }
}
