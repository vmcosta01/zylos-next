//! Browser Anti-Detect: fingerprint generation, stealth JS patches, bot risk scoring.
//!
//! Provides deterministic browser profile generation and anti-detection configuration
//! for headless browser automation. No external browser dependencies — this module
//! generates configuration and JavaScript snippets only.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Headless browser mode selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeadlessMode {
    /// Traditional `--headless` flag (fully hidden).
    Full,
    /// Chrome's `--headless=new` (closer to real browser behaviour).
    New,
    /// No headless flag — visible browser window.
    Disabled,
}

impl fmt::Display for HeadlessMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeadlessMode::Full => write!(f, "full"),
            HeadlessMode::New => write!(f, "new"),
            HeadlessMode::Disabled => write!(f, "disabled"),
        }
    }
}

/// A deterministic browser fingerprint profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserProfile {
    pub user_agent: String,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub timezone: String,
    pub locale: String,
    pub platform: String,
    pub fonts: Vec<String>,
    pub canvas_hash: String,
    pub webgl_vendor: String,
    pub audio_context_hash: String,
    pub client_rects_variation: f64,
    pub navigator_hardware_concurrency: u32,
    pub navigator_device_memory: u32,
}

/// Anti-detection runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiDetectConfig {
    /// Rotate fingerprint on each identity change.
    pub rotate_fingerprint: bool,
    /// Pool of proxy URLs (e.g. `http://user:pass@host:port`).
    pub proxy_pool: Vec<String>,
    /// Headless mode preference.
    pub headless_mode: HeadlessMode,
    /// Inject stealth JavaScript patches before page load.
    pub stealth_scripts: bool,
    /// Current index into `proxy_pool` for round-robin rotation.
    #[serde(default)]
    pub proxy_index: usize,
}

/// Top-level sandbox combining a profile with its anti-detect config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSandbox {
    pub profile: BrowserProfile,
    pub config: AntiDetectConfig,
}

// ---------------------------------------------------------------------------
// Deterministic PRNG helper (xorshift64)
// ---------------------------------------------------------------------------

fn xorshift64(mut state: u64) -> u64 {
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    if state == 0 {
        1
    } else {
        state
    }
}

fn hash_seed(seed: u64, salt: &[u8]) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(salt);
    let result = hasher.finalize();
    u64::from_le_bytes(result[..8].try_into().unwrap())
}

// ---------------------------------------------------------------------------
// Profile generation
// ---------------------------------------------------------------------------

const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:126.0) Gecko/20100101 Firefox/126.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
];

const TIMEZONES: &[&str] = &[
    "America/New_York",
    "America/Chicago",
    "America/Denver",
    "America/Los_Angeles",
    "Europe/London",
    "Europe/Berlin",
    "Europe/Paris",
    "Asia/Tokyo",
    "Asia/Shanghai",
    "Australia/Sydney",
];

const LOCALES: &[&str] = &[
    "en-US", "en-GB", "de-DE", "fr-FR", "ja-JP", "zh-CN", "pt-BR", "es-ES",
];

const PLATFORMS: &[&str] = &["Win32", "MacIntel", "Linux x86_64"];

const WEBGL_VENDORS: &[&str] = &[
    "Google Inc. (NVIDIA)",
    "Google Inc. (AMD)",
    "Google Inc. (Intel)",
    "Apple",
    "Mesa/X.org",
];

const FONT_SETS: &[&[&str]] = &[
    &[
        "Arial",
        "Helvetica",
        "Times New Roman",
        "Georgia",
        "Verdana",
    ],
    &["Arial", "Calibri", "Segoe UI", "Tahoma", "Verdana"],
    &["Helvetica Neue", "Arial", "Courier New", "Georgia", "Times"],
    &["Roboto", "Arial", "Noto Sans", "Droid Sans", "sans-serif"],
];

impl BrowserSandbox {
    /// Generate a deterministic browser profile from a seed.
    ///
    /// The same seed always produces the same profile, enabling reproducible
    /// test scenarios and consistent identity across sessions.
    pub fn generate_profile(seed: u64) -> BrowserProfile {
        let s1 = xorshift64(hash_seed(seed, b"user_agent"));
        let s2 = xorshift64(hash_seed(seed, b"viewport"));
        let s3 = xorshift64(hash_seed(seed, b"timezone"));
        let s4 = xorshift64(hash_seed(seed, b"locale"));
        let s5 = xorshift64(hash_seed(seed, b"platform"));
        let s6 = xorshift64(hash_seed(seed, b"fonts"));
        let _s7 = xorshift64(hash_seed(seed, b"canvas"));
        let s8 = xorshift64(hash_seed(seed, b"webgl"));
        let _s9 = xorshift64(hash_seed(seed, b"audio"));
        let s10 = xorshift64(hash_seed(seed, b"rects"));
        let s11 = xorshift64(hash_seed(seed, b"cores"));
        let s12 = xorshift64(hash_seed(seed, b"memory"));

        // Canvas hash: deterministic hex string from seed
        let mut canvas_hasher = Sha256::new();
        canvas_hasher.update(b"canvas:");
        canvas_hasher.update(seed.to_le_bytes());
        let canvas_hash = hex::encode(&canvas_hasher.finalize()[..16]);

        // Audio context hash
        let mut audio_hasher = Sha256::new();
        audio_hasher.update(b"audio:");
        audio_hasher.update(seed.to_le_bytes());
        let audio_context_hash = hex::encode(&audio_hasher.finalize()[..16]);

        // Client rects variation: small float in [0.0, 0.05]
        let client_rects_variation = ((s10 % 500) as f64) / 10000.0;

        let font_set = FONT_SETS[(s6 as usize) % FONT_SETS.len()];

        BrowserProfile {
            user_agent: USER_AGENTS[(s1 as usize) % USER_AGENTS.len()].to_string(),
            viewport_width: 1280 + ((s2 % 640) as u32), // 1280–1919
            viewport_height: 720 + ((s2 >> 16) % 360) as u32, // 720–1079
            timezone: TIMEZONES[(s3 as usize) % TIMEZONES.len()].to_string(),
            locale: LOCALES[(s4 as usize) % LOCALES.len()].to_string(),
            platform: PLATFORMS[(s5 as usize) % PLATFORMS.len()].to_string(),
            fonts: font_set.iter().map(|s| s.to_string()).collect(),
            canvas_hash,
            webgl_vendor: WEBGL_VENDORS[(s8 as usize) % WEBGL_VENDORS.len()].to_string(),
            audio_context_hash,
            client_rects_variation,
            navigator_hardware_concurrency: [2, 4, 8, 16][(s11 as usize) % 4],
            navigator_device_memory: [2, 4, 8, 16][(s12 as usize) % 4],
        }
    }

    /// Create a new `BrowserSandbox` with default anti-detect config.
    pub fn new(seed: u64) -> Self {
        Self {
            profile: Self::generate_profile(seed),
            config: AntiDetectConfig {
                rotate_fingerprint: true,
                proxy_pool: Vec::new(),
                headless_mode: HeadlessMode::New,
                stealth_scripts: true,
                proxy_index: 0,
            },
        }
    }

    /// Return JavaScript snippets to inject for stealth evasion.
    ///
    /// Patches target common bot detection vectors used by CreepJS,
    /// FingerprintJS, and similar libraries.
    pub fn apply_stealth_patches(&self) -> Vec<String> {
        if !self.config.stealth_scripts {
            return Vec::new();
        }

        let p = &self.profile;
        let mut patches = Vec::with_capacity(8);

        // 1. Override navigator.webdriver
        patches.push(
            r#"Object.defineProperty(navigator, 'webdriver', { get: () => false });"#.to_string(),
        );

        // 2. Patch navigator properties
        patches.push(format!(
            r#"Object.defineProperty(navigator, 'hardwareConcurrency', {{ get: () => {} }});
Object.defineProperty(navigator, 'deviceMemory', {{ get: () => {} }});
Object.defineProperty(navigator, 'platform', {{ get: () => '{}' }});
Object.defineProperty(navigator, 'languages', {{ get: () => ['{}'] }});"#,
            p.navigator_hardware_concurrency, p.navigator_device_memory, p.platform, p.locale,
        ));

        // 3. Canvas fingerprint noise injection
        patches.push(
            r#"(() => {
  const origToDataURL = HTMLCanvasElement.prototype.toDataURL;
  HTMLCanvasElement.prototype.toDataURL = function(type) {
    if (type === 'image/png' || type === undefined) {
      const ctx = this.getContext('2d');
      if (ctx) {
        const style = ctx.fillStyle;
        ctx.fillStyle = 'rgba(255,255,255,0.01)';
        ctx.fillRect(0, 0, 1, 1);
        ctx.fillStyle = style;
      }
    }
    return origToDataURL.apply(this, arguments);
  };
})();"#
                .to_string(),
        );

        // 4. WebGL vendor/renderer override
        patches.push(format!(
            r#"(() => {{
  const getParameter = WebGLRenderingContext.prototype.getParameter;
  WebGLRenderingContext.prototype.getParameter = function(param) {{
    if (param === 37445) return '{}';
    if (param === 37446) return 'ANGLE ({} Direct3D11 vs_5_0 ps_5_0)';
    return getParameter.call(this, param);
  }};
}})();"#,
            p.webgl_vendor, p.webgl_vendor,
        ));

        // 5. AudioContext hash stabilization
        patches.push(
            r#"(() => {
  const origCreateOscillator = AudioContext.prototype.createOscillator;
  AudioContext.prototype.createOscillator = function() {
    const osc = origCreateOscillator.call(this);
    osc._zylos_stable = true;
    return osc;
  };
})();"#
                .to_string(),
        );

        // 6. ClientRects micro-variation
        patches.push(format!(
            r#"(() => {{
  const origGetBoundingClientRect = Element.prototype.getBoundingClientRect;
  Element.prototype.getBoundingClientRect = function() {{
    const rect = origGetBoundingClientRect.call(this);
    const v = {};
    return new DOMRect(rect.x + v, rect.y + v, rect.width, rect.height);
  }};
}})();"#,
            p.client_rects_variation,
        ));

        // 7. Plugin/MimeType array normalisation
        patches.push(
            r#"Object.defineProperty(navigator, 'plugins', { get: () => [1, 2, 3, 4, 5] });
Object.defineProperty(navigator, 'mimeTypes', { get: () => ['application/pdf', 'application/x-nacl'] });"#
                .to_string(),
        );

        // 8. Chrome runtime stub (prevents headless detection via window.chrome)
        patches.push(
            r#"if (!window.chrome) { window.chrome = { runtime: {}, loadTimes: function(){}, csi: function(){} }; }"#
                .to_string(),
        );

        patches
    }

    /// Compute a bot detection risk score in `[0.0, 1.0]`.
    ///
    /// - `0.0` = indistinguishable from a real browser
    /// - `1.0` = certainly detected as automated
    ///
    /// Heuristic based on common CreepJS / FingerprintJS signals.
    pub fn detect_bot_risk(&self) -> f64 {
        let mut risk: f64 = 0.0;

        // Headless mode contributes most risk
        match self.config.headless_mode {
            HeadlessMode::Full => risk += 0.35,
            HeadlessMode::New => risk += 0.10,
            HeadlessMode::Disabled => {}
        }

        // Stealth scripts reduce risk significantly
        if self.config.stealth_scripts {
            risk -= 0.20;
        }

        // Missing proxy pool when rotating is suspicious
        if self.config.rotate_fingerprint && self.config.proxy_pool.is_empty() {
            risk += 0.10;
        }

        // Viewport aspect ratio check — extreme ratios are suspicious
        let ratio = self.profile.viewport_width as f64 / self.profile.viewport_height.max(1) as f64;
        if !(1.2..=2.0).contains(&ratio) {
            risk += 0.05;
        }

        // Canvas hash uniqueness — if it looks too clean, slightly suspicious
        if self.profile.canvas_hash.chars().all(|c| c == '0') {
            risk += 0.05;
        }

        // Clamp to [0.0, 1.0]
        risk.clamp(0.0, 1.0)
    }

    /// Rotate identity by regenerating the profile from a new seed.
    ///
    /// Also advances the proxy round-robin index if a proxy pool is configured.
    pub fn rotate_identity(&mut self, new_seed: u64) {
        self.profile = Self::generate_profile(new_seed);

        // Advance proxy round-robin
        if !self.config.proxy_pool.is_empty() {
            self.config.proxy_index = (self.config.proxy_index + 1) % self.config.proxy_pool.len();
        }
    }

    /// Select next proxy from pool using round-robin.
    ///
    /// Returns `None` if the pool is empty. In production, callers should
    /// perform a health check on the returned proxy before use.
    pub fn next_proxy(&mut self) -> Option<&str> {
        if self.config.proxy_pool.is_empty() {
            return None;
        }
        let proxy = &self.config.proxy_pool[self.config.proxy_index];
        self.config.proxy_index = (self.config.proxy_index + 1) % self.config.proxy_pool.len();
        Some(proxy.as_str())
    }

    /// Select a random proxy from pool using a seed for determinism.
    pub fn random_proxy(&self, seed: u64) -> Option<&str> {
        if self.config.proxy_pool.is_empty() {
            return None;
        }
        let idx = (hash_seed(seed, b"proxy_select") as usize) % self.config.proxy_pool.len();
        Some(&self.config.proxy_pool[idx])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_generation_is_deterministic() {
        let p1 = BrowserSandbox::generate_profile(42);
        let p2 = BrowserSandbox::generate_profile(42);
        assert_eq!(p1.user_agent, p2.user_agent);
        assert_eq!(p1.viewport_width, p2.viewport_width);
        assert_eq!(p1.viewport_height, p2.viewport_height);
        assert_eq!(p1.timezone, p2.timezone);
        assert_eq!(p1.locale, p2.locale);
        assert_eq!(p1.platform, p2.platform);
        assert_eq!(p1.fonts, p2.fonts);
        assert_eq!(p1.canvas_hash, p2.canvas_hash);
        assert_eq!(p1.webgl_vendor, p2.webgl_vendor);
        assert_eq!(p1.audio_context_hash, p2.audio_context_hash);
        assert!((p1.client_rects_variation - p2.client_rects_variation).abs() < f64::EPSILON);
    }

    #[test]
    fn different_seeds_produce_different_profiles() {
        let p1 = BrowserSandbox::generate_profile(1);
        let p2 = BrowserSandbox::generate_profile(2);
        // At least some fields should differ
        let differs = p1.user_agent != p2.user_agent
            || p1.viewport_width != p2.viewport_width
            || p1.timezone != p2.timezone
            || p1.canvas_hash != p2.canvas_hash;
        assert!(differs, "seeds 1 and 2 should produce different profiles");
    }

    #[test]
    fn stealth_patches_return_nonempty_when_enabled() {
        let sandbox = BrowserSandbox::new(42);
        let patches = sandbox.apply_stealth_patches();
        assert!(!patches.is_empty());
        // Should contain webdriver override
        assert!(patches.iter().any(|p| p.contains("webdriver")));
        // Should contain canvas patch
        assert!(patches.iter().any(|p| p.contains("toDataURL")));
        // Should contain WebGL patch
        assert!(patches.iter().any(|p| p.contains("getParameter")));
    }

    #[test]
    fn stealth_patches_empty_when_disabled() {
        let mut sandbox = BrowserSandbox::new(42);
        sandbox.config.stealth_scripts = false;
        let patches = sandbox.apply_stealth_patches();
        assert!(patches.is_empty());
    }

    #[test]
    fn risk_scoring_headless_full_higher_than_new() {
        let mut full = BrowserSandbox::new(42);
        full.config.headless_mode = HeadlessMode::Full;
        let mut new_mode = BrowserSandbox::new(42);
        new_mode.config.headless_mode = HeadlessMode::New;
        assert!(full.detect_bot_risk() > new_mode.detect_bot_risk());
    }

    #[test]
    fn risk_scoring_disabled_lowest() {
        let mut sandbox = BrowserSandbox::new(42);
        sandbox.config.headless_mode = HeadlessMode::Disabled;
        sandbox.config.stealth_scripts = true;
        let risk = sandbox.detect_bot_risk();
        assert!(
            risk < 0.1,
            "disabled headless + stealth should be very low risk: got {}",
            risk
        );
    }

    #[test]
    fn risk_scoring_range() {
        for seed in 0..100 {
            let sandbox = BrowserSandbox::new(seed);
            let risk = sandbox.detect_bot_risk();
            assert!(
                (0.0..=1.0).contains(&risk),
                "risk out of range for seed {}: {}",
                seed,
                risk
            );
        }
    }

    #[test]
    fn identity_rotation_changes_profile() {
        let mut sandbox = BrowserSandbox::new(42);
        let original_ua = sandbox.profile.user_agent.clone();
        let original_canvas = sandbox.profile.canvas_hash.clone();
        sandbox.rotate_identity(99);
        // Profile should have changed (very high probability with different seed)
        let changed = sandbox.profile.user_agent != original_ua
            || sandbox.profile.canvas_hash != original_canvas;
        assert!(
            changed,
            "rotate_identity should produce a different profile"
        );
    }

    #[test]
    fn identity_rotation_is_deterministic() {
        let mut s1 = BrowserSandbox::new(42);
        let mut s2 = BrowserSandbox::new(42);
        s1.rotate_identity(100);
        s2.rotate_identity(100);
        assert_eq!(s1.profile.user_agent, s2.profile.user_agent);
        assert_eq!(s1.profile.canvas_hash, s2.profile.canvas_hash);
    }

    #[test]
    fn proxy_round_robin_cycles() {
        let mut sandbox = BrowserSandbox::new(42);
        sandbox.config.proxy_pool = vec![
            "http://proxy1:8080".to_string(),
            "http://proxy2:8080".to_string(),
            "http://proxy3:8080".to_string(),
        ];
        assert_eq!(sandbox.next_proxy(), Some("http://proxy1:8080"));
        assert_eq!(sandbox.next_proxy(), Some("http://proxy2:8080"));
        assert_eq!(sandbox.next_proxy(), Some("http://proxy3:8080"));
        assert_eq!(sandbox.next_proxy(), Some("http://proxy1:8080")); // wraps
    }

    #[test]
    fn proxy_empty_pool_returns_none() {
        let mut sandbox = BrowserSandbox::new(42);
        assert_eq!(sandbox.next_proxy(), None);
        assert_eq!(sandbox.random_proxy(42), None);
    }

    #[test]
    fn random_proxy_deterministic() {
        let mut sandbox = BrowserSandbox::new(42);
        sandbox.config.proxy_pool = vec![
            "http://a:80".to_string(),
            "http://b:80".to_string(),
            "http://c:80".to_string(),
        ];
        let p1 = sandbox.random_proxy(77);
        let p2 = sandbox.random_proxy(77);
        assert_eq!(p1, p2);
    }

    #[test]
    fn headless_mode_display() {
        assert_eq!(HeadlessMode::Full.to_string(), "full");
        assert_eq!(HeadlessMode::New.to_string(), "new");
        assert_eq!(HeadlessMode::Disabled.to_string(), "disabled");
    }
}
